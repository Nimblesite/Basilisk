//! Code Actions handler: quick fixes for diagnostics.

use std::collections::HashMap;

use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, Diagnostic, NumberOrString, Range, TextEdit,
    Url, WorkspaceEdit,
};

/// Generate code actions for the given diagnostics.
#[must_use]
pub fn code_actions(uri: &Url, diagnostics: &[Diagnostic]) -> Vec<CodeActionOrCommand> {
    let mut actions = Vec::new();
    for diag in diagnostics {
        let Some(NumberOrString::String(code)) = &diag.code else {
            continue;
        };
        let action = match code.as_str() {
            "BSK-E0001" => Some(fix_missing_param_annotation(uri, diag)),
            "BSK-E0002" => Some(fix_missing_return_annotation(uri, diag)),
            _ => None,
        };
        if let Some(a) = action {
            actions.push(CodeActionOrCommand::CodeAction(a));
        }
    }
    actions
}

/// Insert `: Any` after the parameter name.
fn fix_missing_param_annotation(uri: &Url, diag: &Diagnostic) -> CodeAction {
    let insert_pos = diag.range.end;
    let mut changes = HashMap::new();
    changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: Range { start: insert_pos, end: insert_pos },
            new_text: ": Any".to_owned(),
        }],
    );
    CodeAction {
        title: "Add `: Any` annotation (basilisk)".to_owned(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit { changes: Some(changes), ..Default::default() }),
        is_preferred: Some(true),
        ..Default::default()
    }
}

/// Insert `-> None ` before the colon.
fn fix_missing_return_annotation(uri: &Url, diag: &Diagnostic) -> CodeAction {
    let insert_pos = diag.range.start;
    let mut changes = HashMap::new();
    changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: Range { start: insert_pos, end: insert_pos },
            new_text: "-> None ".to_owned(),
        }],
    );
    CodeAction {
        title: "Add `-> None` return type (basilisk)".to_owned(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit { changes: Some(changes), ..Default::default() }),
        is_preferred: Some(true),
        ..Default::default()
    }
}
