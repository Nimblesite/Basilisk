//! Code Actions handler: quick fixes for diagnostics.

use std::sync::atomic::AtomicU64;

use tower_lsp::lsp_types::{CodeActionOrCommand, Diagnostic, NumberOrString, Url};

mod fixes;
mod imports;
mod suppress;

/// Monotonic counter for unique temp-file names.
pub(super) static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

// Re-export pub(crate) items that the server module calls directly.
pub(crate) use imports::organize_imports;

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
            "BSK-E0001" => Some(fixes::fix_missing_param_annotation(uri, diag)),
            "BSK-E0002" => Some(fixes::fix_missing_return_annotation(uri, diag)),
            "BSK-E0003" => Some(fixes::fix_missing_variable_annotation(uri, diag)),
            "BSK-W0050" => Some(fixes::fix_remove_redundant_annotation(uri, diag, source)),
            _ => None,
        };
        if let Some(a) = fix {
            actions.push(CodeActionOrCommand::CodeAction(a));
        }
        // Ergonomic suppression and severity override options for every diagnostic.
        actions.push(CodeActionOrCommand::CodeAction(
            suppress::suppress_with_code(uri, diag, source, code),
        ));
        actions.push(CodeActionOrCommand::CodeAction(
            suppress::demote_to_warning(uri, diag, source, code),
        ));
        actions.push(CodeActionOrCommand::CodeAction(suppress::disable_for_file(
            uri, diag, source, code,
        )));
        // Fallback: generic suppress-all on this line (PEP 484 compatible).
        actions.push(CodeActionOrCommand::CodeAction(
            suppress::suppress_with_type_ignore(uri, diag, source),
        ));
    }

    // Organize imports is always offered when there is source to organize.
    if !source.is_empty() {
        if let Some(action) = imports::organize_imports(uri, source) {
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
        if let Some(action) = imports::expand_wildcard_imports(uri, source) {
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
        if let Some(action) = imports::convert_import_style(uri, source) {
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
        if let Some(action) = imports::add_dunder_all(uri, source) {
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
    }

    actions
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test-only code: unwrap/expect acceptable in unit tests"
)]
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
        let action = fixes::fix_remove_redundant_annotation(&uri, &diag, source);
        assert_eq!(action.title, "Remove redundant type annotation (basilisk)");
        assert!(action.edit.is_some());
        let edit = action.edit.unwrap();
        let changes = edit.changes.unwrap();
        let text_edits = changes.get(&uri).unwrap();
        assert_eq!(text_edits.len(), 1);
        let text_edit = text_edits.first().expect("expected at least one text edit");
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
        assert!(actions.len() >= 2);
        let remove_action = actions.iter().find(|a| match a {
            CodeActionOrCommand::CodeAction(ca) => ca.title.contains("Remove redundant"),
            CodeActionOrCommand::Command(_) => false,
        });
        assert!(
            remove_action.is_some(),
            "Should have remove redundant action"
        );
    }
}
