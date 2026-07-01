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
    /// invalidation. Empty for now — the query only gains cross-file edges once
    /// cross-module type-sharing moves into it; populating it before then would
    /// only over-invalidate. [CHKARCH-INCREMENTAL-SALSA]
    workspace_files: Mutex<Option<WorkspaceFiles>>,
}

impl Default for SalsaAnalysisEngine {
    fn default() -> Self {
        Self {
            db: Mutex::new(BasiliskDatabase::default()),
            sources: DashMap::new(),
            config_inputs: DashMap::new(),
            search_paths_input: Mutex::new(None),
            workspace_files: Mutex::new(None),
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

    /// Get-or-create the (currently empty) workspace file registry input.
    fn workspace_files_for(&self, db: &mut BasiliskDatabase) -> WorkspaceFiles {
        let mut guard = self
            .workspace_files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard.get_or_insert_with(|| WorkspaceFiles::new(&*db, FileRegistry::default()))
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
