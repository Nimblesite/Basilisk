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
        // Reconstruct the source text from the resolved module's cached source.
        let text = resolved.source.clone();
        let diagnostics = entry.diagnostics.clone();
        Some((text, resolved, diagnostics))
    }

    /// Return just the source text for a URI (used by handlers that don't need
    /// the resolved module, e.g. formatting and code actions).
    pub fn get_text(&self, uri: &Url) -> Option<String> {
        let path = uri.to_file_path().ok()?;
        let entry = self.files.get(&path)?;
        entry.resolved.as_ref().map(|r| r.source.clone())
    }

    /// Analyse a file from in-memory text (called on `didOpen` / `didChange`).
    ///
    /// Marks the file as open and updates the index. Returns the LSP
    /// diagnostics ready for publishing.
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
        self.files.insert(path, entry);
        lsp_diags
    }

    /// Re-read a file from disk (called on `didClose` or file-watcher events).
    ///
    /// If the file is currently open, this is a no-op (editor text is
    /// authoritative). Returns `None` if the file could not be read or the
    /// hash is unchanged.
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
        self.files.insert(path, entry);
        Some((uri.clone(), lsp_diags))
    }

    /// Mark a file as closed. After this call, file-watcher events for the
    /// path will cause a disk re-read. Returns the disk-based diagnostics.
    pub fn set_closed(
        &self,
        uri: &Url,
    ) -> Option<(Url, Vec<tower_lsp::lsp_types::Diagnostic>)> {
        let path = uri.to_file_path().ok()?;
        if let Some(mut entry) = self.files.get_mut(&path) {
            entry.is_open = false;
            entry.version = 0;
        }
        // Re-analyse from disk now that the editor is no longer authoritative.
        let text = std::fs::read_to_string(&path).ok()?;
        let (entry, lsp_diags) = analyse(&text, &path);
        self.files.insert(path, entry);
        Some((uri.clone(), lsp_diags))
    }

    /// Scan all workspace roots and populate the index.
    ///
    /// Returns a list of `(Uri, diagnostics)` pairs ready for publishing.
    /// Files already open in the editor are skipped.
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
                self.files.insert(path, entry);
                Some((uri, lsp_diags))
            })
            .collect();

        let error_count = results
            .iter()
            .map(|(_, diags)| {
                diags
                    .iter()
                    .filter(|d| {
                        d.severity
                            == Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR)
                    })
                    .count()
            })
            .sum();

        (results, file_count, error_count)
    }

    /// Collect all `(uri, resolved, text)` triples currently in the index,
    /// used by workspace symbol search.
    pub fn all_resolved(
        &self,
    ) -> Vec<(
        Url,
        Arc<basilisk_resolver::ResolvedModule>,
        String,
    )> {
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

/// Run the full parse → resolve → check pipeline on `text`.
///
/// Always returns a `FileEntry` (resolved may be `None` on failure) and the
/// corresponding LSP diagnostics.
fn analyse(
    text: &str,
    path: &Path,
) -> (FileEntry, Vec<tower_lsp::lsp_types::Diagnostic>) {
    let path_str = path.to_string_lossy().into_owned();
    let hash = fnv1a(text);

    let parsed = match basilisk_parser::parse_source(text.to_owned(), path_str) {
        Ok(p) => p,
        Err(e) => {
            let lsp_diag = parse_error_diagnostic(&e.to_string());
            return (
                FileEntry {
                    source_hash: hash,
                    resolved: None,
                    diagnostics: vec![],
                    version: 0,
                    is_open: false,
                },
                vec![lsp_diag],
            );
        }
    };

    let resolved = match basilisk_resolver::resolve(&parsed) {
        Ok(r) => r,
        Err(_) => {
            return (
                FileEntry {
                    source_hash: hash,
                    resolved: None,
                    diagnostics: vec![],
                    version: 0,
                    is_open: false,
                },
                vec![],
            );
        }
    };

    let checker_diags = basilisk_checker::check(&resolved);
    let lsp_diags = checker_diags
        .iter()
        .map(|d| bsk_to_lsp(d, text))
        .collect();

    (
        FileEntry {
            source_hash: hash,
            resolved: Some(Arc::new(resolved)),
            diagnostics: checker_diags,
            version: 0,
            is_open: false,
        },
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
                if existing.extension().and_then(|s| s.to_str()) == Some("py")
                    && ext == Some("pyi")
                {
                    by_stem.insert(key, path);
                }
            }
            None => {
                by_stem.insert(key, path);
            }
        }
    }
    by_stem.into_values().collect()
}

/// Convert a filesystem path to an LSP `Url`.
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
    use tower_lsp::lsp_types::{
        Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range,
    };
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
pub fn resolve_analysis_mode(
    init_options: Option<&serde_json::Value>,
    roots: &[PathBuf],
) -> AnalysisMode {
    // 1. InitializationOptions (highest priority — set by VS Code setting).
    if let Some(mode_str) = init_options
        .and_then(|o| o.get("analysisMode"))
        .and_then(|v| v.as_str())
    {
        return AnalysisMode::from_str(mode_str);
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
}
