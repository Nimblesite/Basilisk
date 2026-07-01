//! Implements [CHKARCH-INCREMENTAL-SALSA] adoption. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA
//!
//! In-session Salsa analysis engine for the LSP.
//!
//! Holds a persistent [`basilisk_checker::BasiliskDatabase`] plus the salsa
//! **input** handles (one [`SourceFile`] per file, one [`ConfigInput`] per root,
//! one [`SearchPathsInput`] for the workspace). On each analysis it sets those
//! inputs to the current values and reads the memoized queries
//! ([`resolved_module`] for navigation, [`file_diagnostics_resolved`] for
//! diagnostics), so re-analysing an unchanged file is served from the memo and a
//! config-only edit re-runs only the cheap `check` step. Input handles are
//! reused across calls (salsa memoization is identity-based) and salsa backdates
//! a `set` to an unchanged value, so setting every input on every call is cheap.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use basilisk_checker::imports::ImportSearchPaths;
use basilisk_checker::{
    file_diagnostics_resolved, resolved_module, BasiliskDatabase, ConfigInput, ConfigValue,
    Diagnostic, FileRegistry, ResolvedFile, SearchPathsInput, SourceFile, WorkspaceFiles,
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

/// Persistent Salsa database + input handles backing the LSP's incremental
/// single-file analysis.
pub(crate) struct SalsaAnalysisEngine {
    db: Mutex<BasiliskDatabase>,
    /// Per-file source-text inputs, keyed by absolute path.
    sources: DashMap<PathBuf, SourceFile>,
    /// Per-root configuration inputs, keyed by the file's owning root.
    config_inputs: DashMap<PathBuf, ConfigInput>,
    /// The single workspace-wide import search-paths input.
    search_paths_input: Mutex<Option<SearchPathsInput>>,
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
            search_paths_input: Mutex::new(None),
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
    /// `root_key` is the file's owning workspace root (the config-input key).
    pub(crate) fn analyse(
        &self,
        path: &Path,
        text: &str,
        config: &BasiliskConfig,
        root_key: &Path,
        search_paths: &ImportSearchPaths,
    ) -> EngineAnalysis {
        let path_str = path.to_string_lossy().into_owned();

        let mut db = self
            .db
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let source = self.source_for(&mut db, path, &path_str, text);
        let config_input = self.config_for(&mut db, root_key, config);
        let search_paths_input = self.search_paths_for(&mut db, search_paths);
        let workspace = self.workspace_files_for(&mut db);

        // One memoized parse+resolve, whose outcome distinguishes a parse error
        // (→ BSK-PARSE) from a resolve error (→ nothing) from a resolved module.
        match resolved_module(&*db, source, search_paths_input, workspace) {
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
                let diagnostics = file_diagnostics_resolved(
                    &*db,
                    source,
                    config_input,
                    search_paths_input,
                    workspace,
                );
                EngineAnalysis {
                    resolved,
                    diagnostics,
                    parse_error: None,
                }
            }
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

    /// Get-or-create the [`SourceFile`] input for `path`, set to `text`.
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
            let _ = source.set_text(db).to(text.to_owned());
            source
        } else {
            let source = SourceFile::new(&*db, path_str.to_owned(), text.to_owned());
            let _ = self.sources.insert(path.to_path_buf(), source);
            // A new file joined the workspace — the registry must be rebuilt.
            self.registry_dirty.store(true, Ordering::Relaxed);
            source
        }
    }

    /// Get-or-create the [`ConfigInput`] for `root_key`, set to `config`.
    fn config_for(
        &self,
        db: &mut BasiliskDatabase,
        root_key: &Path,
        config: &BasiliskConfig,
    ) -> ConfigInput {
        if let Some(existing) = self.config_inputs.get(root_key) {
            let input = *existing;
            drop(existing);
            let _ = input.set_value(db).to(ConfigValue(config.clone()));
            input
        } else {
            let input = ConfigInput::new(&*db, ConfigValue(config.clone()));
            let _ = self.config_inputs.insert(root_key.to_path_buf(), input);
            input
        }
    }

    /// Get-or-create the workspace [`SearchPathsInput`], set to `search_paths`.
    fn search_paths_for(
        &self,
        db: &mut BasiliskDatabase,
        search_paths: &ImportSearchPaths,
    ) -> SearchPathsInput {
        let mut guard = self
            .search_paths_input
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(input) = *guard {
            let _ = input.set_value(db).to(search_paths.clone());
            input
        } else {
            let input = SearchPathsInput::new(&*db, search_paths.clone());
            *guard = Some(input);
            input
        }
    }
}

#[cfg(test)]
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
        }
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

        let _ = engine.analyse(path, "x = 1\n", &config, root, &sp);
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
