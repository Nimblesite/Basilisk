//! Code Actions handler: quick fixes for diagnostics.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, Diagnostic, NumberOrString, Position, Range,
    TextEdit, Url, WorkspaceEdit,
};

/// Monotonic counter for unique temp-file names.
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate code actions for the given diagnostics.
///
/// `source` is the current document text; it is used to locate line ends
/// (for `# type: ignore`) and to run ruff (for organize imports).
#[must_use]
pub fn code_actions(
    uri: &Url,
    diagnostics: &[Diagnostic],
    source: &str,
) -> Vec<CodeActionOrCommand> {
    let mut actions = Vec::new();

    for diag in diagnostics {
        let Some(NumberOrString::String(code)) = &diag.code else {
            continue;
        };
        let fix = match code.as_str() {
            "BSK-E0001" => Some(fix_missing_param_annotation(uri, diag)),
            "BSK-E0002" => Some(fix_missing_return_annotation(uri, diag)),
            "BSK-E0003" => Some(fix_missing_variable_annotation(uri, diag)),
            _ => None,
        };
        if let Some(a) = fix {
            actions.push(CodeActionOrCommand::CodeAction(a));
        }
        // Every diagnostic also gets a suppress option.
        actions.push(CodeActionOrCommand::CodeAction(suppress_with_type_ignore(
            uri, diag, source,
        )));
    }

    // Organize imports is always offered when there is source to organize.
    if !source.is_empty() {
        if let Some(action) = organize_imports(uri, source) {
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
    }

    actions
}

// ── Per-diagnostic quick fixes ───────────────────────────────────────────────

/// Insert `: Any` after the parameter name.
fn fix_missing_param_annotation(uri: &Url, diag: &Diagnostic) -> CodeAction {
    single_insert(
        uri,
        diag,
        diag.range.end,
        ": Any",
        "Add `: Any` annotation (basilisk)",
    )
}

/// Insert `-> None ` before the colon/body.
fn fix_missing_return_annotation(uri: &Url, diag: &Diagnostic) -> CodeAction {
    single_insert(
        uri,
        diag,
        diag.range.start,
        "-> None ",
        "Add `-> None` return type (basilisk)",
    )
}

/// Insert `: <inferred_type>` after the variable name.
///
/// The annotation is derived from the diagnostic message:
/// - "empty list"  → `: list[Any]`
/// - "empty dict"  → `: dict[str, Any]`
/// - anything else → `: Any` (catches the `None` RHS case)
fn fix_missing_variable_annotation(uri: &Url, diag: &Diagnostic) -> CodeAction {
    let annotation = if diag.message.contains("empty list") {
        ": list[Any]"
    } else if diag.message.contains("empty dict") {
        ": dict[str, Any]"
    } else {
        ": Any"
    };
    single_insert(
        uri,
        diag,
        diag.range.end,
        annotation,
        &format!("Add `{annotation}` annotation (basilisk)"),
    )
}

// ── Suppress with `# type: ignore` ──────────────────────────────────────────

/// Append `  # type: ignore` at the end of the diagnostic's source line.
fn suppress_with_type_ignore(uri: &Url, diag: &Diagnostic, source: &str) -> CodeAction {
    let line_idx = diag.range.start.line as usize;
    #[allow(clippy::cast_possible_truncation)]
    let line_char_len = source
        .lines()
        .nth(line_idx)
        .map_or(0, |l| l.chars().count()) as u32;
    let insert_pos = Position { line: diag.range.start.line, character: line_char_len };

    let mut changes = HashMap::new();
    changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: Range { start: insert_pos, end: insert_pos },
            new_text: "  # type: ignore".to_owned(),
        }],
    );
    CodeAction {
        title: "Suppress with `# type: ignore` (basilisk)".to_owned(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit { changes: Some(changes), ..Default::default() }),
        is_preferred: Some(false),
        ..Default::default()
    }
}

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

    #[allow(clippy::cast_possible_truncation)]
    let line_count = source.lines().count() as u32;
    #[allow(clippy::cast_possible_truncation)]
    let last_line_len = source
        .lines()
        .last()
        .map_or(0, |l| l.chars().count()) as u32;
    let full_range = Range {
        start: Position { line: 0, character: 0 },
        end: Position { line: line_count, character: last_line_len },
    };

    let mut changes = HashMap::new();
    changes.insert(
        uri.clone(),
        vec![TextEdit { range: full_range, new_text: new_source }],
    );
    Some(CodeAction {
        title: "Organize imports (ruff)".to_owned(),
        kind: Some(CodeActionKind::SOURCE_ORGANIZE_IMPORTS),
        diagnostics: None,
        edit: Some(WorkspaceEdit { changes: Some(changes), ..Default::default() }),
        is_preferred: Some(true),
        ..Default::default()
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build a [`CodeAction`] that inserts `text` at `pos`.
fn single_insert(
    uri: &Url,
    diag: &Diagnostic,
    pos: Position,
    text: &str,
    title: &str,
) -> CodeAction {
    let mut changes = HashMap::new();
    changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: Range { start: pos, end: pos },
            new_text: text.to_owned(),
        }],
    );
    CodeAction {
        title: title.to_owned(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit { changes: Some(changes), ..Default::default() }),
        is_preferred: Some(true),
        ..Default::default()
    }
}
