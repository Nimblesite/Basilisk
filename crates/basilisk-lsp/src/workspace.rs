//! Implements [ANALYSIS-INDEX]. See docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-INDEX
//! Workspace index — persistent per-file analysis state for whole-module and
//! cross-module analysis modes.
//!
//! See `docs/LSP-ANALYSIS-MODES-SPEC.md` for the full specification.

use std::path::PathBuf;
use std::sync::Arc;

use basilisk_config::BasiliskConfig;
use basilisk_uv::PackageRegistry;
use dashmap::DashMap;
use tower_lsp::lsp_types::Url;

use crate::config::AnalysisMode;
use crate::import_graph::ImportGraph;
use crate::workspace_analysis::{analyse_with_config, bsk_to_lsp, fnv1a};
use crate::workspace_scan::{collect_python_files, deduplicate_by_stem, path_to_uri};

// ── FileEntry ────────────────────────────────────────────────────────────────

/// Per-file analysis state cached in the workspace index.
#[derive(Debug)]
pub struct FileEntry {
    /// FNV-1a hash of the source text at last analysis; used for invalidation.
    pub source_hash: u64,
    /// Raw source text — always present, even when parsing/resolving failed.
    pub text: String,
    /// Resolved symbol table from the last successful parse+resolve cycle.
    /// `None` if the file failed to parse or resolve.
    pub resolved: Option<Arc<basilisk_resolver::ResolvedModule>>,
    /// Diagnostics from the last check cycle.
    pub diagnostics: Vec<basilisk_checker::Diagnostic>,
    /// LSP document version (non-zero for open documents).
    pub version: i32,
    /// `true` iff the editor currently has this file open; editor text is authoritative.
    pub is_open: bool,
}

// ── WorkspaceIndex ───────────────────────────────────────────────────────────

/// Process-scoped index of all analysed files.
///
/// Owned by `LspServer`. All handlers access file state through this type
/// rather than the old `DashMap<Url, DocumentState>`.
pub struct WorkspaceIndex {
    /// Workspace root directories.
    pub roots: Vec<PathBuf>,
    /// File path → analysis state.
    pub files: DashMap<PathBuf, FileEntry>,
    /// Analysis mode controlling which files are analysed.
    pub mode: AnalysisMode,
    /// Import dependency graph for cross-module invalidation.
    ///
    /// Built during workspace scan in `crossModule` mode.
    /// Protected by a `Mutex` for interior mutability.
    pub import_graph: std::sync::Mutex<ImportGraph>,
    /// Package registry from uv lock file, if this is a uv project.
    ///
    /// Built during workspace initialisation and rebuilt when `uv.lock`
    /// changes. Used for import classification and dependency diagnostics.
    pub registry: Option<Arc<PackageRegistry>>,
    /// Per-root project-level checker configuration.
    ///
    /// Each workspace root can have its own `pyproject.toml` or `basilisk.json`
    /// with different rule severity overrides, per-module, and per-path settings.
    /// Files are matched to their owning root to apply the correct config.
    pub root_configs: std::collections::HashMap<PathBuf, BasiliskConfig>,
    /// Fallback checker configuration used when a file doesn't belong to any
    /// known root, or for single-root backwards compatibility.
    pub checker_config: BasiliskConfig,
    /// Import search paths (venv site-packages, workspace members, stub paths,
    /// uv registry) cached from the last full workspace scan.
    ///
    /// Reused by the incremental single-file analysis path (`didOpen` /
    /// `didChange` / disk reload) so third-party import resolution matches the
    /// full scan and the editor does not resurrect false `BSK-E0010`.
    /// Implements [ANALYSIS-INCR-IMPORTS]. See
    /// docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-INCR-IMPORTS
    pub search_paths: std::sync::RwLock<Option<Arc<crate::import_resolver::ImportSearchPaths>>>,
}

impl std::fmt::Debug for WorkspaceIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceIndex")
            .field("roots", &self.roots)
            .field("mode", &self.mode)
            .field("file_count", &self.files.len())
            .finish_non_exhaustive()
    }
}

impl WorkspaceIndex {
    /// Create an empty index for the given roots, mode, and project config.
    ///
    /// Each root is checked for its own `pyproject.toml` / `basilisk.json`.
    /// If a root has no config file, the provided `checker_config` is used as
    /// the fallback for that root.
    #[must_use]
    pub fn new(roots: Vec<PathBuf>, mode: AnalysisMode, checker_config: BasiliskConfig) -> Self {
        let root_configs = Self::load_root_configs(&roots, &checker_config);
        Self {
            roots,
            files: DashMap::new(),
            mode,
            import_graph: std::sync::Mutex::new(ImportGraph::new()),
            registry: None,
            root_configs,
            checker_config,
            search_paths: std::sync::RwLock::new(None),
        }
    }

    /// Load each root's `BasiliskConfig` from its `pyproject.toml` /
    /// `basilisk.json`, falling back to `fallback` for roots without a config
    /// file.
    ///
    /// [CHKARCH-VERSION-TARGET] An explicit `python-version` in the config wins;
    /// otherwise the project's target is detected from `.python-version` /
    /// `requires-python` / `uv.lock` so version-aware rules follow the real
    /// target (issue #93).
    fn load_root_configs(
        roots: &[PathBuf],
        fallback: &BasiliskConfig,
    ) -> std::collections::HashMap<PathBuf, BasiliskConfig> {
        roots
            .iter()
            .map(|root| {
                let has_config =
                    root.join("pyproject.toml").is_file() || root.join("basilisk.json").is_file();
                let mut cfg = if has_config {
                    basilisk_config::load_basilisk_config(root)
                } else {
                    fallback.clone()
                };
                if cfg.python_version.is_none() {
                    cfg.python_version =
                        basilisk_uv::python_version::resolve_target_python_version(root);
                }
                (root.clone(), cfg)
            })
            .collect()
    }

    /// Re-read every root's `BasiliskConfig` from disk so a change to a watched
    /// config file (`pyproject.toml` / `basilisk.json` / `.python-version`)
    /// takes effect — version-aware rules and severity overrides — without an
    /// LSP restart. The caller re-checks open files afterwards (e.g. via
    /// [`Self::recheck_all_files`]). Implements [CHKARCH-VERSION-TARGET].
    pub fn reload_root_configs(&mut self) {
        self.root_configs = Self::load_root_configs(&self.roots, &self.checker_config);
    }

    /// Cache the import search paths built during the workspace scan.
    ///
    /// Subsequent incremental analyses (`didOpen` / `didChange` / disk reload)
    /// resolve imports against these so the editor's diagnostics match the
    /// full-scan diagnostics. Implements [ANALYSIS-INCR-IMPORTS].
    pub fn set_search_paths(&self, search_paths: crate::import_resolver::ImportSearchPaths) {
        if let Ok(mut guard) = self.search_paths.write() {
            *guard = Some(Arc::new(search_paths));
        }
    }

    /// Snapshot the cached import search paths, if a scan has built them.
    #[must_use]
    fn search_paths_snapshot(&self) -> Option<Arc<crate::import_resolver::ImportSearchPaths>> {
        self.search_paths
            .read()
            .ok()
            .and_then(|guard| guard.clone())
    }

    /// Run the analysis pipeline for one file, then resolve its imports against
    /// the cached search paths and re-check.
    ///
    /// When no search paths are cached yet (before the first scan completes),
    /// this is identical to a plain parse → resolve → check. Once the scan has
    /// populated the search paths, incremental edits resolve third-party and
    /// workspace imports exactly like the full scan — without this, every
    /// `didOpen` / `didChange` re-marks imports `Unresolved`, resurrecting
    /// false `BSK-E0010` in the editor for packages the CLI resolves fine.
    /// Implements [ANALYSIS-INCR-IMPORTS].
    fn analyse_and_resolve(
        &self,
        text: &str,
        path: &std::path::Path,
    ) -> (FileEntry, Vec<tower_lsp::lsp_types::Diagnostic>) {
        let config = self.config_for_file(path);
        let (mut entry, lsp_diags) = analyse_with_config(text, path, config);

        // Excluded files (vendored/bundled) are parsed so navigation still
        // works, but never contribute diagnostics — the editor's per-file path
        // must match the bulk workspace scan and `basilisk check`, which skip
        // them. Without this, opening a `bundled/` file squiggles every line.
        // Implements [CHKARCH-CONFIG-EXCLUDE].
        if self.is_path_excluded(path) {
            entry.diagnostics.clear();
            return (entry, Vec::new());
        }

        let Some(search_paths) = self.search_paths_snapshot() else {
            return (entry, lsp_diags);
        };
        let Some(resolved_arc) = entry.resolved.as_mut() else {
            return (entry, lsp_diags);
        };

        let resolved = Arc::make_mut(resolved_arc);
        crate::import_resolver::resolve_module_imports(resolved, &search_paths);

        let checker_diags = basilisk_checker::check_with_config(resolved, config);
        let lsp_diags = checker_diags
            .iter()
            .map(|d| crate::workspace_analysis::bsk_to_lsp(d, text))
            .collect();
        entry.diagnostics = checker_diags;
        (entry, lsp_diags)
    }

    /// Get the checker config for a file, looking up the owning root.
    ///
    /// Finds the root that is a prefix of the file path, and returns
    /// that root's config. Falls back to the default `checker_config`.
    #[must_use]
    pub fn config_for_file(&self, file_path: &std::path::Path) -> &BasiliskConfig {
        // Find the longest matching root (most specific).
        self.roots
            .iter()
            .filter(|root| file_path.starts_with(root))
            .max_by_key(|root| root.components().count())
            .and_then(|root| self.root_configs.get(root))
            .unwrap_or(&self.checker_config)
    }

    /// Whether `file_path` matches the owning root's `exclude` patterns.
    ///
    /// Uses the same gitignore-style matcher as the workspace scan
    /// (`basilisk_config::path_matches_pattern`, relative to the owning root),
    /// so the incremental per-file path agrees with the bulk scan on which
    /// vendored/bundled files are skipped. Implements [CHKARCH-CONFIG-EXCLUDE].
    #[must_use]
    fn is_path_excluded(&self, file_path: &std::path::Path) -> bool {
        let Some(root) = self
            .roots
            .iter()
            .filter(|root| file_path.starts_with(root))
            .max_by_key(|root| root.components().count())
        else {
            return false;
        };
        let config = self.root_configs.get(root).unwrap_or(&self.checker_config);
        let relative = file_path.strip_prefix(root).unwrap_or(file_path);
        config
            .exclude
            .iter()
            .any(|pattern| basilisk_config::path_matches_pattern(relative, pattern))
    }

    /// Return the `FileEntry` for a URI, if present.
    ///
    /// Canonicalizes the path to handle macOS `/var` → `/private/var` symlinks
    /// and other platform symlink differences.
    #[must_use]
    pub fn get_by_uri(
        &self,
        uri: &Url,
    ) -> Option<(
        String,
        Arc<basilisk_resolver::ResolvedModule>,
        Vec<basilisk_checker::Diagnostic>,
    )> {
        let path = uri.to_file_path().ok()?;
        // Try the literal path first, then canonicalized (handles symlinks).
        let entry = self.files.get(&path).or_else(|| {
            let canonical = path.canonicalize().ok()?;
            self.files.get(&canonical)
        })?;
        let resolved = entry.resolved.clone()?;
        let text = entry.text.clone();
        let diagnostics = entry.diagnostics.clone();
        Some((text, resolved, diagnostics))
    }

    /// Return just the source text for a URI (used by handlers that don't need
    /// the resolved module, e.g. formatting and code actions).
    ///
    /// Returns the raw text even when parsing/resolving failed, so that
    /// handlers like completion can attempt their own recovery.
    #[must_use]
    pub fn get_text(&self, uri: &Url) -> Option<String> {
        let path = uri.to_file_path().ok()?;
        let entry = self.files.get(&path).or_else(|| {
            let canonical = path.canonicalize().ok()?;
            self.files.get(&canonical)
        })?;
        Some(entry.text.clone())
    }

    /// Analyse a file from in-memory text (called on `didOpen` / `didChange`).
    ///
    /// Marks the file as open and updates the index. Returns the LSP
    /// diagnostics ready for publishing.
    #[must_use]
    pub fn set_open(
        &self,
        uri: &Url,
        text: &str,
        version: i32,
    ) -> Vec<tower_lsp::lsp_types::Diagnostic> {
        let path = uri.to_file_path().unwrap_or_default();

        // Capture cross-module data from the previous entry before overwriting.
        let prev_cross_module = self.files.get(&path).and_then(|prev| {
            prev.resolved.as_ref().map(|r| {
                (
                    r.imported_symbols.clone(),
                    r.imports
                        .iter()
                        .filter_map(|imp| {
                            imp.resolved_path
                                .as_ref()
                                .map(|p| (imp.module.clone(), p.clone()))
                        })
                        .collect::<Vec<_>>(),
                )
            })
        });

        let (entry, lsp_diags) = self.analyse_and_resolve(text, &path);
        let mut entry = entry;
        entry.is_open = true;
        entry.version = version;

        // Restore cross-module symbols and resolved import paths from the
        // previous entry so that goto-definition and other cross-module
        // features keep working after didOpen re-parses the file.
        if let Some((prev_symbols, prev_resolved_paths)) = prev_cross_module {
            if let Some(ref mut resolved_arc) = entry.resolved {
                let resolved = Arc::make_mut(resolved_arc);
                if resolved.imported_symbols.is_empty() {
                    resolved.imported_symbols = prev_symbols;
                }
                for (module, resolved_path) in prev_resolved_paths {
                    for imp in &mut resolved.imports {
                        if imp.module == module && imp.resolved_path.is_none() {
                            imp.resolved_path = Some(resolved_path.clone());
                        }
                    }
                }
            }
        }

        let _ = self.files.insert(path, entry);
        lsp_diags
    }

    /// Like [`Self::set_open`], but in cross-module mode also refreshes
    /// dependents when the edited (open) file's exported symbol set changes — so
    /// editing an OPEN module updates its importers live. `set_open` alone
    /// re-analyses only the edited file, and the file-watcher path skips open
    /// files (`reload_from_disk` bails when `is_open`), so without this an
    /// in-editor export edit leaves dependents stale until the file is closed.
    /// Implements [ANALYSIS-SYMBOLS-INVAL] for the open-file path (GitHub #56).
    #[must_use]
    pub fn set_open_refresh_dependents(
        &self,
        uri: &Url,
        text: &str,
        version: i32,
    ) -> Vec<(Url, Vec<tower_lsp::lsp_types::Diagnostic>)> {
        let path = uri.to_file_path().unwrap_or_default();
        let track_exports = matches!(self.mode, AnalysisMode::CrossModule);
        let before = track_exports.then(|| self.exported_symbol_names(&path));
        let own_diags = self.set_open(uri, text, version);
        if before.is_some_and(|prev| self.exported_symbol_names(&path) != prev) {
            // Exports changed: re-resolve + re-check so importers' stale symbol
            // diagnostics refresh without closing the file or reloading the server.
            return self.reresolve_imports_and_recheck();
        }
        vec![(uri.clone(), own_diags)]
    }

    /// Re-read a file from disk (called on `didClose` or file-watcher events).
    ///
    /// If the file is currently open, this is a no-op (editor text is
    /// authoritative). Returns `None` if the file could not be read or the
    /// hash is unchanged.
    #[must_use]
    pub fn reload_from_disk(
        &self,
        uri: &Url,
    ) -> Option<(Url, Vec<tower_lsp::lsp_types::Diagnostic>)> {
        let path = uri.to_file_path().ok()?;

        // Skip if the editor has the file open.
        if self.files.get(&path).is_some_and(|e| e.is_open) {
            return None;
        }

        let text = std::fs::read_to_string(&path).ok()?;
        let new_hash = fnv1a(&text);

        // Skip if content unchanged.
        if self
            .files
            .get(&path)
            .is_some_and(|e| e.source_hash == new_hash)
        {
            return None;
        }

        let (entry, lsp_diags) = self.analyse_and_resolve(&text, &path);
        let _ = self.files.insert(path, entry);
        Some((uri.clone(), lsp_diags))
    }

    /// Mark a file as closed. After this call, file-watcher events for the
    /// path will cause a disk re-read. Returns the disk-based diagnostics.
    #[must_use]
    pub fn set_closed(&self, uri: &Url) -> (Url, Vec<tower_lsp::lsp_types::Diagnostic>) {
        let Some(path) = uri.to_file_path().ok() else {
            return (uri.clone(), vec![]);
        };
        if let Some(mut entry) = self.files.get_mut(&path) {
            entry.is_open = false;
            entry.version = 0;
        }
        // Re-analyse from disk now that the editor is no longer authoritative.
        // If the file no longer exists on disk, remove it from the index and
        // clear its diagnostics (e.g. an in-memory-only test file).
        let Ok(text) = std::fs::read_to_string(&path) else {
            let _ = self.files.remove(&path);
            return (uri.clone(), vec![]);
        };
        let (entry, lsp_diags) = self.analyse_and_resolve(&text, &path);
        let _ = self.files.insert(path, entry);
        (uri.clone(), lsp_diags)
    }

    /// Drop a file from the index entirely.
    ///
    /// Used when a watched file is deleted on disk so that a subsequent
    /// workspace re-resolution does not resurrect its diagnostics from a stale
    /// entry. Implements [ANALYSIS-INCR-IMPORTS].
    pub fn forget_file(&self, uri: &Url) {
        if let Ok(path) = uri.to_file_path() {
            let _ = self.files.remove(&path);
        }
    }

    /// Re-check every indexed file with its current `ResolvedModule` and return
    /// the freshly converted LSP diagnostics keyed by URI.
    ///
    /// Updates each entry's stored diagnostics in place. Shared by the
    /// re-resolution path so a single recheck loop serves every caller.
    #[must_use]
    pub fn recheck_all_files(&self) -> Vec<(Url, Vec<tower_lsp::lsp_types::Diagnostic>)> {
        self.files
            .iter_mut()
            .filter_map(|mut entry| {
                let resolved = entry.resolved.clone()?;
                let file_config = self.config_for_file(entry.key());
                let checker_diags = basilisk_checker::check_with_config(&resolved, file_config);
                let lsp_diags: Vec<tower_lsp::lsp_types::Diagnostic> = checker_diags
                    .iter()
                    .map(|d| bsk_to_lsp(d, &entry.text))
                    .collect();
                entry.diagnostics = checker_diags;
                let uri = path_to_uri(entry.key())?;
                Some((uri, lsp_diags))
            })
            .collect()
    }

    /// Re-resolve every indexed file's imports against the cached search paths,
    /// then re-check all files.
    ///
    /// Called when the package layout changes (e.g. a new module is created) so
    /// that files importing the new module stop reporting `BSK-E0010` without an
    /// LSP reload. When no search paths are cached yet this degrades to a plain
    /// recheck. Implements [ANALYSIS-INCR-IMPORTS].
    #[must_use]
    pub fn reresolve_imports_and_recheck(
        &self,
    ) -> Vec<(Url, Vec<tower_lsp::lsp_types::Diagnostic>)> {
        if let Some(search_paths) = self.search_paths_snapshot() {
            crate::import_resolver::resolve_workspace_imports(self, &search_paths);
        }
        // Implements [ANALYSIS-SYMBOLS-INVAL] (GitHub #56): refresh dependents'
        // imported symbols so symbol-level diagnostics don't go stale.
        if matches!(self.mode, AnalysisMode::CrossModule) {
            crate::cross_module::populate_cross_module_symbols(self);
            self.build_import_graph();
        }
        self.recheck_all_files()
    }

    /// Reload one file from disk, reporting whether its exported top-level
    /// symbol set changed. Implements [ANALYSIS-SYMBOLS-INVAL] (GitHub #56).
    pub fn reload_and_diff_exports(
        &self,
        uri: &Url,
    ) -> Option<((Url, Vec<tower_lsp::lsp_types::Diagnostic>), bool)> {
        let path = uri.to_file_path().ok()?;
        let before = self.exported_symbol_names(&path);
        let result = self.reload_from_disk(uri)?;
        let changed = self.exported_symbol_names(&path) != before;
        Some((result, changed))
    }

    /// The directories to scan under `root`: the configured `[tool.basilisk]
    /// include` roots (relative to `root`) if any, else `root` itself. Mirrors
    /// the CLI's `effective_check_paths` so the editor and `basilisk check`
    /// agree on which files are analysed. Implements [CHKARCH-CONFIG-INCLUDE].
    fn scan_dirs_for(&self, root: &std::path::Path) -> Vec<PathBuf> {
        match self.root_configs.get(root) {
            Some(cfg) if !cfg.include.is_empty() => {
                cfg.include.iter().map(|inc| root.join(inc)).collect()
            }
            _ => vec![root.to_path_buf()],
        }
    }

    /// Scan all workspace roots and populate the index.
    ///
    /// Returns a list of `(Uri, diagnostics)` pairs ready for publishing.
    /// Files already open in the editor are skipped.
    #[must_use]
    pub fn scan(
        &self,
    ) -> (
        Vec<(Url, Vec<tower_lsp::lsp_types::Diagnostic>)>,
        usize,
        usize,
    ) {
        let mut all_files: Vec<PathBuf> = Vec::new();

        for root in &self.roots {
            let cfg = crate::config::load_config(root);
            for scan_dir in self.scan_dirs_for(root) {
                collect_python_files(&scan_dir, &mut all_files, &cfg.exclude, root);
            }
        }

        // Prefer .pyi over .py when both exist for the same stem.
        let deduped = deduplicate_by_stem(all_files);
        let file_count = deduped.len();

        let results: Vec<(Url, Vec<tower_lsp::lsp_types::Diagnostic>)> = deduped
            .into_iter()
            .filter_map(|path| {
                // Skip files already open in the editor.
                if self.files.get(&path).is_some_and(|e| e.is_open) {
                    return None;
                }
                let text = std::fs::read_to_string(&path).ok()?;
                let uri = path_to_uri(&path)?;
                let (entry, lsp_diags) =
                    analyse_with_config(&text, &path, self.config_for_file(&path));
                let _ = self.files.insert(path, entry);
                Some((uri, lsp_diags))
            })
            .collect();

        let error_count = results
            .iter()
            .map(|(_, diags)| {
                diags
                    .iter()
                    .filter(|d| d.severity == Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR))
                    .count()
            })
            .sum();

        (results, file_count, error_count)
    }

    /// Collect all `(uri, resolved, text)` triples currently in the index,
    /// used by workspace symbol search.
    #[must_use]
    pub fn all_resolved(&self) -> Vec<(Url, Arc<basilisk_resolver::ResolvedModule>, String)> {
        self.files
            .iter()
            .filter_map(|entry| {
                let path = entry.key().clone();
                let resolved = entry.resolved.clone()?;
                let text = resolved.source.clone();
                let uri = path_to_uri(&path)?;
                Some((uri, resolved, text))
            })
            .collect()
    }

    /// Build (or rebuild) the import graph from the current index state.
    ///
    /// Called after workspace scan or when the analysis mode is `CrossModule`.
    pub fn build_import_graph(&self) {
        let Ok(mut graph) = self.import_graph.lock() else {
            return;
        };
        *graph = ImportGraph::new();
        graph.build_from_index(self);
    }

    /// Re-analyse files that transitively depend on a changed file.
    ///
    /// Returns `(uri, diagnostics)` pairs for all files that were re-analysed
    /// due to the change. The changed file itself is NOT included (it should
    /// already have been re-analysed by the caller).
    #[must_use]
    pub fn invalidate_dependents(
        &self,
        changed_path: &std::path::Path,
    ) -> Vec<(Url, Vec<tower_lsp::lsp_types::Diagnostic>)> {
        let importers = {
            let Ok(graph) = self.import_graph.lock() else {
                return vec![];
            };
            graph.transitive_importers(changed_path)
        };

        let mut results = Vec::new();
        for importer_path in importers {
            // Re-analyse using the stored text (could be in-memory or from disk).
            let text = {
                let Some(entry) = self.files.get(&importer_path) else {
                    continue;
                };
                entry.text.clone()
            };

            let (new_entry, lsp_diags) = self.analyse_and_resolve(&text, &importer_path);
            let version = self.files.get(&importer_path).map_or(0, |e| e.version);
            let is_open = self.files.get(&importer_path).is_some_and(|e| e.is_open);
            let mut entry = new_entry;
            entry.version = version;
            entry.is_open = is_open;
            let _ = self.files.insert(importer_path.clone(), entry);

            if let Some(uri) = path_to_uri(&importer_path) {
                results.push((uri, lsp_diags));
            }
        }

        results
    }

    /// Map uv workspace members to LSP workspace folder URIs.
    ///
    /// Parses `[tool.uv.workspace]` from `pyproject.toml` at each workspace
    /// root, resolves member paths, and converts them to `lsp_types::WorkspaceFolder`
    /// entries. This enables multi-root LSP features (diagnostics, navigation)
    /// to work seamlessly across workspace members.
    ///
    /// Returns an empty vec if no uv workspace is configured.
    #[must_use]
    pub fn workspace_member_folders(&self) -> Vec<tower_lsp::lsp_types::WorkspaceFolder> {
        let mut folders = Vec::new();

        for root in &self.roots {
            let Ok(Some(workspace)) = basilisk_uv::parse_uv_workspace(root) else {
                continue;
            };

            for member_dir in &workspace.members {
                let Some(uri) = Url::from_file_path(member_dir).ok() else {
                    continue;
                };
                let name = member_dir
                    .file_name()
                    .map_or_else(|| uri.to_string(), |n| n.to_string_lossy().into_owned());

                folders.push(tower_lsp::lsp_types::WorkspaceFolder { uri, name });
            }
        }

        folders
    }

    /// Extract the set of exported symbol names from a file.
    ///
    /// Used for diffing exports before and after a re-analysis to determine
    /// whether dependents need invalidation.
    #[must_use]
    pub fn exported_symbol_names(
        &self,
        path: &std::path::Path,
    ) -> std::collections::HashSet<String> {
        let mut names = std::collections::HashSet::new();
        let Some(entry) = self.files.get(path) else {
            return names;
        };
        let Some(resolved) = &entry.resolved else {
            return names;
        };
        for func in &resolved.functions {
            let _ = names.insert(func.name.clone());
        }
        for class in &resolved.classes {
            let _ = names.insert(class.name.clone());
        }
        for var in &resolved.module_vars {
            let _ = names.insert(var.name.clone());
        }
        names
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only code: unwrap acceptable in unit tests"
)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::config::AnalysisMode;
    use crate::workspace_analysis::fnv1a;
    use crate::workspace_analysis::resolve_analysis_mode;
    use crate::workspace_scan::{deduplicate_by_stem, is_excluded};

    static TEST_CTR: AtomicU64 = AtomicU64::new(0);

    /// Generate a unique temp dir path to avoid races between parallel tests.
    fn unique_tmp(prefix: &str) -> std::path::PathBuf {
        let n = TEST_CTR.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("{prefix}_{n}_{}", std::process::id()))
    }

    #[test]
    fn test_fnv1a_differs_for_different_strings() {
        assert_ne!(fnv1a("hello"), fnv1a("world"));
    }

    #[test]
    fn test_fnv1a_stable() {
        // Must be deterministic across calls.
        assert_eq!(fnv1a("basilisk"), fnv1a("basilisk"));
    }

    #[test]
    fn test_deduplicate_prefers_pyi() {
        let files = vec![
            PathBuf::from("/workspace/foo.py"),
            PathBuf::from("/workspace/foo.pyi"),
            PathBuf::from("/workspace/bar.py"),
        ];
        let deduped = deduplicate_by_stem(files);
        let has_pyi = deduped
            .iter()
            .any(|p| p.extension().is_some_and(|e| e == "pyi"));
        let has_py_foo = deduped
            .iter()
            .any(|p| p.file_name().is_some_and(|n| n == "foo.py"));
        assert!(has_pyi, "should have kept foo.pyi");
        assert!(!has_py_foo, "should have dropped foo.py");
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn test_is_excluded() {
        let root = PathBuf::from("/ws");
        let exclude = vec![PathBuf::from("vendor"), PathBuf::from("build")];
        assert!(is_excluded(
            &PathBuf::from("/ws/vendor/lib.py"),
            &exclude,
            &root
        ));
        assert!(!is_excluded(
            &PathBuf::from("/ws/src/main.py"),
            &exclude,
            &root
        ));
    }

    #[test]
    fn test_resolve_analysis_mode_from_init_options() {
        let opts = serde_json::json!({ "analysisMode": "openFilesOnly" });
        let mode = resolve_analysis_mode(Some(&opts), &[]);
        assert_eq!(mode, AnalysisMode::OpenFilesOnly);
    }

    #[test]
    fn test_resolve_analysis_mode_default() {
        let mode = resolve_analysis_mode(None, &[]);
        assert_eq!(mode, AnalysisMode::WholeModule);
    }

    #[test]
    fn test_resolve_analysis_mode_cross_module() {
        let opts = serde_json::json!({ "analysisMode": "crossModule" });
        let mode = resolve_analysis_mode(Some(&opts), &[]);
        assert_eq!(mode, AnalysisMode::CrossModule);
    }

    #[test]
    fn test_resolve_analysis_mode_whole_module_explicit() {
        let opts = serde_json::json!({ "analysisMode": "wholeModule" });
        let mode = resolve_analysis_mode(Some(&opts), &[]);
        assert_eq!(mode, AnalysisMode::WholeModule);
    }

    #[test]
    fn test_resolve_analysis_mode_unknown_falls_back_to_whole() {
        let opts = serde_json::json!({ "analysisMode": "bogusMode" });
        let mode = resolve_analysis_mode(Some(&opts), &[]);
        assert_eq!(mode, AnalysisMode::WholeModule);
    }

    // ── WorkspaceIndex set_open / get_text ───────────────────────────────────

    fn make_index() -> WorkspaceIndex {
        WorkspaceIndex::new(vec![], AnalysisMode::WholeModule, BasiliskConfig::default())
    }

    fn make_uri(path: &str) -> tower_lsp::lsp_types::Url {
        tower_lsp::lsp_types::Url::parse(&format!("file://{path}")).unwrap()
    }

    #[test]
    fn test_set_open_stores_text_even_on_parse_error() {
        let idx = make_index();
        let uri = make_uri("/tmp/broken.py");
        // Trailing dot is a syntax error.
        let src = "class Dog:\n    pass\n\nDog.";
        let _ = idx.set_open(&uri, src, 1);
        // get_text must return the raw text even though parsing failed.
        let text = idx.get_text(&uri).unwrap();
        assert_eq!(text, src);
    }

    #[test]
    fn test_set_open_stores_text_on_success() {
        let idx = make_index();
        let uri = make_uri("/tmp/valid.py");
        let src = "def foo(x: int) -> int:\n    return x\n";
        let _ = idx.set_open(&uri, src, 1);
        let text = idx.get_text(&uri).unwrap();
        assert_eq!(text, src);
    }

    #[test]
    fn test_set_open_marks_is_open() {
        let idx = make_index();
        let uri = make_uri("/tmp/open.py");
        let _ = idx.set_open(&uri, "x: int = 1\n", 1);
        let path = uri.to_file_path().unwrap();
        let entry = idx.files.get(&path).unwrap();
        assert!(entry.is_open);
        assert_eq!(entry.version, 1);
    }

    #[test]
    fn test_set_open_produces_diagnostics_for_type_error() {
        let idx = make_index();
        let uri = make_uri("/tmp/err.py");
        // Missing return type annotation — should trigger BSK-E0001.
        let src = "def foo(x: int):\n    return x\n";
        let diags = idx.set_open(&uri, src, 1);
        assert!(
            !diags.is_empty(),
            "expected diagnostics for missing return annotation"
        );
    }

    // ── Issue #80 (editor): opening a vendored/excluded file must NOT publish
    //    diagnostics. Fix #80 excluded `bundled`/`_vendored` from the workspace
    //    *scan*, but the per-file path (didOpen/didChange -> set_open ->
    //    analyse_and_resolve) ignored `exclude` and squiggled any opened file.
    //    The editor must match the scan and `basilisk check`.
    #[test]
    fn test_set_open_excluded_file_publishes_no_diagnostics() {
        let root = unique_tmp("bsk_excluded_open");
        // Default config => DEFAULT_EXCLUDES (includes `bundled` / `_vendored`).
        let idx = WorkspaceIndex::new(
            vec![root.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        // A vendored file with blatant type errors that WOULD normally fire.
        let vendored = root.join("bundled").join("debugpy").join("vendored.py");
        let uri = Url::from_file_path(&vendored).unwrap();
        let diags = idx.set_open(&uri, "def f(x):\n    return x\n", 1);
        assert!(
            diags.is_empty(),
            "opening an excluded (bundled/) file must publish no diagnostics, got: {diags:?}"
        );
    }

    // Complement: a non-excluded file under the same root must STILL be checked,
    // so the exclusion is specific rather than disabling diagnostics wholesale.
    #[test]
    fn test_set_open_non_excluded_file_under_root_still_publishes() {
        let root = unique_tmp("bsk_included_open");
        let idx = WorkspaceIndex::new(
            vec![root.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let src_file = root.join("src").join("app.py");
        let uri = Url::from_file_path(&src_file).unwrap();
        let diags = idx.set_open(&uri, "def f(x):\n    return x\n", 1);
        assert!(
            !diags.is_empty(),
            "a non-excluded file under the root must still be checked, got none"
        );
    }

    #[test]
    fn test_get_text_missing_uri_returns_none() {
        let idx = make_index();
        let uri = make_uri("/tmp/nonexistent.py");
        assert!(idx.get_text(&uri).is_none());
    }

    #[test]
    fn test_get_by_uri_returns_none_on_parse_error() {
        // When the file fails to parse, resolved is None, so get_by_uri returns None.
        let idx = make_index();
        let uri = make_uri("/tmp/bad.py");
        let src = "class Dog:\n    pass\n\nDog.";
        let _ = idx.set_open(&uri, src, 1);
        assert!(
            idx.get_by_uri(&uri).is_none(),
            "get_by_uri should be None when resolved is None"
        );
    }

    #[test]
    fn test_get_by_uri_returns_data_on_success() {
        let idx = make_index();
        let uri = make_uri("/tmp/ok.py");
        let src = "x: int = 1\n";
        let _ = idx.set_open(&uri, src, 1);
        let result = idx.get_by_uri(&uri);
        assert!(
            result.is_some(),
            "expected Some from get_by_uri on valid source"
        );
        let (text, _resolved, _diags) = result.unwrap();
        assert_eq!(text, src);
    }

    // ── set_closed ───────────────────────────────────────────────────────────

    #[test]
    fn test_set_closed_nonexistent_file_returns_empty_diagnostics() {
        // A file that was opened in memory but doesn't exist on disk.
        let idx = make_index();
        let uri = make_uri("/tmp/memory_only_xyz123.py");
        let src = "def greet(name):\n    return f\"Hello, {name}!\"\n";
        let _ = idx.set_open(&uri, src, 1);
        // Closing it: file doesn't exist on disk → should return empty diagnostics.
        let (ret_uri, diags) = idx.set_closed(&uri);
        assert_eq!(ret_uri, uri);
        assert!(
            diags.is_empty(),
            "expected empty diagnostics for non-disk file after close"
        );
        // Entry should be removed from the index.
        let path = uri.to_file_path().unwrap();
        assert!(idx.files.get(&path).is_none());
    }

    #[test]
    fn test_set_closed_existing_file_re_analyses() {
        let idx = make_index();
        // Write a real temp file.
        let dir = unique_tmp("bsk_set_closed");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("test.py");
        std::fs::write(&file_path, "x: int = 1\n").unwrap();

        let uri = Url::from_file_path(&file_path).unwrap();
        let _ = idx.set_open(&uri, "x: int = 1\n", 1);
        let (ret_uri, _diags) = idx.set_closed(&uri);
        assert_eq!(ret_uri, uri);
        // Entry is still in the index.
        assert!(idx.files.get(&file_path).is_some());
        // is_open should be false.
        assert!(!idx.files.get(&file_path).unwrap().is_open);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── reload_from_disk ─────────────────────────────────────────────────────

    #[test]
    fn test_reload_from_disk_skips_open_files() {
        let idx = make_index();
        let uri = make_uri("/tmp/openfile.py");
        let _ = idx.set_open(&uri, "x: int = 1\n", 1);
        // reload_from_disk must return None for open files.
        let result = idx.reload_from_disk(&uri);
        assert!(result.is_none(), "should skip open files");
    }

    #[test]
    fn test_reload_from_disk_skips_unchanged_hash() {
        let dir = unique_tmp("bsk_reload");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("unchanged.py");
        let src = "x: int = 1\n";
        std::fs::write(&file_path, src).unwrap();

        let idx = make_index();
        let uri = Url::from_file_path(&file_path).unwrap();
        // First load.
        let _ = idx.reload_from_disk(&uri);
        // Second load — same content, should return None.
        let result = idx.reload_from_disk(&uri);
        // Note: first call returns Some (newly added), second call returns None (no change).
        // We only assert the second call behaviour.
        assert!(result.is_none(), "unchanged file should not re-analyse");

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── scan ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_scan_empty_roots_produces_no_results() {
        let idx = WorkspaceIndex::new(vec![], AnalysisMode::WholeModule, BasiliskConfig::default());
        let (results, file_count, _) = idx.scan();
        assert!(results.is_empty());
        assert_eq!(file_count, 0);
    }

    #[test]
    fn test_scan_collects_python_files() {
        let dir = unique_tmp("bsk_scan");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.py"), "x: int = 1\n").unwrap();
        std::fs::write(dir.join("b.py"), "y: str = 'hi'\n").unwrap();

        let idx = WorkspaceIndex::new(
            vec![dir.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let (results, file_count, _) = idx.scan();
        assert_eq!(file_count, 2, "expected 2 files scanned");
        assert_eq!(results.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Issue #80: vendored / bundled third-party code must not be scanned ───
    //
    // The extension vendors third-party Python under `vscode-extension/bundled/`
    // (debugpy and its nested `_vendored/` tree). Without a default exclude for
    // `bundled`/`_vendored`, the workspace scan type-checks code we ship verbatim
    // and never edit, flooding ~34k irrelevant diagnostics and burying the user's
    // real errors. The scan must skip these directories by default.
    #[test]
    fn test_scan_excludes_bundled_and_vendored_dirs() {
        let dir = unique_tmp("bsk_scan_bundled");
        let bundled = dir.join("vscode-extension").join("bundled").join("debugpy");
        let vendored = dir.join("pkg").join("_vendored").join("pydevd");
        std::fs::create_dir_all(&bundled).unwrap();
        std::fs::create_dir_all(&vendored).unwrap();

        // A real source file that SHOULD be scanned.
        std::fs::write(dir.join("main.py"), "x: int = 1\n").unwrap();
        // Vendored files that SHOULD be skipped.
        std::fs::write(bundled.join("peb_teb.py"), "def f(x):\n    return x\n").unwrap();
        std::fs::write(vendored.join("pydevd.py"), "def g(y):\n    return y\n").unwrap();

        let idx = WorkspaceIndex::new(
            vec![dir.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let (results, file_count, _) = idx.scan();

        assert_eq!(
            file_count, 1,
            "only main.py should be scanned; bundled/_vendored must be excluded"
        );
        let scanned: Vec<String> = results.iter().map(|(uri, _)| uri.to_string()).collect();
        assert!(
            !scanned.iter().any(|u| u.contains("/bundled/")),
            "bundled debugpy code must not be scanned: {scanned:?}"
        );
        assert!(
            !scanned.iter().any(|u| u.contains("/_vendored/")),
            "_vendored code must not be scanned: {scanned:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Issue #80: user-facing `exclude` must accept glob patterns ───────────
    //
    // The workspace `exclude` config is the user's knob for extending the
    // default ignore set. It must support gitignore-style globs applied to both
    // nested directories (`**/generated/**`) and individual files (`*.pb.py`),
    // not just literal path prefixes.
    #[test]
    fn test_scan_user_exclude_supports_glob_patterns() {
        let dir = unique_tmp("bsk_scan_glob_exclude");
        let gen = dir.join("src").join("generated");
        std::fs::create_dir_all(&gen).unwrap();
        std::fs::write(dir.join("app.py"), "x: int = 1\n").unwrap();
        // Excluded by `**/generated/**` (nested directory, any depth).
        std::fs::write(gen.join("models.py"), "y: int = 2\n").unwrap();
        // Excluded by `*.pb.py` (file glob, any depth).
        std::fs::write(dir.join("schema.pb.py"), "z: int = 3\n").unwrap();
        // The user-facing exclude knob, read by the scan via load_config.
        std::fs::write(
            dir.join("basilisk.json"),
            r#"{"exclude": ["**/generated/**", "*.pb.py"]}"#,
        )
        .unwrap();

        let idx = WorkspaceIndex::new(
            vec![dir.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let (results, file_count, _) = idx.scan();
        let scanned: Vec<String> = results.iter().map(|(uri, _)| uri.to_string()).collect();

        assert_eq!(
            file_count, 1,
            "only app.py should survive the glob excludes: {scanned:?}"
        );
        assert!(
            scanned.iter().any(|u| u.ends_with("/app.py")),
            "app.py must still be scanned: {scanned:?}"
        );
        assert!(
            !scanned.iter().any(|u| u.contains("generated")),
            "**/generated/** must exclude the nested directory: {scanned:?}"
        );
        assert!(
            !scanned.iter().any(|u| u.contains("schema.pb.py")),
            "*.pb.py glob must exclude the file: {scanned:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scan_skips_open_files() {
        let dir = unique_tmp("bsk_scan_skip_open");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("open.py");
        std::fs::write(&file_path, "x: int = 1\n").unwrap();

        let idx = WorkspaceIndex::new(
            vec![dir.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let uri = Url::from_file_path(&file_path).unwrap();
        let _ = idx.set_open(&uri, "x: int = 1\n", 1);

        let (results, file_count, _) = idx.scan();
        // File is open, so scan should skip it.
        assert_eq!(file_count, 1, "file_count should count the file");
        assert_eq!(
            results.len(),
            0,
            "open file should be excluded from scan results"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── all_resolved ─────────────────────────────────────────────────────────

    #[test]
    fn test_all_resolved_returns_entries_with_resolved() {
        let idx = make_index();
        let uri1 = make_uri("/tmp/r1.py");
        let uri2 = make_uri("/tmp/r2.py");
        let _ = idx.set_open(&uri1, "x: int = 1\n", 1);
        let _ = idx.set_open(&uri2, "class Bad:\n    pass\nBad.", 1); // parse error → no resolved
        let resolved_list = idx.all_resolved();
        // Only uri1 should appear (uri2 failed to parse).
        assert_eq!(resolved_list.len(), 1);
    }

    // ── Phase 5: uv.lock change triggers registry reparse ───────────────────

    /// Helper: create a minimal uv project with a `uv.lock` and `pyproject.toml`.
    fn create_uv_project(dir: &std::path::Path, packages: &[(&str, &str)]) {
        // pyproject.toml with [tool.uv] so it's detected as a uv project.
        let dep_names: Vec<String> = packages
            .iter()
            .map(|(name, _)| format!("\"{name}\""))
            .collect();
        let pyproject = format!(
            "[project]\nname = \"test-project\"\nversion = \"0.1.0\"\ndependencies = [{}]\n\n[tool.uv]\n",
            dep_names.join(", ")
        );
        std::fs::write(dir.join("pyproject.toml"), pyproject).unwrap();

        // uv.lock with the specified packages.
        write_uv_lock(dir, packages);

        // Marker file so detect_uv_project finds it.
        std::fs::write(
            dir.join("uv.lock"),
            std::fs::read_to_string(dir.join("uv.lock")).unwrap(),
        )
        .unwrap();
    }

    /// Helper: write a uv.lock TOML file with the given packages.
    fn write_uv_lock(dir: &std::path::Path, packages: &[(&str, &str)]) {
        use std::fmt::Write as _;
        let mut lock_content = String::from("version = 1\nrequires-python = \">=3.12\"\n\n");
        for (name, version) in packages {
            let _ = write!(
                lock_content,
                "[[package]]\nname = \"{name}\"\nversion = \"{version}\"\nsource = {{ registry = \"https://pypi.org/simple\" }}\n\n"
            );
        }
        std::fs::write(dir.join("uv.lock"), lock_content).unwrap();
    }

    #[test]
    fn test_lockfile_change_triggers_registry_reparse() {
        let dir = unique_tmp("bsk_uv_reparse");
        std::fs::create_dir_all(&dir).unwrap();
        create_uv_project(&dir, &[("requests", "2.31.0")]);

        // Build initial registry.
        let roots = vec![dir.clone()];
        let registry1 = build_registry_from_roots(&roots);
        assert!(registry1.is_some(), "registry should be built from uv.lock");
        let reg1 = registry1.unwrap();
        assert!(reg1.has_package("requests"));
        assert!(!reg1.has_package("flask"));

        // Simulate uv.lock change: add flask.
        write_uv_lock(&dir, &[("requests", "2.31.0"), ("flask", "3.0.0")]);

        // Re-parse — should pick up flask.
        let registry2 = build_registry_from_roots(&roots);
        assert!(registry2.is_some());
        let reg2 = registry2.unwrap();
        assert!(reg2.has_package("requests"));
        assert!(
            reg2.has_package("flask"),
            "flask should appear after lock change"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Phase 5: add/remove package updates diagnostics ─────────────────────

    #[test]
    fn test_lockfile_add_package_clears_e0010() {
        let dir = unique_tmp("bsk_uv_add_pkg");
        std::fs::create_dir_all(&dir).unwrap();
        create_uv_project(&dir, &[("requests", "2.31.0")]);

        let roots = vec![dir.clone()];
        let config = crate::config::load_config(&dir);

        // Build workspace index with a file that imports `flask`.
        let idx = WorkspaceIndex::new(
            roots.clone(),
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let uri = make_uri(&format!("{}/app.py", dir.display()));
        let _ = idx.set_open(&uri, "import flask\n", 1);

        // Resolve with registry that does NOT have flask.
        rebuild_and_resolve_imports(&idx, &roots, &config);

        // Re-check: flask import should be unresolved (E0010).
        recheck_all(&idx);
        let diags_before = get_diagnostics(&idx, &uri);
        assert!(
            has_diag(&diags_before, "BSK-E0010", "flask"),
            "expected BSK-E0010 for unresolved flask import, got: {diags_before:?}"
        );

        // Now add flask to the lock file and rebuild.
        write_uv_lock(&dir, &[("requests", "2.31.0"), ("flask", "3.0.0")]);
        // Also add flask to pyproject dependencies.
        let pyproject = "[project]\nname = \"test-project\"\nversion = \"0.1.0\"\ndependencies = [\"requests\", \"flask\"]\n\n[tool.uv]\n";
        std::fs::write(dir.join("pyproject.toml"), pyproject).unwrap();

        rebuild_and_resolve_imports(&idx, &roots, &config);
        recheck_all(&idx);

        // After adding flask to the registry, classify_unresolved should now
        // return NeedsSync (in registry but not on filesystem) instead of
        // NotInstalled. The diagnostic message changes accordingly.
        let diags_after = get_diagnostics(&idx, &uri);
        assert!(
            !has_diag(&diags_after, "BSK-E0010", "not a dependency"),
            "flask should no longer show 'not a dependency' after being added to lock: {diags_after:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_lockfile_remove_package_fires_e0010() {
        let dir = unique_tmp("bsk_uv_rm_pkg");
        std::fs::create_dir_all(&dir).unwrap();
        create_uv_project(&dir, &[("requests", "2.31.0"), ("flask", "3.0.0")]);

        let roots = vec![dir.clone()];
        let config = crate::config::load_config(&dir);

        let idx = WorkspaceIndex::new(
            roots.clone(),
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let uri = make_uri(&format!("{}/app.py", dir.display()));
        let _ = idx.set_open(&uri, "import flask\n", 1);

        // Resolve with registry that HAS flask.
        rebuild_and_resolve_imports(&idx, &roots, &config);
        recheck_all(&idx);

        // Now remove flask from the lock file.
        write_uv_lock(&dir, &[("requests", "2.31.0")]);
        let pyproject = "[project]\nname = \"test-project\"\nversion = \"0.1.0\"\ndependencies = [\"requests\"]\n\n[tool.uv]\n";
        std::fs::write(dir.join("pyproject.toml"), pyproject).unwrap();

        rebuild_and_resolve_imports(&idx, &roots, &config);
        recheck_all(&idx);

        let diags = get_diagnostics(&idx, &uri);
        assert!(
            has_diag(&diags, "BSK-E0010", "flask"),
            "expected BSK-E0010 for flask after removal from lock, got: {diags:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Issue #22: sibling-module imports in script directories ─────────────
    //
    // `import configure_agent_backend` from `scripts/configure_agent_backend_test.py`
    // must resolve to the sibling `scripts/configure_agent_backend.py` even when
    // the workspace root is the project root (not `scripts/`). This mirrors
    // Python's `sys.path[0]` behaviour and prevents BSK-E0010 false positives
    // for the common scripts-with-tests pattern.
    #[test]
    fn test_sibling_import_in_scripts_dir_does_not_emit_e0010() {
        let project_root = unique_tmp("bsk_e0010_sibling_root");
        let scripts_dir = project_root.join("scripts");
        std::fs::create_dir_all(&scripts_dir).unwrap();

        // The sibling module being imported.
        std::fs::write(
            scripts_dir.join("configure_agent_backend.py"),
            "VALUE: int = 1\n",
        )
        .unwrap();
        // The importing file lives next to the sibling.
        let test_path = scripts_dir.join("configure_agent_backend_test.py");
        std::fs::write(&test_path, "import configure_agent_backend\n").unwrap();

        // Workspace root is the *project* root — `scripts/` is NOT listed.
        let roots = vec![project_root.clone()];
        let config = crate::config::load_config(&project_root);

        let idx = WorkspaceIndex::new(
            roots.clone(),
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let uri = Url::from_file_path(&test_path).unwrap();
        let _ = idx.set_open(&uri, "import configure_agent_backend\n", 1);

        let search_paths = crate::import_resolver::ImportSearchPaths::from_config(
            &roots, &config, /*registry=*/ None,
        );
        crate::import_resolver::resolve_workspace_imports(&idx, &search_paths);
        recheck_all(&idx);

        let diags = get_diagnostics(&idx, &uri);
        assert!(
            !has_diag(&diags, "BSK-E0010", "configure_agent_backend"),
            "sibling-module import in a script directory must resolve via sys.path[0] \
             fallback; got BSK-E0010: {diags:?}"
        );

        let _ = std::fs::remove_dir_all(&project_root);
    }

    // ── Issue #24: src layout test helpers must resolve ─────────────────────
    //
    // Project layout (very common — pytest src layout):
    //   pyproject.toml
    //   src/agent_backend/__init__.py
    //   src/agent_backend/db/models.py
    //   tests/helpers.py        ← imports `from agent_backend.db.models import X`
    //   tests/test_foo.py       ← imports `from tests.helpers import Y`
    //
    // Both imports must resolve:
    //   * `agent_backend.db.models` — via `src/` on search path (workspace_member)
    //   * `tests.helpers` — via the workspace root being on the search path
    #[test]
    fn test_src_layout_test_helpers_resolve() {
        let root = unique_tmp("bsk_e0010_src_layout");
        let src_pkg = root.join("src").join("agent_backend").join("db");
        let tests_dir = root.join("tests");
        std::fs::create_dir_all(&src_pkg).unwrap();
        std::fs::create_dir_all(&tests_dir).unwrap();

        // pyproject.toml so src layout discovery picks up `src/`.
        std::fs::write(
            root.join("pyproject.toml"),
            "[project]\nname = \"agent_backend\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        // Production package.
        std::fs::write(
            root.join("src").join("agent_backend").join("__init__.py"),
            "",
        )
        .unwrap();
        std::fs::write(
            root.join("src")
                .join("agent_backend")
                .join("db")
                .join("__init__.py"),
            "",
        )
        .unwrap();
        std::fs::write(src_pkg.join("models.py"), "class AgentConfig: ...\n").unwrap();

        // Test helpers (PEP 420 namespace, no __init__.py at tests/).
        let helpers_path = tests_dir.join("helpers.py");
        std::fs::write(
            &helpers_path,
            "from agent_backend.db.models import AgentConfig\n",
        )
        .unwrap();
        let test_path = tests_dir.join("test_foo.py");
        std::fs::write(&test_path, "from tests.helpers import AgentConfig\n").unwrap();

        let roots = vec![root.clone()];
        let config = crate::config::load_config(&root);

        let idx = WorkspaceIndex::new(
            roots.clone(),
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );

        // Open the helper and the test file (mirrors a real LSP session).
        let helpers_uri = Url::from_file_path(&helpers_path).unwrap();
        let _ = idx.set_open(
            &helpers_uri,
            "from agent_backend.db.models import AgentConfig\n",
            1,
        );
        let test_uri = Url::from_file_path(&test_path).unwrap();
        let _ = idx.set_open(&test_uri, "from tests.helpers import AgentConfig\n", 1);

        // Mirror the LSP init flow: from_config discovers workspace_members
        // (src/ for src-layout projects), then imports are resolved.
        let search_paths = crate::import_resolver::ImportSearchPaths::from_config(
            &roots, &config, /*registry=*/ None,
        );
        crate::import_resolver::resolve_workspace_imports(&idx, &search_paths);
        recheck_all(&idx);

        // tests/helpers.py — imports agent_backend.db.models (via src/).
        let helpers_diags = get_diagnostics(&idx, &helpers_uri);
        assert!(
            !has_diag(&helpers_diags, "BSK-E0010", "agent_backend"),
            "BSK-E0010 false positive: src-layout production import from a test \
             helper must resolve via src/ on the search path; got: {helpers_diags:?}"
        );

        // tests/test_foo.py — imports tests.helpers (via workspace root).
        let test_diags = get_diagnostics(&idx, &test_uri);
        assert!(
            !has_diag(&test_diags, "BSK-E0010", "tests.helpers"),
            "BSK-E0010 false positive: `tests.helpers` import must resolve when the \
             workspace root is on the search path; got: {test_diags:?}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    // ── Editor edits must resolve third-party imports (no false BSK-E0010) ───
    //
    // Regression: the full workspace scan resolves `import requests` against the
    // venv site-packages (no BSK-E0010), but opening/editing a file ran parse →
    // syntactic-resolve → check WITHOUT the import search paths, so every
    // third-party import was re-marked `Unresolved` and BSK-E0010 fired in the
    // editor for packages the CLI resolves fine. The diagnostics that
    // `set_open` *publishes* must already reflect import resolution.
    // Implements [ANALYSIS-INCR-IMPORTS].
    #[test]
    fn test_set_open_resolves_site_package_imports_no_e0010() {
        let root = unique_tmp("bsk_incr_imports_e0010");
        // Fake site-packages with a typed `requests` package (py.typed marker).
        let site_packages = root.join("site-packages");
        let requests = site_packages.join("requests");
        std::fs::create_dir_all(&requests).unwrap();
        std::fs::write(requests.join("__init__.py"), "").unwrap();
        std::fs::write(requests.join("py.typed"), "").unwrap();

        let main_path = root.join("main.py");
        std::fs::write(&main_path, "import requests\n").unwrap();

        let roots = vec![root.clone()];
        let idx = WorkspaceIndex::new(
            roots.clone(),
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );

        // Mirror the LSP scan: cache the import search paths in the index.
        // Built directly (not via `from_config`) so an ambient `VIRTUAL_ENV`
        // in the test environment cannot redirect site-packages discovery.
        idx.set_search_paths(crate::import_resolver::ImportSearchPaths {
            roots: roots.clone(),
            extra_paths: vec![],
            stub_paths: vec![],
            workspace_members: vec![],
            site_packages: Some(site_packages.clone()),
            registry: None,
        });

        // Simulate the editor opening the file. The diagnostics it PUBLISHES
        // (the return value) must not contain BSK-E0010 for `requests`.
        let uri = Url::from_file_path(&main_path).unwrap();
        let published = idx.set_open(&uri, "import requests\n", 1);
        assert!(
            !lsp_codes(&published).iter().any(|c| c == "BSK-E0010"),
            "editor-opened file must resolve `requests` via the cached search \
             paths; got BSK-E0010 in published diagnostics: {published:?}"
        );

        // The cached checker diagnostics must agree (used by other features).
        let stored = get_diagnostics(&idx, &uri);
        assert!(
            !has_diag(&stored, "BSK-E0010", "requests"),
            "stored diagnostics must not carry BSK-E0010 for resolved `requests`: {stored:?}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    // ── Phase 5: .python-version change updates stdlib availability ──────────

    #[test]
    fn test_python_version_change_updates_config() {
        let dir = unique_tmp("bsk_uv_pyver");
        std::fs::create_dir_all(&dir).unwrap();

        // Start with Python 3.11.
        std::fs::write(dir.join(".python-version"), "3.11\n").unwrap();
        let ver1 = basilisk_uv::python_version::read_python_version(&dir);
        assert_eq!(ver1, Some("3.11".to_owned()));

        // Simulate .python-version change to 3.12.
        std::fs::write(dir.join(".python-version"), "3.12\n").unwrap();
        let ver2 = basilisk_uv::python_version::read_python_version(&dir);
        assert_eq!(ver2, Some("3.12".to_owned()));

        // Verify that the change is detected and a different value is returned.
        assert_ne!(ver1, ver2, "python version should change after file update");

        // Verify stdlib module availability doesn't regress — `tomllib` was
        // added in 3.11 so it should be available in both versions.
        assert!(
            basilisk_stubs::is_stdlib_module("tomllib"),
            "tomllib should be a stdlib module"
        );

        // `os` should always be available regardless of version.
        assert!(basilisk_stubs::is_stdlib_module("os"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Phase 6: multi-root LSP workspace folder mapping ────────────────────

    #[test]
    fn test_workspace_member_folders_with_uv_workspace() {
        let dir = unique_tmp("bsk_uv_ws_folders");
        std::fs::create_dir_all(&dir).unwrap();

        // Create workspace members.
        let pkg_a = dir.join("packages").join("alpha");
        let pkg_b = dir.join("packages").join("beta");
        std::fs::create_dir_all(&pkg_a).unwrap();
        std::fs::create_dir_all(&pkg_b).unwrap();

        // pyproject.toml with workspace members.
        let pyproject = "[tool.uv.workspace]\nmembers = [\"packages/*\"]\n";
        std::fs::write(dir.join("pyproject.toml"), pyproject).unwrap();

        let idx = WorkspaceIndex::new(
            vec![dir.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let folders = idx.workspace_member_folders();

        assert_eq!(
            folders.len(),
            2,
            "expected 2 workspace folders, got: {folders:?}"
        );

        let names: Vec<&str> = folders.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"alpha"), "should contain alpha: {names:?}");
        assert!(names.contains(&"beta"), "should contain beta: {names:?}");

        // Each folder should have a valid file:// URI.
        for folder in &folders {
            assert!(
                folder.uri.scheme() == "file",
                "folder URI should be file://: {}",
                folder.uri
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_workspace_member_folders_no_uv_workspace() {
        let dir = unique_tmp("bsk_uv_ws_none");
        std::fs::create_dir_all(&dir).unwrap();

        let idx = WorkspaceIndex::new(
            vec![dir.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let folders = idx.workspace_member_folders();

        assert!(
            folders.is_empty(),
            "non-uv workspace should return no folders"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_workspace_member_folders_excludes_are_still_enumerated() {
        // Workspace exclude patterns are stored in the UvWorkspace but
        // the folder mapping enumerates all physical members — filtering
        // by excludes is the caller's responsibility.
        let dir = unique_tmp("bsk_uv_ws_excl");
        std::fs::create_dir_all(&dir).unwrap();

        let pkg = dir.join("libs").join("core");
        std::fs::create_dir_all(&pkg).unwrap();

        let pyproject = "[tool.uv.workspace]\nmembers = [\"libs/*\"]\nexclude = [\"libs/core\"]\n";
        std::fs::write(dir.join("pyproject.toml"), pyproject).unwrap();

        let idx = WorkspaceIndex::new(
            vec![dir.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let folders = idx.workspace_member_folders();

        // The folder mapping reports what's physically present; the caller
        // applies exclude logic.
        assert_eq!(folders.len(), 1);
        if let Some(folder) = folders.first() {
            assert_eq!(folder.name, "core");
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Test helpers ────────────────────────────────────────────────────────

    /// Build a `PackageRegistry` from workspace roots, mirroring the LSP init flow.
    fn build_registry_from_roots(
        roots: &[std::path::PathBuf],
    ) -> Option<Arc<basilisk_uv::PackageRegistry>> {
        let uv_info = basilisk_uv::detect_uv_project(roots)?;
        if !uv_info.has_lockfile {
            return None;
        }
        let lock_path = uv_info.root.join("uv.lock");
        let lock_file = basilisk_uv::parse_lock_file(&lock_path).ok()?;
        let deps = basilisk_uv::extract_pyproject_deps(&uv_info.root);
        let registry = basilisk_uv::PackageRegistry::from_lock_file(&lock_file, &deps);
        Some(Arc::new(registry))
    }

    /// Build the registry from `roots`, derive `ImportSearchPaths`, and run
    /// `resolve_workspace_imports` against `idx`. Collapses the three-line
    /// import-resolution dance that every workspace test repeats.
    fn rebuild_and_resolve_imports(
        idx: &WorkspaceIndex,
        roots: &[std::path::PathBuf],
        config: &crate::config::WorkspaceConfig,
    ) {
        let registry = build_registry_from_roots(roots);
        let search_paths =
            crate::import_resolver::ImportSearchPaths::from_config(roots, config, registry);
        crate::import_resolver::resolve_workspace_imports(idx, &search_paths);
    }

    /// Re-check all files in the workspace index and update their diagnostics.
    fn recheck_all(index: &WorkspaceIndex) {
        for mut entry in index.files.iter_mut() {
            let Some(resolved) = &entry.resolved else {
                continue;
            };
            let checker_diags = basilisk_checker::check(resolved);
            entry.diagnostics = checker_diags;
        }
    }

    /// Extract checker diagnostics for a given URI from the workspace index.
    fn get_diagnostics(index: &WorkspaceIndex, uri: &Url) -> Vec<basilisk_checker::Diagnostic> {
        let path = uri.to_file_path().unwrap();
        index
            .files
            .get(&path)
            .map(|e| e.diagnostics.clone())
            .unwrap_or_default()
    }

    // ── Config-driven severity tests ────────────────────────────────────────
    //
    // These prove that the LSP honours `BasiliskConfig` rule severity
    // overrides identically to the CLI. Before the fix, the LSP always used
    // `BasiliskConfig::default()` and ignored project-level configuration.

    /// Source that triggers BSK-E0001 (missing parameter annotation).
    const SRC_MISSING_ANNOTATION: &str = "def greet(name):\n    return name\n";

    /// Source that triggers BSK-W0050 (redundant type annotation).
    const SRC_REDUNDANT_ANNOTATION: &str = "x: int = 42\n";

    /// Helper: build a `WorkspaceIndex` with a custom `BasiliskConfig`.
    fn make_index_with_config(config: BasiliskConfig) -> WorkspaceIndex {
        WorkspaceIndex::new(vec![], AnalysisMode::WholeModule, config)
    }

    /// Helper: build a `WorkspaceIndex` whose config overrides exactly one rule's severity.
    fn make_index_with_rule_override(
        code: &str,
        severity: basilisk_config::RuleSeverity,
    ) -> WorkspaceIndex {
        let config = BasiliskConfig {
            rules: std::collections::HashMap::from([(code.to_owned(), severity)]),
            ..Default::default()
        };
        make_index_with_config(config)
    }

    /// Helper: extract LSP diagnostic codes for a URI.
    fn lsp_codes(diags: &[tower_lsp::lsp_types::Diagnostic]) -> Vec<String> {
        diags
            .iter()
            .filter_map(|d| match &d.code {
                Some(tower_lsp::lsp_types::NumberOrString::String(s)) => Some(s.clone()),
                _ => None,
            })
            .collect()
    }

    /// Count diagnostics with `code` in `diags`.
    fn count_code(diags: &[basilisk_checker::Diagnostic], code: &str) -> usize {
        diags.iter().filter(|d| d.code.code == code).count()
    }

    /// Build a minimal `basilisk_checker::Diagnostic` for severity-mapping
    /// tests where only `code`, `severity`, and `message` are interesting.
    fn make_test_diag(
        code: &'static str,
        severity: basilisk_checker::Severity,
        message: &str,
    ) -> basilisk_checker::Diagnostic {
        basilisk_checker::Diagnostic {
            code: basilisk_checker::ErrorCode {
                code,
                docs_url: "https://www.basilisk-python.dev/errors/test",
            },
            severity,
            message: message.to_owned(),
            span: basilisk_resolver::Span::new(0, 1),
            path: "test.py".to_owned(),
            help: None,
            note: None,
            provenance: None,
        }
    }

    /// Returns `true` when at least one diagnostic in `diags` has `code` and a
    /// message containing `substring`. Used by tests that look for a
    /// specific (code, message-fragment) pair, e.g. unresolved-flask vs.
    /// unresolved-requests in the same E0010 firing.
    fn has_diag(diags: &[basilisk_checker::Diagnostic], code: &str, substring: &str) -> bool {
        diags
            .iter()
            .any(|d| d.code.code == code && d.message.contains(substring))
    }

    /// Assert that every checker diagnostic for `code` has the expected severity.
    fn assert_checker_severity(
        index: &WorkspaceIndex,
        uri: &Url,
        code: &str,
        expected: basilisk_checker::Severity,
    ) {
        let diags = get_diagnostics(index, uri);
        let matching: Vec<_> = diags.iter().filter(|d| d.code.code == code).collect();
        assert!(!matching.is_empty(), "expected {code} diagnostic, got none");
        for d in &matching {
            assert_eq!(
                d.severity, expected,
                "{code} severity must be {expected:?}, got {:?}",
                d.severity
            );
        }
    }

    /// Assert that every LSP diagnostic for `code` in `diags` has the expected severity.
    fn assert_lsp_severity(
        diags: &[tower_lsp::lsp_types::Diagnostic],
        code: &str,
        expected: tower_lsp::lsp_types::DiagnosticSeverity,
    ) {
        let codes = lsp_codes(diags);
        assert!(
            codes.contains(&code.to_owned()),
            "expected {code} in LSP diagnostics, got {codes:?}"
        );
        for d in diags {
            if let Some(tower_lsp::lsp_types::NumberOrString::String(c)) = &d.code {
                if c == code {
                    assert_eq!(
                        d.severity,
                        Some(expected),
                        "{code} LSP severity must be {expected:?}, got {:?}",
                        d.severity
                    );
                }
            }
        }
    }

    // ── Default config: W-codes are warnings, E-codes are errors ────────────

    #[test]
    fn default_config_w0050_is_warning_in_checker_diagnostics() {
        let idx = make_index();
        let uri = make_uri("/tmp/cfg_w0050_default.py");
        let _ = idx.set_open(&uri, SRC_REDUNDANT_ANNOTATION, 1);
        assert_checker_severity(&idx, &uri, "BSK-W0050", basilisk_checker::Severity::Warning);
    }

    #[test]
    fn default_config_w0050_lsp_severity_is_warning() {
        let idx = make_index();
        let uri = make_uri("/tmp/cfg_w0050_lsp.py");
        let lsp_diags = idx.set_open(&uri, SRC_REDUNDANT_ANNOTATION, 1);
        assert_lsp_severity(
            &lsp_diags,
            "BSK-W0050",
            tower_lsp::lsp_types::DiagnosticSeverity::WARNING,
        );
    }

    #[test]
    fn default_config_e0001_is_error_in_checker_diagnostics() {
        let idx = make_index();
        let uri = make_uri("/tmp/cfg_e0001_default.py");
        let _ = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);
        assert_checker_severity(&idx, &uri, "BSK-E0001", basilisk_checker::Severity::Error);
    }

    #[test]
    fn default_config_e0001_lsp_severity_is_error() {
        let idx = make_index();
        let uri = make_uri("/tmp/cfg_e0001_lsp.py");
        let lsp_diags = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);
        assert_lsp_severity(
            &lsp_diags,
            "BSK-E0001",
            tower_lsp::lsp_types::DiagnosticSeverity::ERROR,
        );
    }

    // ── Global rule severity override: demote error to warning ──────────────

    #[test]
    fn config_override_demotes_e0001_to_warning_in_checker() {
        let idx =
            make_index_with_rule_override("BSK-E0001", basilisk_config::RuleSeverity::Warning);
        let uri = make_uri("/tmp/cfg_demote_e0001.py");
        let _ = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);
        assert_checker_severity(&idx, &uri, "BSK-E0001", basilisk_checker::Severity::Warning);
    }

    #[test]
    fn config_override_demotes_e0001_to_warning_in_lsp() {
        let idx =
            make_index_with_rule_override("BSK-E0001", basilisk_config::RuleSeverity::Warning);
        let uri = make_uri("/tmp/cfg_demote_e0001_lsp.py");
        let lsp_diags = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);
        assert_lsp_severity(
            &lsp_diags,
            "BSK-E0001",
            tower_lsp::lsp_types::DiagnosticSeverity::WARNING,
        );
    }

    // ── Global rule severity override: demote error to info ─────────────────

    #[test]
    fn config_override_demotes_e0001_to_info_in_checker() {
        let idx = make_index_with_rule_override("BSK-E0001", basilisk_config::RuleSeverity::Info);
        let uri = make_uri("/tmp/cfg_info_e0001.py");
        let _ = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);
        assert_checker_severity(&idx, &uri, "BSK-E0001", basilisk_checker::Severity::Info);
    }

    #[test]
    fn config_override_demotes_e0001_to_info_in_lsp() {
        let idx = make_index_with_rule_override("BSK-E0001", basilisk_config::RuleSeverity::Info);
        let uri = make_uri("/tmp/cfg_info_e0001_lsp.py");
        let lsp_diags = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);
        assert_lsp_severity(
            &lsp_diags,
            "BSK-E0001",
            tower_lsp::lsp_types::DiagnosticSeverity::INFORMATION,
        );
    }

    // ── Global rule severity override: disable rule entirely ────────────────

    #[test]
    fn config_override_disables_e0001_removes_from_checker() {
        let idx =
            make_index_with_rule_override("BSK-E0001", basilisk_config::RuleSeverity::Disabled);
        let uri = make_uri("/tmp/cfg_disable_e0001.py");
        let _ = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);

        let diags = get_diagnostics(&idx, &uri);
        let e0001_count = count_code(&diags, "BSK-E0001");
        assert_eq!(
            e0001_count, 0,
            "disabled rule BSK-E0001 must produce zero diagnostics, got {e0001_count}"
        );
    }

    #[test]
    fn config_override_disables_e0001_removes_from_lsp() {
        let idx =
            make_index_with_rule_override("BSK-E0001", basilisk_config::RuleSeverity::Disabled);
        let uri = make_uri("/tmp/cfg_disable_e0001_lsp.py");
        let lsp_diags = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);

        let codes = lsp_codes(&lsp_diags);
        assert!(
            !codes.contains(&"BSK-E0001".to_owned()),
            "disabled BSK-E0001 must not appear in LSP diagnostics, got {codes:?}"
        );
    }

    // ── W0050 severity override: promote warning to error ───────────────────

    #[test]
    fn config_override_promotes_w0050_to_error_in_lsp() {
        // `RuleSeverity::Error` promotes a warning-default rule UP to a hard
        // error, so a project can dial strictness up (e.g. make "no type stubs"
        // a red error) — not just down. BSK-W0050 defaults to Warning; with the
        // override it must surface as ERROR through the LSP.
        let idx = make_index_with_rule_override("BSK-W0050", basilisk_config::RuleSeverity::Error);
        let uri = make_uri("/tmp/cfg_promote_w0050.py");
        let lsp_diags = idx.set_open(&uri, SRC_REDUNDANT_ANNOTATION, 1);
        assert_lsp_severity(
            &lsp_diags,
            "BSK-W0050",
            tower_lsp::lsp_types::DiagnosticSeverity::ERROR,
        );
    }

    // ── W0050 disabled via config ───────────────────────────────────────────

    #[test]
    fn config_override_disables_w0050() {
        let idx =
            make_index_with_rule_override("BSK-W0050", basilisk_config::RuleSeverity::Disabled);
        let uri = make_uri("/tmp/cfg_disable_w0050.py");
        let lsp_diags = idx.set_open(&uri, SRC_REDUNDANT_ANNOTATION, 1);

        let codes = lsp_codes(&lsp_diags);
        assert!(
            !codes.contains(&"BSK-W0050".to_owned()),
            "disabled BSK-W0050 must not appear in LSP diagnostics, got {codes:?}"
        );

        let diags = get_diagnostics(&idx, &uri);
        let w0050_count = count_code(&diags, "BSK-W0050");
        assert_eq!(
            w0050_count, 0,
            "disabled BSK-W0050 must produce zero checker diagnostics"
        );
    }

    // ── uv_stub_suggestions config ──────────────────────────────────────────

    #[test]
    fn config_uv_stub_suggestions_false_suppresses_e0152() {
        // BSK-E0152 should be suppressed when uv_stub_suggestions is false.
        let config = BasiliskConfig {
            uv_stub_suggestions: false,
            ..Default::default()
        };
        let idx = make_index_with_config(config);
        let uri = make_uri("/tmp/cfg_no_stubs.py");
        // Even with source that might trigger E0152, the config suppresses it.
        let src = "import os\n";
        let _ = idx.set_open(&uri, src, 1);

        let diags = get_diagnostics(&idx, &uri);
        let e0152_count = count_code(&diags, "BSK-E0152");
        assert_eq!(
            e0152_count, 0,
            "BSK-E0152 should be suppressed when uv_stub_suggestions is false"
        );
    }

    // ── Config stored on WorkspaceIndex ─────────────────────────────────────

    #[test]
    fn workspace_index_stores_checker_config() {
        let config = BasiliskConfig {
            rules: std::collections::HashMap::from([(
                "BSK-E0001".to_owned(),
                basilisk_config::RuleSeverity::Warning,
            )]),
            uv_stub_suggestions: false,
            ..Default::default()
        };
        let idx = make_index_with_config(config);

        // Verify config is stored and accessible.
        assert_eq!(
            idx.checker_config.rule_severity("BSK-E0001"),
            Some(basilisk_config::RuleSeverity::Warning),
            "checker_config must store the rule severity override"
        );
        assert!(
            !idx.checker_config.uv_stub_suggestions,
            "checker_config must store uv_stub_suggestions=false"
        );
    }

    // ── Config applies across all analysis entry points ─────────────────────

    #[test]
    fn config_applies_to_set_open() {
        let idx =
            make_index_with_rule_override("BSK-E0001", basilisk_config::RuleSeverity::Disabled);
        let uri = make_uri("/tmp/cfg_set_open.py");
        let lsp_diags = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);

        let codes = lsp_codes(&lsp_diags);
        assert!(
            !codes.contains(&"BSK-E0001".to_owned()),
            "set_open must apply checker_config — disabled E0001 should be absent"
        );
    }

    #[test]
    fn config_applies_to_reload_from_disk() {
        let dir = unique_tmp("bsk_cfg_reload");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("reload_cfg.py");
        std::fs::write(&file_path, SRC_MISSING_ANNOTATION).unwrap();

        let idx =
            make_index_with_rule_override("BSK-E0001", basilisk_config::RuleSeverity::Disabled);

        // First, set_open to get it in the index, then close to allow reload.
        let uri = Url::from_file_path(&file_path).unwrap();
        let _ = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);
        idx.files.get_mut(&file_path).unwrap().is_open = false;

        // Modify the content on disk (different hash) so reload_from_disk runs.
        std::fs::write(
            &file_path,
            "def greet(name):\n    return name\n\n# changed\n",
        )
        .unwrap();

        let result = idx.reload_from_disk(&uri);
        assert!(
            result.is_some(),
            "reload_from_disk should return diagnostics"
        );

        let (_, lsp_diags) = result.unwrap();
        let codes = lsp_codes(&lsp_diags);
        assert!(
            !codes.contains(&"BSK-E0001".to_owned()),
            "reload_from_disk must apply checker_config — disabled E0001 should be absent"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reload_root_configs_applies_changed_python_version() {
        // [CHKARCH-VERSION-TARGET] Editing `[tool.basilisk] python-version` must
        // make version-aware rules (the BSK-E0155 PEP 695 gate) update without an
        // LSP restart: reload_root_configs re-reads the target, and the next
        // recheck reflects it.
        let dir = unique_tmp("bsk_cfg_pyver");
        std::fs::create_dir_all(&dir).unwrap();
        let write_version = |v: &str| {
            std::fs::write(
                dir.join("pyproject.toml"),
                format!("[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\npython-version = \"{v}\"\n"),
            )
            .unwrap();
        };
        write_version("3.11");
        let src = "type Alias = int\n";
        let file_path = dir.join("pep695.py");
        std::fs::write(&file_path, src).unwrap();

        let mut idx = WorkspaceIndex::new(
            vec![dir.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let uri = Url::from_file_path(&file_path).unwrap();
        let recheck_has_e0155 = |idx: &WorkspaceIndex| {
            idx.recheck_all_files()
                .into_iter()
                .find(|(u, _)| *u == uri)
                .is_some_and(|(_, d)| lsp_codes(&d).contains(&"BSK-E0155".to_owned()))
        };

        // 3.11 target: PEP 695 `type` syntax is gated.
        let initial = idx.set_open(&uri, src, 1);
        assert!(
            lsp_codes(&initial).contains(&"BSK-E0155".to_owned()),
            "PEP 695 on a 3.11 target must fire BSK-E0155"
        );

        // Switch the configured target to 3.12 on disk.
        write_version("3.12");

        // A recheck WITHOUT reloading config reuses the target cached at
        // construction — still stale 3.11 (the bug this guards against).
        assert!(
            recheck_has_e0155(&idx),
            "without reload, the recheck still uses the stale 3.11 target"
        );

        // Reloading per-root config picks up 3.12, where PEP 695 is native.
        idx.reload_root_configs();
        assert!(
            !recheck_has_e0155(&idx),
            "reload_root_configs must apply the new python-version (3.12 allows PEP 695)"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn config_applies_to_set_closed() {
        let dir = unique_tmp("bsk_cfg_closed");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("close_cfg.py");
        std::fs::write(&file_path, SRC_MISSING_ANNOTATION).unwrap();

        let idx =
            make_index_with_rule_override("BSK-E0001", basilisk_config::RuleSeverity::Disabled);

        let uri = Url::from_file_path(&file_path).unwrap();
        let _ = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);

        let (_, lsp_diags) = idx.set_closed(&uri);
        let codes = lsp_codes(&lsp_diags);
        assert!(
            !codes.contains(&"BSK-E0001".to_owned()),
            "set_closed must apply checker_config — disabled E0001 should be absent"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_honors_include_roots() {
        // [CHKARCH-CONFIG-INCLUDE] The LSP workspace scan must walk only the
        // configured `[tool.basilisk] include` roots, like `basilisk check` —
        // a file outside them (e.g. generated code) must not be scanned.
        let dir = unique_tmp("bsk_scan_include");
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::create_dir_all(dir.join("gen")).unwrap();
        std::fs::write(
            dir.join("pyproject.toml"),
            "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\ninclude = [\"src\"]\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("src/ok.py"),
            "def add(a: int, b: int) -> int:\n    return a + b\n",
        )
        .unwrap();
        // A file OUTSIDE the include roots — must not be scanned.
        std::fs::write(
            dir.join("gen/outside.py"),
            "def bad() -> int:\n    return undefined_name\n",
        )
        .unwrap();

        let idx = WorkspaceIndex::new(
            vec![dir.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let (results, file_count, _errors) = idx.scan();
        let scanned: Vec<String> = results.iter().map(|(u, _)| u.to_string()).collect();

        assert!(
            scanned.iter().any(|u| u.ends_with("src/ok.py")),
            "files inside include roots must be scanned, got: {scanned:?}"
        );
        assert!(
            !scanned.iter().any(|u| u.ends_with("gen/outside.py")),
            "files outside include roots must NOT be scanned, got: {scanned:?}"
        );
        assert_eq!(file_count, 1, "only the included file should be scanned");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn config_applies_to_scan() {
        let dir = unique_tmp("bsk_cfg_scan");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("scan_cfg.py"), SRC_MISSING_ANNOTATION).unwrap();

        let config = BasiliskConfig {
            rules: std::collections::HashMap::from([(
                "BSK-E0001".to_owned(),
                basilisk_config::RuleSeverity::Disabled,
            )]),
            ..Default::default()
        };
        let idx = WorkspaceIndex::new(vec![dir.clone()], AnalysisMode::WholeModule, config);

        let (results, file_count, _) = idx.scan();
        assert!(file_count > 0, "scan should find at least one file");

        for (_, lsp_diags) in &results {
            let codes = lsp_codes(lsp_diags);
            assert!(
                !codes.contains(&"BSK-E0001".to_owned()),
                "scan must apply checker_config — disabled E0001 should be absent, got {codes:?}"
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Multi-root workspace ───────────────────────────────────────────────

    #[test]
    fn multi_root_per_root_config() {
        let root_a = unique_tmp("bsk_multiroot_a");
        let root_b = unique_tmp("bsk_multiroot_b");
        std::fs::create_dir_all(&root_a).unwrap();
        std::fs::create_dir_all(&root_b).unwrap();

        // Root A: disable E0001 via pyproject.toml
        std::fs::write(
            root_a.join("pyproject.toml"),
            "[tool.basilisk.rules]\n\"BSK-E0001\" = \"disabled\"\n",
        )
        .unwrap();
        std::fs::write(root_a.join("a.py"), SRC_MISSING_ANNOTATION).unwrap();

        // Root B: no config file (default rules apply)
        std::fs::write(root_b.join("b.py"), SRC_MISSING_ANNOTATION).unwrap();

        let idx = WorkspaceIndex::new(
            vec![root_a.clone(), root_b.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );

        // Check that root A's config disables E0001
        let cfg_a = idx.config_for_file(&root_a.join("a.py"));
        assert_eq!(
            cfg_a.rule_severity("BSK-E0001"),
            Some(basilisk_config::RuleSeverity::Disabled),
            "root A should have E0001 disabled"
        );

        // Check that root B uses default config (E0001 not overridden)
        let cfg_b = idx.config_for_file(&root_b.join("b.py"));
        assert_eq!(
            cfg_b.rule_severity("BSK-E0001"),
            None,
            "root B should have default config (no E0001 override)"
        );

        let _ = std::fs::remove_dir_all(&root_a);
        let _ = std::fs::remove_dir_all(&root_b);
    }

    #[test]
    fn config_for_file_falls_back_to_default() {
        let root = unique_tmp("bsk_cfgfallback");
        std::fs::create_dir_all(&root).unwrap();

        let idx = WorkspaceIndex::new(
            vec![root.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );

        // File outside any root should fall back to default config.
        let cfg = idx.config_for_file(std::path::Path::new("/nonexistent/foo.py"));
        assert!(
            cfg.rules.is_empty(),
            "fallback config should have no rule overrides"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    // ── Multiple overrides in one config ────────────────────────────────────

    #[test]
    fn config_multiple_overrides_applied_together() {
        let config = BasiliskConfig {
            rules: std::collections::HashMap::from([
                (
                    "BSK-E0001".to_owned(),
                    basilisk_config::RuleSeverity::Warning,
                ),
                (
                    "BSK-W0050".to_owned(),
                    basilisk_config::RuleSeverity::Disabled,
                ),
            ]),
            ..Default::default()
        };
        let idx = make_index_with_config(config);

        // File with both E0001 and W0050 triggers.
        let uri = make_uri("/tmp/cfg_multi.py");
        let src = "x: int = 42\n\ndef greet(name):\n    return name\n";
        let lsp_diags = idx.set_open(&uri, src, 1);
        let codes = lsp_codes(&lsp_diags);

        // W0050 should be gone (disabled).
        assert!(
            !codes.contains(&"BSK-W0050".to_owned()),
            "disabled BSK-W0050 must not appear, got {codes:?}"
        );

        // E0001 should be present but demoted to Warning.
        assert!(
            codes.contains(&"BSK-E0001".to_owned()),
            "demoted BSK-E0001 should still appear, got {codes:?}"
        );

        for d in &lsp_diags {
            if let Some(tower_lsp::lsp_types::NumberOrString::String(code)) = &d.code {
                if code == "BSK-E0001" {
                    assert_eq!(
                        d.severity,
                        Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING),
                        "demoted E0001 must be WARNING in combined config"
                    );
                }
            }
        }

        // Also verify the raw checker diagnostics match.
        let diags = get_diagnostics(&idx, &uri);
        let w0050_count = count_code(&diags, "BSK-W0050");
        assert_eq!(w0050_count, 0, "W0050 disabled in checker too");
        for d in diags.iter().filter(|d| d.code.code == "BSK-E0001") {
            assert_eq!(d.severity, basilisk_checker::Severity::Warning);
        }
    }

    // ── Severity values are correct LSP numbers ─────────────────────────────

    #[test]
    fn lsp_severity_constants_are_distinct() {
        // LSP protocol: ERROR, WARNING, INFORMATION, HINT must all differ.
        let error = tower_lsp::lsp_types::DiagnosticSeverity::ERROR;
        let warning = tower_lsp::lsp_types::DiagnosticSeverity::WARNING;
        let info = tower_lsp::lsp_types::DiagnosticSeverity::INFORMATION;
        let hint = tower_lsp::lsp_types::DiagnosticSeverity::HINT;

        assert_ne!(error, warning, "ERROR and WARNING must differ");
        assert_ne!(error, info, "ERROR and INFORMATION must differ");
        assert_ne!(error, hint, "ERROR and HINT must differ");
        assert_ne!(warning, info, "WARNING and INFORMATION must differ");
        assert_ne!(warning, hint, "WARNING and HINT must differ");
        assert_ne!(info, hint, "INFORMATION and HINT must differ");
    }

    #[test]
    fn bsk_to_lsp_maps_warning_to_warning_not_error() {
        let diag = make_test_diag(
            "BSK-W0050",
            basilisk_checker::Severity::Warning,
            "test warning",
        );
        let lsp_diag = crate::workspace_analysis::bsk_to_lsp(&diag, "x\n");
        assert_eq!(
            lsp_diag.severity,
            Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING),
            "Warning severity must map to LSP WARNING (2), not ERROR (1)"
        );
        assert_ne!(
            lsp_diag.severity,
            Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR),
            "Warning must NEVER map to ERROR"
        );
    }

    #[test]
    fn bsk_to_lsp_maps_error_to_error() {
        let diag = make_test_diag("BSK-E0001", basilisk_checker::Severity::Error, "test error");
        let lsp_diag = crate::workspace_analysis::bsk_to_lsp(&diag, "x\n");
        assert_eq!(
            lsp_diag.severity,
            Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR),
            "Error severity must map to LSP ERROR (1)"
        );
    }

    #[test]
    fn bsk_to_lsp_maps_info_to_information() {
        let diag = make_test_diag("BSK-I0001", basilisk_checker::Severity::Info, "test info");
        let lsp_diag = crate::workspace_analysis::bsk_to_lsp(&diag, "x\n");
        assert_eq!(
            lsp_diag.severity,
            Some(tower_lsp::lsp_types::DiagnosticSeverity::INFORMATION),
            "Info severity must map to LSP INFORMATION (3)"
        );
    }

    // ── Config loaded from pyproject.toml via WorkspaceIndex constructor ────

    #[test]
    fn workspace_index_with_pyproject_config_applies_overrides() {
        let dir = unique_tmp("bsk_cfg_pyproject");
        std::fs::create_dir_all(&dir).unwrap();

        // Write a pyproject.toml that disables E0001.
        std::fs::write(
            dir.join("pyproject.toml"),
            "[tool.basilisk.rules]\n\"BSK-E0001\" = \"disabled\"\n",
        )
        .unwrap();

        // Write a Python file that triggers E0001.
        std::fs::write(dir.join("check_me.py"), SRC_MISSING_ANNOTATION).unwrap();

        // Load config the same way the LSP init does.
        let config = basilisk_config::load_basilisk_config(&dir);
        assert_eq!(
            config.rule_severity("BSK-E0001"),
            Some(basilisk_config::RuleSeverity::Disabled),
            "pyproject.toml should disable BSK-E0001"
        );

        let idx = WorkspaceIndex::new(vec![dir.clone()], AnalysisMode::WholeModule, config);

        // Scan should apply the config.
        let (results, file_count, _) = idx.scan();
        assert!(file_count > 0, "should find at least one file");

        for (_, lsp_diags) in &results {
            let codes = lsp_codes(lsp_diags);
            assert!(
                !codes.contains(&"BSK-E0001".to_owned()),
                "pyproject.toml disabled E0001 must not appear in scan results"
            );
        }

        // Also verify via set_open.
        let uri = Url::from_file_path(dir.join("check_me.py")).unwrap();
        let lsp_diags = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);
        let codes = lsp_codes(&lsp_diags);
        assert!(
            !codes.contains(&"BSK-E0001".to_owned()),
            "pyproject.toml disabled E0001 must not appear via set_open either"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn workspace_index_with_pyproject_demotes_to_warning() {
        let dir = unique_tmp("bsk_cfg_pyproject_demote");
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("pyproject.toml"),
            "[tool.basilisk.rules]\n\"BSK-E0001\" = \"warning\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("demote_me.py"), SRC_MISSING_ANNOTATION).unwrap();

        let config = basilisk_config::load_basilisk_config(&dir);
        let idx = WorkspaceIndex::new(vec![dir.clone()], AnalysisMode::WholeModule, config);

        let uri = Url::from_file_path(dir.join("demote_me.py")).unwrap();
        let lsp_diags = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);

        let codes = lsp_codes(&lsp_diags);
        assert!(
            codes.contains(&"BSK-E0001".to_owned()),
            "demoted E0001 should still appear"
        );

        for d in &lsp_diags {
            if let Some(tower_lsp::lsp_types::NumberOrString::String(code)) = &d.code {
                if code == "BSK-E0001" {
                    assert_eq!(
                        d.severity,
                        Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING),
                        "pyproject.toml demoted E0001 must be WARNING in LSP"
                    );
                }
            }
        }

        // Verify checker diagnostics too.
        let diags = get_diagnostics(&idx, &uri);
        for d in diags.iter().filter(|d| d.code.code == "BSK-E0001") {
            assert_eq!(
                d.severity,
                basilisk_checker::Severity::Warning,
                "pyproject.toml demoted E0001 must be Warning in checker"
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Default config vs custom config produce different results ────────────

    #[test]
    fn default_and_custom_configs_produce_different_severity() {
        // Prove the fix: same source, different configs, different severities.
        let uri_path = "/tmp/cfg_diff.py";

        // Default config: E0001 is Error.
        let default_idx = make_index();
        let default_uri = make_uri(uri_path);
        let default_diags = default_idx.set_open(&default_uri, SRC_MISSING_ANNOTATION, 1);
        let default_severities: Vec<_> = default_diags
            .iter()
            .filter(|d| {
                matches!(&d.code, Some(tower_lsp::lsp_types::NumberOrString::String(c)) if c == "BSK-E0001")
            })
            .filter_map(|d| d.severity)
            .collect();

        // Custom config: E0001 demoted to Warning.
        let custom_idx =
            make_index_with_rule_override("BSK-E0001", basilisk_config::RuleSeverity::Warning);
        let custom_uri = make_uri(uri_path);
        let custom_diags = custom_idx.set_open(&custom_uri, SRC_MISSING_ANNOTATION, 1);
        let custom_severities: Vec<_> = custom_diags
            .iter()
            .filter(|d| {
                matches!(&d.code, Some(tower_lsp::lsp_types::NumberOrString::String(c)) if c == "BSK-E0001")
            })
            .filter_map(|d| d.severity)
            .collect();

        // Both should have E0001 diagnostics.
        assert!(!default_severities.is_empty(), "default must have E0001");
        assert!(!custom_severities.is_empty(), "custom must have E0001");

        // Default = ERROR, Custom = WARNING.
        assert!(
            default_severities
                .iter()
                .all(|s| *s == tower_lsp::lsp_types::DiagnosticSeverity::ERROR),
            "default config E0001 must be ERROR"
        );
        assert!(
            custom_severities
                .iter()
                .all(|s| *s == tower_lsp::lsp_types::DiagnosticSeverity::WARNING),
            "custom config E0001 must be WARNING"
        );

        // They must differ — this is the core assertion proving the fix.
        assert_ne!(
            default_severities, custom_severities,
            "default and custom configs MUST produce different LSP severities for E0001"
        );
    }
}
