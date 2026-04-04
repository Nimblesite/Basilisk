//! Analysis pipeline and diagnostic conversion for the workspace index.
//!
//! Runs parse → resolve → check and converts checker diagnostics to LSP
//! diagnostics. Also provides the FNV-1a hash used for file invalidation and
//! the `resolve_analysis_mode` helper for server initialisation.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use basilisk_config::AdoptionStore;

use crate::config::{AnalysisMode, WorkspaceConfig};

use super::workspace::FileEntry;

const FALLBACK_DOCS_URL: &str = "https://www.basilisk-python.dev";

/// Build a `FileEntry` with the given analysis results.
pub(crate) fn make_entry(
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

/// Run the full parse → resolve → check pipeline with project-level config.
///
/// Applies per-module, per-path, and global rule severity overrides from
/// the provided [`BasiliskConfig`].
pub(crate) fn analyse_with_config(
    text: &str,
    path: &Path,
    config: &basilisk_config::BasiliskConfig,
) -> (FileEntry, Vec<tower_lsp::lsp_types::Diagnostic>) {
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

    let checker_diags = basilisk_checker::check_with_config(&resolved, config);
    let lsp_diags = checker_diags.iter().map(|d| bsk_to_lsp(d, text)).collect();

    (
        make_entry(hash, text, Some(Arc::new(resolved)), checker_diags),
        lsp_diags,
    )
}

/// Convert a Basilisk checker diagnostic to an LSP diagnostic.
#[must_use]
pub fn bsk_to_lsp(
    d: &basilisk_checker::Diagnostic,
    text: &str,
) -> tower_lsp::lsp_types::Diagnostic {
    use crate::util::byte_offset_to_position;
    use tower_lsp::lsp_types::{
        CodeDescription, Diagnostic, DiagnosticSeverity, NumberOrString, Range, Url,
    };

    let start = byte_offset_to_position(text, d.span.start_usize());
    let end = byte_offset_to_position(text, d.span.end_usize());
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

/// Build an LSP diagnostic from a parse error message.
pub(crate) fn parse_error_diagnostic(message: &str) -> tower_lsp::lsp_types::Diagnostic {
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

/// FNV-1a 64-bit hash of a string slice.
pub(crate) fn fnv1a(s: &str) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    s.bytes().fold(OFFSET, |acc, byte| {
        (acc ^ u64::from(byte)).wrapping_mul(PRIME)
    })
}

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
        return cfg.analysis_mode;
    }

    // 3. Hard default.
    AnalysisMode::WholeModule
}

/// Apply adoption overrides to checker diagnostics.
///
/// Demotes adopted error codes from `Error` to `Warning` severity for the
/// given file path. The path is made relative to `project_root` before
/// looking up the adoption store.
pub(crate) fn apply_adoptions(
    diagnostics: &mut [basilisk_checker::Diagnostic],
    file_path: &Path,
    project_root: &Path,
    store: &AdoptionStore,
) {
    let relative = file_path.strip_prefix(project_root).unwrap_or(file_path);

    for diag in diagnostics.iter_mut() {
        if diag.severity == basilisk_checker::Severity::Error
            && store.is_demoted(relative, diag.code.code)
        {
            diag.severity = basilisk_checker::Severity::Warning;
        }
    }
}
