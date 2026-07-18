//! Implements [CHKARCH-INCREMENTAL-SALSA] adoption. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA
//!
//! In-session Salsa analysis engine for the LSP.
//!
//! Holds a persistent [`basilisk_checker::BasiliskDatabase`] plus the salsa
//! **input** handles (one [`SourceFile`] per file, one [`ConfigInput`] per config
//! scope, one [`SearchPathsInput`] per workspace root). On each analysis it syncs those
//! inputs to the current values and reads the memoized queries
//! ([`cross_resolved_module`] for navigation, [`file_diagnostics_resolved`] for
//! diagnostics), so re-analysing an unchanged file is served from the memo and a
//! config-only edit re-runs only the cheap `check` step. Input handles are
//! reused across calls (salsa memoization is identity-based), and every write
//! **compares before setting**: salsa 0.27 treats a same-value `set` as a new
//! revision that re-executes dependents (no equality shortcut — pinned by
//! `salsa_set_semantics.rs` in `basilisk-checker`), so unconditional re-sets
//! would silently discard the whole database's memos on every call.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use basilisk_checker::imports::ImportSearchPaths;
use basilisk_checker::{
    cross_resolved_module, file_diagnostics_cross, file_diagnostics_resolved, BasiliskDatabase,
    ConfigInput, ConfigValue, Diagnostic, FileRegistry, ResolvedFile, SearchPathsInput, SourceFile,
    WorkspaceFiles,
};
use basilisk_config::BasiliskConfig;
use dashmap::DashMap;
use salsa::Setter as _;

/// The result of a single-file salsa analysis.
pub(crate) struct EngineAnalysis {
    /// The import-resolved module for navigation, or `None` if the file failed
    /// to parse or resolve.
    pub resolved: Option<Arc<basilisk_resolver::ResolvedModule>>,
    /// The checker diagnostics for the file (empty on parse/resolve failure).
    pub diagnostics: Vec<Diagnostic>,
    /// A parse-error message, if the file failed to parse (drives `BSK-PARSE`).
    pub parse_error: Option<String>,
}

/// Root/config-scoped inputs that accompany one source analysis.
#[derive(Clone, Copy)]
pub(crate) struct AnalysisInputs<'a> {
    pub config: &'a BasiliskConfig,
    pub config_key: &'a Path,
    pub search_paths_root: &'a Path,
    pub search_paths: &'a ImportSearchPaths,
}

/// Persistent Salsa database + input handles backing the LSP's incremental
/// single-file analysis.
pub(crate) struct SalsaAnalysisEngine {
    db: Mutex<BasiliskDatabase>,
    /// Per-file source-text inputs, keyed by absolute path.
    sources: DashMap<PathBuf, SourceFile>,
    /// Per-directory configuration inputs, keyed by the file's config scope.
    config_inputs: DashMap<PathBuf, ConfigInput>,
    /// Per-root import search-path inputs. Distinct target interpreters must
    /// keep distinct Salsa identities or alternating roots would invalidate
    /// and overwrite a shared input.
    search_paths_inputs: DashMap<PathBuf, SearchPathsInput>,
    /// The workspace file registry input for content-precise cross-file
    /// invalidation — a path → `SourceFile` map so a query can depend on the
    /// content of the files it imports (e.g. an edited user-stub `.pyi` updates
    /// its importers). [CHKARCH-INCREMENTAL-SALSA]
    workspace_files: Mutex<Option<WorkspaceFiles>>,
    /// Set when a new `SourceFile` is added, so the next analysis rebuilds
    /// `workspace_files`. Editing an existing file leaves the registry untouched.
    registry_dirty: AtomicBool,
}

impl Default for SalsaAnalysisEngine {
    fn default() -> Self {
        Self {
            db: Mutex::new(BasiliskDatabase::default()),
            sources: DashMap::new(),
            config_inputs: DashMap::new(),
            search_paths_inputs: DashMap::new(),
            workspace_files: Mutex::new(None),
            registry_dirty: AtomicBool::new(false),
        }
    }
}

impl std::fmt::Debug for SalsaAnalysisEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SalsaAnalysisEngine")
            .field("source_count", &self.sources.len())
            .finish_non_exhaustive()
    }
}

impl SalsaAnalysisEngine {
    /// Analyse one file through the memoized salsa queries, resolving imports
    /// against `search_paths`.
    ///
    /// `config_key` identifies the file's merged configuration scope, while
    /// `search_paths_root` identifies its owning workspace target environment.
    /// The resolved module always comes from [`cross_resolved_module`] and so
    /// always carries `imported_symbols` — external stub/py.typed enrichment
    /// drives hover, completion, and navigation in every mode (GitHub #287).
    /// `cross_module` gates only the **diagnostics** query: with it set,
    /// [`file_diagnostics_cross`] sees other tracked files' current content
    /// and editing an imported file's exports invalidates exactly its
    /// importers; without it, the plain diagnostics query keeps byte-for-byte
    /// CLI parity. [CHKARCH-INCREMENTAL-SALSA]
    pub(crate) fn analyse(
        &self,
        path: &Path,
        text: &str,
        inputs: AnalysisInputs<'_>,
        cross_module: bool,
    ) -> EngineAnalysis {
        let path_str = path.to_string_lossy().into_owned();

        let mut db = self
            .db
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let source = self.source_for(&mut db, path, &path_str, text);
        let config_input = self.config_for(&mut db, inputs.config_key, inputs.config);
        let search_paths_input =
            self.search_paths_for(&mut db, inputs.search_paths_root, inputs.search_paths);
        let workspace = self.workspace_files_for(&mut db);

        // One memoized parse+resolve, whose outcome distinguishes a parse error
        // (→ BSK-PARSE) from a resolve error (→ nothing) from a resolved module.
        // The resolved view is always the cross-module one: its
        // `imported_symbols` enrichment from stubs / py.typed packages drives
        // hover, completion, and navigation for external symbols in every mode
        // (GitHub #287). Diagnostics stay mode-gated below, so non-cross modes
        // keep byte-for-byte CLI parity on what they report.
        let outcome = cross_resolved_module(&*db, source, search_paths_input, workspace);
        match outcome {
            ResolvedFile::ParseError(message) => EngineAnalysis {
                resolved: None,
                diagnostics: Vec::new(),
                parse_error: Some(message.clone()),
            },
            ResolvedFile::ResolveError => EngineAnalysis {
                resolved: None,
                diagnostics: Vec::new(),
                parse_error: None,
            },
            ResolvedFile::Resolved(module) => {
                let resolved = Some(Arc::clone(module));
                let diagnostics = if cross_module {
                    file_diagnostics_cross(
                        &*db,
                        source,
                        config_input,
                        search_paths_input,
                        workspace,
                    )
                } else {
                    file_diagnostics_resolved(
                        &*db,
                        source,
                        config_input,
                        search_paths_input,
                        workspace,
                    )
                };
                EngineAnalysis {
                    resolved,
                    diagnostics,
                    parse_error: None,
                }
            }
        }
    }

    /// Bulk-register `files` as tracked `SourceFile` inputs ahead of a sweep.
    ///
    /// A workspace-wide re-analysis calls this once with every indexed file so
    /// the registry is rebuilt a single time (the first subsequent [`Self::analyse`])
    /// instead of once per newly-seen file — and so every cross-file edge sees
    /// every workspace file from the very first query.
    pub(crate) fn prime<I>(&self, files: I)
    where
        I: IntoIterator<Item = (PathBuf, String)>,
    {
        let mut db = self
            .db
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (path, text) in files {
            let path_str = path.to_string_lossy().into_owned();
            let _ = self.source_for(&mut db, &path, &path_str, &text);
        }
    }

    /// Get-or-create the workspace file registry input, rebuilding it from the
    /// current `sources` only when a new file has been added since last time.
    /// Editing an existing file leaves the registry untouched (its content edge
    /// flows through that file's own `SourceFile`), so steady-state editing does
    /// not churn the registry.
    fn workspace_files_for(&self, db: &mut BasiliskDatabase) -> WorkspaceFiles {
        let mut guard = self
            .workspace_files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match *guard {
            Some(input) if !self.registry_dirty.swap(false, Ordering::Relaxed) => input,
            Some(input) => {
                let _ = input.set_files(db).to(self.build_registry());
                input
            }
            None => {
                self.registry_dirty.store(false, Ordering::Relaxed);
                let input = WorkspaceFiles::new(&*db, self.build_registry());
                *guard = Some(input);
                input
            }
        }
    }

    /// Drop a deleted file's `SourceFile` from the engine and mark the registry
    /// stale, so a cross-file edge never points at a path that no longer exists
    /// and the engine's map stays consistent with the workspace index.
    ///
    /// Note: salsa 0.27 cannot reclaim the input's internal storage/memos, so a
    /// re-created file gets a fresh input (the old memo lingers until the DB is
    /// dropped). This keeps the *engine's own* bookkeeping bounded; it is not a
    /// full salsa-DB reclaim.
    pub(crate) fn remove(&self, path: &Path) {
        if self.sources.remove(path).is_some() {
            self.registry_dirty.store(true, Ordering::Relaxed);
        }
    }

    /// Number of files the engine currently tracks (test hook).
    #[cfg(test)]
    pub(crate) fn tracked_source_count(&self) -> usize {
        self.sources.len()
    }

    /// Snapshot the current path → `SourceFile` map for the workspace registry.
    fn build_registry(&self) -> FileRegistry {
        FileRegistry(
            self.sources
                .iter()
                .map(|entry| (entry.key().clone(), *entry.value()))
                .collect(),
        )
    }

    /// Get-or-create the [`SourceFile`] input for `path`, synced to `text`.
    ///
    /// Writes only on a real change: salsa re-executes dependents of ANY `set`,
    /// equal value or not, so an unconditional re-set here would invalidate the
    /// file's whole query chain on every analysis.
    fn source_for(
        &self,
        db: &mut BasiliskDatabase,
        path: &Path,
        path_str: &str,
        text: &str,
    ) -> SourceFile {
        if let Some(existing) = self.sources.get(path) {
            let source = *existing;
            drop(existing);
            if source.text(&*db) != text {
                let _ = source.set_text(db).to(text.to_owned());
            }
            source
        } else {
            let source = SourceFile::new(&*db, path_str.to_owned(), text.to_owned());
            let _ = self.sources.insert(path.to_path_buf(), source);
            // A new file joined the workspace — the registry must be rebuilt.
            self.registry_dirty.store(true, Ordering::Relaxed);
            source
        }
    }

    /// Get-or-create the [`ConfigInput`] for `root_key`, synced to `config`.
    ///
    /// Compare-before-set: re-setting an unchanged config would re-run every
    /// file's `check` step on every analysis (see [`Self::source_for`]).
    fn config_for(
        &self,
        db: &mut BasiliskDatabase,
        root_key: &Path,
        config: &BasiliskConfig,
    ) -> ConfigInput {
        if let Some(existing) = self.config_inputs.get(root_key) {
            let input = *existing;
            drop(existing);
            if input.value(&*db).0 != *config {
                let _ = input.set_value(db).to(ConfigValue(config.clone()));
            }
            input
        } else {
            let input = ConfigInput::new(&*db, ConfigValue(config.clone()));
            let _ = self.config_inputs.insert(root_key.to_path_buf(), input);
            input
        }
    }

    /// Get-or-create one root's [`SearchPathsInput`], synced to `search_paths`.
    ///
    /// Compare-before-set: re-setting unchanged search paths would re-resolve
    /// every file's imports on every analysis (see [`Self::source_for`]).
    fn search_paths_for(
        &self,
        db: &mut BasiliskDatabase,
        root: &Path,
        search_paths: &ImportSearchPaths,
    ) -> SearchPathsInput {
        if let Some(existing) = self.search_paths_inputs.get(root) {
            let input = *existing;
            drop(existing);
            if input.value(&*db) != search_paths {
                let _ = input.set_value(db).to(search_paths.clone());
            }
            input
        } else {
            let input = SearchPathsInput::new(&*db, search_paths.clone());
            let _ = self.search_paths_inputs.insert(root.to_path_buf(), input);
            input
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test-only code: expect acceptable in unit tests"
)]
mod tests {
    use super::*;

    fn empty_search_paths() -> ImportSearchPaths {
        ImportSearchPaths {
            roots: vec![],
            extra_paths: vec![],
            stub_paths: vec![],
            workspace_members: vec![],
            site_packages: None,
            registry: None,
            typeshed_path: None,
            typeshed_snapshot: None,
        }
    }

    /// Regression for #287, wiring layer: the default (non-cross-module)
    /// analysis must still populate `imported_symbols` from external
    /// stub/py.typed packages. Hover, completion, and navigation read the
    /// engine's resolved view; gating the enrichment on the reserved
    /// `crossModule` mode left dot-access hover on inherited external methods
    /// dead in every real editor session.
    #[test]
    fn default_mode_analysis_populates_external_imported_symbols() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let site = dir.path().join("site-packages");
        let pkg = site.join("pydantic");
        std::fs::create_dir_all(&pkg).expect("create package dir");
        std::fs::write(pkg.join("py.typed"), "").expect("write py.typed marker");
        std::fs::write(
            pkg.join("__init__.py"),
            "from typing import TYPE_CHECKING\n\nif TYPE_CHECKING:\n    from .main import *\n",
        )
        .expect("write __init__.py");
        std::fs::write(
            pkg.join("main.py"),
            "class BaseModel:\n    @classmethod\n    def model_validate(cls, obj: object) -> 'BaseModel': ...\n",
        )
        .expect("write main.py");

        let workspace = dir.path().join("ws");
        std::fs::create_dir_all(&workspace).expect("create workspace dir");
        let file = workspace.join("app.py");
        let text = "from pydantic import BaseModel\n\nclass C(BaseModel):\n    name: str\n";
        std::fs::write(&file, text).expect("write app.py");

        let engine = SalsaAnalysisEngine::default();
        let config = BasiliskConfig::default();
        let mut search_paths = empty_search_paths();
        search_paths.site_packages = Some(site);
        search_paths.roots = vec![workspace.clone()];

        let analysis = engine.analyse(
            &file,
            text,
            AnalysisInputs {
                config: &config,
                config_key: &workspace,
                search_paths_root: &workspace,
                search_paths: &search_paths,
            },
            false,
        );
        let resolved = analysis.resolved.expect("module should resolve");
        let base = resolved
            .imported_symbols
            .get("BaseModel")
            .expect("default-mode analysis must populate external imported symbols (GitHub #287)");
        assert!(
            base.methods.iter().any(|m| m.name == "model_validate"),
            "the re-exported class must carry its methods for dot-access hover"
        );
    }

    /// A deleted file's `SourceFile` is dropped from the engine's map so its
    /// bookkeeping stays bounded by tracked files, not every file ever touched.
    #[test]
    fn remove_drops_a_tracked_source() {
        let engine = SalsaAnalysisEngine::default();
        let config = BasiliskConfig::default();
        let sp = empty_search_paths();
        let path = Path::new("/tmp/bsk_engine_remove/a.py");
        let root = Path::new("/tmp/bsk_engine_remove");

        let _ = engine.analyse(
            path,
            "x = 1\n",
            AnalysisInputs {
                config: &config,
                config_key: root,
                search_paths_root: root,
                search_paths: &sp,
            },
            false,
        );
        assert_eq!(
            engine.tracked_source_count(),
            1,
            "analysing a file must track its SourceFile input"
        );

        engine.remove(path);
        assert_eq!(
            engine.tracked_source_count(),
            0,
            "removing a deleted file must drop its SourceFile input"
        );
    }
}
