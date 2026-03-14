//! Import-related code actions.
//!
//! Provides: organize imports (ruff), expand wildcard imports, convert import
//! style, and add `__all__` declaration.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::Ordering;

use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, Position, Range, TextEdit, Url, WorkspaceEdit,
};

use super::TMP_COUNTER;

// ── Organize imports via ruff ─────────────────────────────────────────────────

/// Run `ruff check --select I --fix` on the document source and return a
/// full-file replacement [`CodeAction`], or `None` if ruff is not installed or
/// the source is already sorted.
pub(crate) fn organize_imports(uri: &Url, source: &str) -> Option<CodeAction> {
    let id = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_path = std::env::temp_dir().join(format!("basilisk_org_{id}.py"));

    std::fs::write(&tmp_path, source).ok()?;

    let status = std::process::Command::new("ruff")
        .args(["check", "--select", "I", "--fix", "--quiet"])
        .arg(&tmp_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()?;

    // ruff exits 0 (no changes) or 1 (applied fixes); both are success.
    // Exit ≥ 2 means an internal error — skip in that case.
    if !matches!(status.code(), Some(0 | 1)) {
        let _ = std::fs::remove_file(&tmp_path);
        return None;
    }

    let new_source = std::fs::read_to_string(&tmp_path).ok()?;
    let _ = std::fs::remove_file(&tmp_path);

    if new_source == source {
        return None; // Already sorted — don't offer a no-op action.
    }

    let mut changes = HashMap::new();
    let _ = changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: full_document_range(source),
            new_text: new_source,
        }],
    );
    Some(CodeAction {
        title: "Organize imports (ruff)".to_owned(),
        kind: Some(CodeActionKind::SOURCE_ORGANIZE_IMPORTS),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(true),
        ..Default::default()
    })
}

// ── Expand wildcard imports via ruff ──────────────────────────────────────────

/// Run `ruff check --select F403 --fix` on the document source to expand
/// wildcard imports, or `None` if ruff is not installed or no wildcards exist.
pub(crate) fn expand_wildcard_imports(uri: &Url, source: &str) -> Option<CodeAction> {
    if !source.contains("import *") {
        return None;
    }

    let id = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_path = std::env::temp_dir().join(format!("basilisk_wild_{id}.py"));

    std::fs::write(&tmp_path, source).ok()?;

    let status = std::process::Command::new("ruff")
        .args(["check", "--select", "F403", "--fix", "--quiet"])
        .arg(&tmp_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()?;

    if !matches!(status.code(), Some(0 | 1)) {
        let _ = std::fs::remove_file(&tmp_path);
        return None;
    }

    let new_source = std::fs::read_to_string(&tmp_path).ok()?;
    let _ = std::fs::remove_file(&tmp_path);

    if new_source == source {
        return None;
    }

    let mut changes = HashMap::new();
    let _ = changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: full_document_range(source),
            new_text: new_source,
        }],
    );
    Some(CodeAction {
        title: "Expand wildcard imports (ruff)".to_owned(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(false),
        ..Default::default()
    })
}

// ── Convert import style via ruff ─────────────────────────────────────────────

/// Run `ruff check --select E401 --fix` to convert between `import X` and
/// `from X import Y` styles, or `None` if ruff is not installed or no
/// changes are needed.
pub(crate) fn convert_import_style(uri: &Url, source: &str) -> Option<CodeAction> {
    if !source.contains("import ") {
        return None;
    }

    let id = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_path = std::env::temp_dir().join(format!("basilisk_conv_{id}.py"));

    std::fs::write(&tmp_path, source).ok()?;

    let status = std::process::Command::new("ruff")
        .args(["check", "--select", "E401", "--fix", "--quiet"])
        .arg(&tmp_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()?;

    if !matches!(status.code(), Some(0 | 1)) {
        let _ = std::fs::remove_file(&tmp_path);
        return None;
    }

    let new_source = std::fs::read_to_string(&tmp_path).ok()?;
    let _ = std::fs::remove_file(&tmp_path);

    if new_source == source {
        return None;
    }

    let full_range = full_document_range(source);
    let mut changes = HashMap::new();
    let _ = changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: full_range,
            new_text: new_source,
        }],
    );
    Some(CodeAction {
        title: "Fix multiple imports on one line (ruff E401)".to_owned(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(false),
        ..Default::default()
    })
}

// ── Add __all__ declaration ───────────────────────────────────────────────────

/// Offer to add an `__all__` declaration listing all public names in the module.
/// Only offered when `__all__` is not already defined.
pub(crate) fn add_dunder_all(uri: &Url, source: &str) -> Option<CodeAction> {
    if source.contains("__all__") {
        return None;
    }

    let public_names = collect_public_names(source);
    if public_names.is_empty() {
        return None;
    }

    let names_str = public_names
        .iter()
        .map(|n| format!("    \"{n}\","))
        .collect::<Vec<_>>()
        .join("\n");
    let all_text = format!("__all__ = [\n{names_str}\n]\n\n");

    let insert_line = last_import_line(source);
    let insert_pos = Position {
        line: insert_line,
        character: 0,
    };

    let mut changes = HashMap::new();
    let _ = changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: Range {
                start: insert_pos,
                end: insert_pos,
            },
            new_text: all_text,
        }],
    );
    Some(CodeAction {
        title: "Add __all__ declaration (basilisk)".to_owned(),
        kind: Some(CodeActionKind::SOURCE),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(false),
        ..Default::default()
    })
}

/// Collect public (non-underscore-prefixed) top-level names from source text.
fn collect_public_names(source: &str) -> Vec<&str> {
    let mut public_names: Vec<&str> = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("def ") {
            if let Some(name) = rest.split('(').next() {
                let name = name.trim();
                if !name.starts_with('_') {
                    public_names.push(name);
                }
            }
        } else if let Some(rest) = trimmed.strip_prefix("class ") {
            if let Some(name) = rest.split(['(', ':']).next() {
                let name = name.trim();
                if !name.starts_with('_') {
                    public_names.push(name);
                }
            }
        } else if !trimmed.starts_with('#')
            && !trimmed.starts_with("import ")
            && !trimmed.starts_with("from ")
            && !trimmed.is_empty()
        {
            if let Some(name) = trimmed.split('=').next() {
                let name = name.split(':').next().unwrap_or("").trim();
                if !name.is_empty()
                    && !name.starts_with('_')
                    && name.chars().all(|c| c.is_alphanumeric() || c == '_')
                {
                    public_names.push(name);
                }
            }
        }
    }
    public_names
}

/// Return the 0-based line number *after* the last import statement.
fn last_import_line(source: &str) -> u32 {
    let mut insert_line: u32 = 0;
    for (idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("import ") || trimmed.starts_with("from ") {
            insert_line = u32::try_from(idx + 1).unwrap_or(u32::MAX);
        }
    }
    insert_line
}

// ── Shared helper ─────────────────────────────────────────────────────────────

/// Compute the LSP range covering the entire document.
pub(super) fn full_document_range(source: &str) -> Range {
    let line_count = u32::try_from(source.lines().count()).unwrap_or(u32::MAX);
    let last_line_char_count = source.lines().last().map_or(0, |l| l.chars().count());
    let last_line_len = u32::try_from(last_line_char_count).unwrap_or(u32::MAX);
    Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: line_count,
            character: last_line_len,
        },
    }
}
