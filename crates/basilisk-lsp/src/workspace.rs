//! Workspace index — persistent per-file analysis state for whole-module and
//! cross-module analysis modes.
//!
//! See `docs/WHOLE-MODULE-ANALYSIS-SPEC.md` for the full specification.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use dashmap::DashMap;
use tower_lsp::lsp_types::Url;

use crate::config::{AnalysisMode, WorkspaceConfig};

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
}

impl WorkspaceIndex {
    /// Create an empty index for the given roots and mode.
    #[must_use]
    pub fn new(roots: Vec<PathBuf>, mode: AnalysisMode) -> Self {
        Self {
            roots,
            files: DashMap::new(),
            mode,
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
        let (entry, lsp_diags) = analyse(text, &path);
        let mut entry = entry;
        entry.is_open = true;
        entry.version = version;
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
}

// ── Analysis helpers ─────────────────────────────────────────────────────────

/// Build a `FileEntry` with the given analysis results.
fn make_entry(
    hash: u64,
    text: &str,
    resolved: Option<Arc<basilisk_resolver::ResolvedModule>>,
    checker_diags: Vec<basilisk_checker::Diagnostic>,
) -> FileEntry {
    FileEntry {
        source_hash: hash,
        text: text.to_owned(),
        resolved,
        diagnostics: checker_diags,
        version: 0,
        is_open: false,
    }
}

/// Run the full parse → resolve → check pipeline on `text`.
///
/// Always returns a `FileEntry` (resolved may be `None` on failure) and the
/// corresponding LSP diagnostics.
fn analyse(text: &str, path: &Path) -> (FileEntry, Vec<tower_lsp::lsp_types::Diagnostic>) {
    let path_str = path.to_string_lossy().into_owned();
    let hash = fnv1a(text);

    let parsed = match basilisk_parser::parse_source(text.to_owned(), path_str) {
        Ok(p) => p,
        Err(e) => {
            let lsp_diag = parse_error_diagnostic(&e.to_string());
            return (make_entry(hash, text, None, vec![]), vec![lsp_diag]);
        }
    };

    let Ok(resolved) = basilisk_resolver::resolve(&parsed) else {
        return (make_entry(hash, text, None, vec![]), vec![]);
    };

    let checker_diags = basilisk_checker::check(&resolved);
    let lsp_diags = checker_diags.iter().map(|d| bsk_to_lsp(d, text)).collect();

    (
        make_entry(hash, text, Some(Arc::new(resolved)), checker_diags),
        lsp_diags,
    )
}

// ── File collection ──────────────────────────────────────────────────────────

/// Recursively collect all `.py` / `.pyi` files under `dir`, skipping hidden
/// dirs, common non-source directories, and user-configured exclude paths.
pub fn collect_python_files(
    dir: &Path,
    out: &mut Vec<PathBuf>,
    exclude: &[PathBuf],
    workspace_root: &Path,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with('.')
                || name_str == "__pycache__"
                || name_str == "node_modules"
                || name_str == "venv"
                || name_str == ".tox"
                || name_str == ".mypy_cache"
                || name_str == ".ruff_cache"
            {
                continue;
            }
            if is_excluded(&path, exclude, workspace_root) {
                continue;
            }
            collect_python_files(&path, out, exclude, workspace_root);
        } else if path
            .extension()
            .is_some_and(|ext| ext == "py" || ext == "pyi")
        {
            out.push(path);
        }
    }
}

/// Check if a path matches any configured exclude patterns.
#[must_use]
pub fn is_excluded(path: &Path, exclude: &[PathBuf], workspace_root: &Path) -> bool {
    let relative = path.strip_prefix(workspace_root).unwrap_or(path);
    exclude.iter().any(|exc| relative.starts_with(exc))
}

/// Deduplicate a list of Python files by stem, preferring `.pyi` over `.py`.
fn deduplicate_by_stem(files: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut by_stem: HashMap<String, PathBuf> = HashMap::new();
    for path in files {
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let ext = path.extension().and_then(|s| s.to_str());
        let key = stem.to_owned();
        match by_stem.get(&key) {
            Some(existing) => {
                if existing.extension().and_then(|s| s.to_str()) == Some("py") && ext == Some("pyi")
                {
                    let _ = by_stem.insert(key, path);
                }
            }
            None => {
                let _ = by_stem.insert(key, path);
            }
        }
    }
    by_stem.into_values().collect()
}

/// Convert a filesystem path to an LSP `Url`.
#[must_use]
pub fn path_to_uri(path: &Path) -> Option<Url> {
    Url::from_file_path(path).ok()
}

// ── Diagnostic conversion ────────────────────────────────────────────────────

const FALLBACK_DOCS_URL: &str = "https://www.basilisk-python.dev";

fn bsk_to_lsp(d: &basilisk_checker::Diagnostic, text: &str) -> tower_lsp::lsp_types::Diagnostic {
    use crate::util::byte_offset_to_position;
    use tower_lsp::lsp_types::{
        CodeDescription, Diagnostic, DiagnosticSeverity, NumberOrString, Range, Url,
    };

    let start = byte_offset_to_position(text, d.span.start as usize);
    let end = byte_offset_to_position(text, d.span.end as usize);
    let severity = match d.severity {
        basilisk_checker::Severity::Error | basilisk_checker::Severity::SafetyViolation => {
            DiagnosticSeverity::ERROR
        }
        basilisk_checker::Severity::Warning => DiagnosticSeverity::WARNING,
        basilisk_checker::Severity::Info => DiagnosticSeverity::INFORMATION,
    };
    let Ok(fallback) = Url::parse(FALLBACK_DOCS_URL) else {
        return Diagnostic {
            range: Range { start, end },
            severity: Some(severity),
            code: Some(NumberOrString::String(d.code.code.to_owned())),
            source: Some("basilisk".to_owned()),
            message: d.message.clone(),
            ..Default::default()
        };
    };
    Diagnostic {
        range: Range { start, end },
        severity: Some(severity),
        code: Some(NumberOrString::String(d.code.code.to_owned())),
        code_description: Some(CodeDescription {
            href: Url::parse(d.code.docs_url).unwrap_or(fallback),
        }),
        source: Some("basilisk".to_owned()),
        message: d.message.clone(),
        ..Default::default()
    }
}

fn parse_error_diagnostic(message: &str) -> tower_lsp::lsp_types::Diagnostic {
    use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range};
    Diagnostic {
        range: Range {
            start: Position::new(0, 0),
            end: Position::new(0, 0),
        },
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String("BSK-PARSE".to_owned())),
        source: Some("basilisk".to_owned()),
        message: format!("Parse error: {message}"),
        ..Default::default()
    }
}

// ── Hash ─────────────────────────────────────────────────────────────────────

/// FNV-1a 64-bit hash of a string slice.
fn fnv1a(s: &str) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    s.bytes().fold(OFFSET, |acc, byte| {
        (acc ^ u64::from(byte)).wrapping_mul(PRIME)
    })
}

// ── WorkspaceConfig helper ───────────────────────────────────────────────────

/// Extract `AnalysisMode` from `InitializationOptions` JSON, falling back to
/// workspace config file, then the hard default (`WholeModule`).
#[must_use]
pub fn resolve_analysis_mode(
    init_options: Option<&serde_json::Value>,
    roots: &[PathBuf],
) -> AnalysisMode {
    // 1. InitializationOptions (highest priority — set by VS Code setting).
    if let Some(mode_str) = init_options
        .and_then(|o| o.get("analysisMode"))
        .and_then(|v| v.as_str())
    {
        return AnalysisMode::parse(mode_str);
    }

    // 2. Workspace config file.
    if let Some(root) = roots.first() {
        let cfg: WorkspaceConfig = crate::config::load_config(root);
        if cfg.analysis_mode != AnalysisMode::WholeModule {
            return cfg.analysis_mode;
        }
        // WholeModule is also the default, but an explicit file config of WholeModule
        // is indistinguishable from the default. Either way the result is correct.
        return cfg.analysis_mode;
    }

    // 3. Hard default.
    AnalysisMode::WholeModule
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

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
        let dir = std::env::temp_dir().join("bsk_set_closed_test");
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
        let dir = std::env::temp_dir().join("bsk_reload_test");
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
        let dir = std::env::temp_dir().join("bsk_scan_test");
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
        let dir = std::env::temp_dir().join("bsk_scan_skip_open");
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
