//! Workspace index — persistent per-file analysis state for whole-module and
//! cross-module analysis modes.
//!
//! See `docs/WHOLE-MODULE-ANALYSIS-SPEC.md` for the full specification.

use std::path::PathBuf;
use std::sync::Arc;

use dashmap::DashMap;
use tower_lsp::lsp_types::Url;

use crate::config::AnalysisMode;
use crate::import_graph::ImportGraph;
use crate::workspace_analysis::{analyse, fnv1a};
use crate::workspace_scan::{collect_python_files, deduplicate_by_stem, path_to_uri};

// ── FileEntry ────────────────────────────────────────────────────────────────

/// Per-file analysis state cached in the workspace index.
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
}

impl WorkspaceIndex {
    /// Create an empty index for the given roots and mode.
    #[must_use]
    pub fn new(roots: Vec<PathBuf>, mode: AnalysisMode) -> Self {
        Self {
            roots,
            files: DashMap::new(),
            mode,
            import_graph: std::sync::Mutex::new(ImportGraph::new()),
        }
    }

    /// Return the `FileEntry` for a URI, if present.
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
        let entry = self.files.get(&path)?;
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
        let entry = self.files.get(&path)?;
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

        let (entry, lsp_diags) = analyse(text, &path);
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

        let (entry, lsp_diags) = analyse(&text, &path);
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
        let (entry, lsp_diags) = analyse(&text, &path);
        let _ = self.files.insert(path, entry);
        (uri.clone(), lsp_diags)
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
            collect_python_files(root, &mut all_files, &cfg.exclude, root);
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
                let (entry, lsp_diags) = analyse(&text, &path);
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

            let (new_entry, lsp_diags) = analyse(&text, &importer_path);
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
        WorkspaceIndex::new(vec![], AnalysisMode::WholeModule)
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
        let idx = WorkspaceIndex::new(vec![], AnalysisMode::WholeModule);
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

        let idx = WorkspaceIndex::new(vec![dir.clone()], AnalysisMode::WholeModule);
        let (results, file_count, _) = idx.scan();
        assert_eq!(file_count, 2, "expected 2 files scanned");
        assert_eq!(results.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scan_skips_open_files() {
        let dir = unique_tmp("bsk_scan_skip_open");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("open.py");
        std::fs::write(&file_path, "x: int = 1\n").unwrap();

        let idx = WorkspaceIndex::new(vec![dir.clone()], AnalysisMode::WholeModule);
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
}
