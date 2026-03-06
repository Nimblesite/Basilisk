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
            "BSK-W0050" => Some(fix_remove_redundant_annotation(uri, diag, source)),
            _ => None,
        };
        if let Some(a) = fix {
            actions.push(CodeActionOrCommand::CodeAction(a));
        }
        // Ergonomic suppression and severity override options for every diagnostic.
        actions.push(CodeActionOrCommand::CodeAction(suppress_with_code(
            uri, diag, source, code,
        )));
        actions.push(CodeActionOrCommand::CodeAction(demote_to_warning(
            uri, diag, source, code,
        )));
        actions.push(CodeActionOrCommand::CodeAction(disable_for_file(
            uri, diag, source, code,
        )));
        // Fallback: generic suppress-all on this line (PEP 484 compatible).
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

/// Remove redundant type annotation (for BSK-W0050).
///
/// Example: `x: int = 42` → `x = 42`
/// Finds the colon on the diagnostic's source line and removes everything
/// from the colon to the equals sign (including the colon and space).
fn fix_remove_redundant_annotation(uri: &Url, diag: &Diagnostic, source: &str) -> CodeAction {
    let line_idx = diag.range.start.line as usize;

    // Find the colon after the variable name (within the diagnostic span).
    // The diagnostic span covers the variable name; we need to find the colon
    // that appears after it on the same line.
    let line_text = source.lines().nth(line_idx).unwrap_or("");
    let colon_pos = line_text.find(':');
    let equals_pos = line_text.find('=');

    let (range_to_remove, new_text) = match (colon_pos, equals_pos) {
        (Some(colon), Some(eq)) if colon < eq => {
            // Remove from colon up to (but not including) the equals sign.
            // Include any spaces between colon and equals.
            let start = Position {
                line: diag.range.start.line,
                character: u32::try_from(colon).unwrap_or(u32::MAX),
            };
            let end = Position {
                line: diag.range.start.line,
                character: u32::try_from(eq).unwrap_or(u32::MAX),
            };
            (Range { start, end }, String::new())
        }
        _ => {
            // Fallback: just remove the diagnostic range (the variable name)
            // This shouldn't happen for proper W0050 diagnostics.
            (diag.range, String::new())
        }
    };

    let mut changes = HashMap::new();
    changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: range_to_remove,
            new_text,
        }],
    );
    CodeAction {
        title: "Remove redundant type annotation (basilisk)".to_owned(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(true),
        ..Default::default()
    }
}

// ── Ergonomic suppression and severity overrides ─────────────────────────────

/// Append `  # type: ignore[CODE]` at the end of the diagnostic's source line.
fn suppress_with_code(uri: &Url, diag: &Diagnostic, source: &str, code: &str) -> CodeAction {
    let comment = format!("  # type: ignore[{code}]");
    let insert_pos = line_end_position(diag, source);
    let mut changes = HashMap::new();
    changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: Range {
                start: insert_pos,
                end: insert_pos,
            },
            new_text: comment,
        }],
    );
    CodeAction {
        title: format!("Ignore `{code}` on this line"),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(true),
        ..Default::default()
    }
}

/// Append `  # type: warning[CODE]` to demote the error to a warning.
fn demote_to_warning(uri: &Url, diag: &Diagnostic, source: &str, code: &str) -> CodeAction {
    let comment = format!("  # type: warning[{code}]");
    let insert_pos = line_end_position(diag, source);
    let mut changes = HashMap::new();
    changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: Range {
                start: insert_pos,
                end: insert_pos,
            },
            new_text: comment,
        }],
    );
    CodeAction {
        title: format!("Demote `{code}` to warning on this line"),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(false),
        ..Default::default()
    }
}

/// Insert `# basilisk: file-disabled[CODE]` at line 0 to disable for the whole file.
fn disable_for_file(uri: &Url, diag: &Diagnostic, _source: &str, code: &str) -> CodeAction {
    let comment = format!("# basilisk: file-disabled[{code}]\n");
    let insert_pos = Position {
        line: 0,
        character: 0,
    };
    let mut changes = HashMap::new();
    changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: Range {
                start: insert_pos,
                end: insert_pos,
            },
            new_text: comment,
        }],
    );
    CodeAction {
        title: format!("Disable `{code}` for this file"),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(false),
        ..Default::default()
    }
}

/// Get the end-of-line position for a diagnostic's line.
fn line_end_position(diag: &Diagnostic, source: &str) -> Position {
    let line_idx = diag.range.start.line as usize;
    #[allow(clippy::cast_possible_truncation)]
    let line_char_len = source
        .lines()
        .nth(line_idx)
        .map_or(0, |l| l.chars().count()) as u32;
    Position {
        line: diag.range.start.line,
        character: line_char_len,
    }
}

// ── Suppress with `# type: ignore` (generic fallback) ──────────────────────

/// Append `  # type: ignore` at the end of the diagnostic's source line.
fn suppress_with_type_ignore(uri: &Url, diag: &Diagnostic, source: &str) -> CodeAction {
    let line_idx = diag.range.start.line as usize;
    #[allow(clippy::cast_possible_truncation)]
    let line_char_len = source
        .lines()
        .nth(line_idx)
        .map_or(0, |l| l.chars().count()) as u32;
    let insert_pos = Position {
        line: diag.range.start.line,
        character: line_char_len,
    };

    let mut changes = HashMap::new();
    changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: Range {
                start: insert_pos,
                end: insert_pos,
            },
            new_text: "  # type: ignore".to_owned(),
        }],
    );
    CodeAction {
        title: "Suppress with `# type: ignore` (basilisk)".to_owned(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
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
    let last_line_len = source.lines().last().map_or(0, |l| l.chars().count()) as u32;
    let full_range = Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: line_count,
            character: last_line_len,
        },
    };

    let mut changes = HashMap::new();
    changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: full_range,
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
            range: Range {
                start: pos,
                end: pos,
            },
            new_text: text.to_owned(),
        }],
    );
    CodeAction {
        title: title.to_owned(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(true),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::{Diagnostic, NumberOrString, Position, Range, Url};

    #[test]
    fn test_fix_remove_redundant_annotation() {
        let uri = Url::parse("file:///test.py").unwrap();
        let diag = Diagnostic {
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 1,
                },
            },
            severity: Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String("BSK-W0050".to_owned())),
            code_description: None,
            source: Some("basilisk".to_owned()),
            message: "Redundant type annotation".to_owned(),
            tags: None,
            related_information: None,
            data: None,
        };
        let source = "x: int = 42\n";
        let action = super::fix_remove_redundant_annotation(&uri, &diag, source);
        assert_eq!(action.title, "Remove redundant type annotation (basilisk)");
        assert!(action.edit.is_some());
        let edit = action.edit.unwrap();
        let changes = edit.changes.unwrap();
        let text_edits = changes.get(&uri).unwrap();
        assert_eq!(text_edits.len(), 1);
        let text_edit = &text_edits[0];
        assert_eq!(text_edit.new_text, "");
        assert_eq!(text_edit.range.start.line, 0);
        assert_eq!(text_edit.range.start.character, 1); // colon position
        assert_eq!(text_edit.range.end.line, 0);
        assert_eq!(text_edit.range.end.character, 7); // equals position
    }

    #[test]
    fn test_code_actions_includes_w0050() {
        let uri = Url::parse("file:///test.py").unwrap();
        let diag = Diagnostic {
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 1,
                },
            },
            severity: Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String("BSK-W0050".to_owned())),
            code_description: None,
            source: Some("basilisk".to_owned()),
            message: "Redundant type annotation".to_owned(),
            tags: None,
            related_information: None,
            data: None,
        };
        let source = "x: int = 42\n";
        let actions = super::code_actions(&uri, &[diag], source);
        // Should have at least two actions: remove redundant and suppress
        assert!(actions.len() >= 2);
        let remove_action = actions.iter().find(|a| match a {
            CodeActionOrCommand::CodeAction(ca) => ca.title.contains("Remove redundant"),
            _ => false,
        });
        assert!(
            remove_action.is_some(),
            "Should have remove redundant action"
        );
    }
}
