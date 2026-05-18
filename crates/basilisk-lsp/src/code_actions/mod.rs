//! Code Actions handler: quick fixes for diagnostics.

use std::collections::HashMap;
use std::sync::atomic::AtomicU64;

use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, Command, Diagnostic, NumberOrString, Range,
    TextEdit, Url, WorkspaceEdit,
};

mod fixes;
mod imports;
pub mod mass_fix;
pub(crate) mod refactor;
mod suppress;

/// Monotonic counter for unique temp-file names.
pub(super) static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Construct a [`CodeAction`] whose edit is a precomputed `changes` map.
///
/// Many code-action builders construct a `HashMap<Url, Vec<TextEdit>>` and
/// wrap it in `WorkspaceEdit { changes: Some(changes), ..Default::default() }`.
/// This helper deduplicates that boilerplate for actions that carry no
/// associated diagnostic (`diagnostics: None`).
pub(super) fn code_action_with_changes(
    title: String,
    kind: CodeActionKind,
    changes: HashMap<Url, Vec<TextEdit>>,
    is_preferred: bool,
) -> CodeAction {
    CodeAction {
        title,
        kind: Some(kind),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(is_preferred),
        ..Default::default()
    }
}

// Re-export pub(crate) items that the server module calls directly.
pub(crate) use imports::organize_imports;
pub(crate) use mass_fix::{fix_all_by_rule, fix_all_in_file, fix_all_quickfix};

/// Generate code actions for the given diagnostics and cursor range.
///
/// `source` is the current document text; it is used to locate line ends
/// (for `# type: ignore`) and to run ruff (for organize imports).
/// `range` is the cursor/selection range from the LSP request.
/// `resolved` is the resolved module, if available (used for abstract methods).
#[must_use]
pub fn code_actions(
    uri: &Url,
    diagnostics: &[Diagnostic],
    source: &str,
    range: &Range,
    resolved: Option<&basilisk_resolver::ResolvedModule>,
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
            "BSK-E0005" => Some(fixes::fix_missing_attribute_annotation(uri, diag)),
            _ => None,
        };

        // uv-based quick fixes for unresolved imports and missing stubs.
        if code == "BSK-E0010" {
            if let Some(module) = extract_module_from_diagnostic(&diag.message) {
                actions.push(CodeActionOrCommand::CodeAction(make_uv_add_action(
                    diag, &module,
                )));
            }
        }
        if code == "BSK-W0010" {
            if let Some(module) = extract_module_from_diagnostic(&diag.message) {
                actions.push(CodeActionOrCommand::CodeAction(make_uv_add_stubs_action(
                    diag, &module,
                )));
            }
        }
        if code == "BSK-W0013" {
            actions.push(CodeActionOrCommand::CodeAction(make_uv_sync_action(diag)));
        }
        if code == crate::server::test_handlers::PYTEST_NOT_FOUND_CODE {
            actions.push(CodeActionOrCommand::CodeAction(
                make_uv_add_dev_pytest_action(diag),
            ));
        }
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
        // Offer to disable the rule in pyproject.toml project config.
        actions.push(CodeActionOrCommand::CodeAction(
            suppress::disable_in_project_config(diag, code),
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

    collect_refactoring_actions(uri, source, range, resolved, &mut actions);

    actions
}

/// Collect refactoring code actions (extract, convert, inline, implement).
fn collect_refactoring_actions(
    uri: &Url,
    source: &str,
    range: &Range,
    resolved: Option<&basilisk_resolver::ResolvedModule>,
    actions: &mut Vec<CodeActionOrCommand>,
) {
    for action in refactor::extract_variable(uri, source, range) {
        actions.push(CodeActionOrCommand::CodeAction(action));
    }
    if let Some(action) = refactor::extract_constant(uri, source, range) {
        actions.push(CodeActionOrCommand::CodeAction(action));
    }
    if let Some(action) = refactor::extract_function(uri, source, range) {
        actions.push(CodeActionOrCommand::CodeAction(action));
    }
    if let Some(action) = refactor::inline_variable(uri, source, range) {
        actions.push(CodeActionOrCommand::CodeAction(action));
    }
    for action in refactor::convert_union_syntax(uri, source, range) {
        actions.push(CodeActionOrCommand::CodeAction(action));
    }
    for action in refactor::convert_optional_syntax(uri, source, range) {
        actions.push(CodeActionOrCommand::CodeAction(action));
    }
    for action in refactor::convert_fstring(uri, source, range) {
        actions.push(CodeActionOrCommand::CodeAction(action));
    }
    for action in refactor::convert_literals(uri, source, range) {
        actions.push(CodeActionOrCommand::CodeAction(action));
    }
    for action in refactor::convert_ternary(uri, source, range) {
        actions.push(CodeActionOrCommand::CodeAction(action));
    }
    for action in refactor::convert_namedtuple(uri, source, range) {
        actions.push(CodeActionOrCommand::CodeAction(action));
    }
    if let Some(action) = refactor::inline_function_call(uri, source, range) {
        actions.push(CodeActionOrCommand::CodeAction(action));
    }
    if let Some(action) = refactor::move_symbol_to_new_file(uri, source, range) {
        actions.push(CodeActionOrCommand::CodeAction(action));
    }
    if let Some(action) = refactor::move_symbol_to_existing_file(uri, source, range) {
        actions.push(CodeActionOrCommand::CodeAction(action));
    }
    if let Some(resolved) = resolved {
        let position_offset = crate::util::position_to_byte_offset(source, range.start);
        if let Some(action) =
            refactor::implement_abstract_methods(uri, source, resolved, position_offset)
        {
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
        if let Some(action) =
            refactor::remove_parameter(uri, source, range, resolved, position_offset)
        {
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
        if let Some(action) = refactor::add_parameter(uri, source, range, resolved, position_offset)
        {
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
        if let Some(action) =
            refactor::reorder_parameters(uri, source, range, resolved, position_offset)
        {
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
    }
}

/// Extract the top-level module name from a diagnostic message.
///
/// Looks for a quoted identifier (backticks, single or double quotes) and
/// returns the first dotted component, e.g. `` `foo.bar` `` yields `"foo"`.
fn extract_module_from_diagnostic(message: &str) -> Option<String> {
    // Find content between quotes: `module`, 'module', or "module"
    let start = message
        .find('`')
        .or_else(|| message.find('\''))
        .or_else(|| message.find('"'))?;
    let quote_char = char::from(*message.as_bytes().get(start)?);
    let after_quote = message.get(start + 1..)?;
    let end = after_quote.find(quote_char)?;
    let full_module = after_quote.get(..end)?;

    // Take only the top-level package (before the first dot).
    let top_level = full_module.split('.').next()?;
    if top_level.is_empty() {
        return None;
    }
    Some(top_level.to_owned())
}

/// Build a code action that runs `uv add <package>` for an unresolved import.
fn make_uv_add_action(diag: &Diagnostic, module: &str) -> CodeAction {
    CodeAction {
        title: format!("Add '{module}' dependency (uv add)"),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        command: Some(Command {
            title: format!("uv add {module}"),
            command: basilisk_common::commands::UV_ADD.to_owned(),
            arguments: Some(vec![serde_json::Value::String(module.to_owned())]),
        }),
        ..CodeAction::default()
    }
}

/// Build a code action that runs `uv sync` for a stale lock file.
fn make_uv_sync_action(diag: &Diagnostic) -> CodeAction {
    CodeAction {
        title: "Sync environment (uv sync)".to_owned(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        command: Some(Command {
            title: "uv sync".to_owned(),
            command: basilisk_common::commands::UV_SYNC.to_owned(),
            arguments: None,
        }),
        ..CodeAction::default()
    }
}

/// Build a code action that runs `uv add --dev pytest` when pytest is missing.
fn make_uv_add_dev_pytest_action(diag: &Diagnostic) -> CodeAction {
    CodeAction {
        title: "Install pytest (uv add --dev pytest)".to_owned(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        command: Some(Command {
            title: "uv add --dev pytest".to_owned(),
            command: basilisk_common::commands::UV_ADD_DEV.to_owned(),
            arguments: Some(vec![serde_json::Value::String("pytest".to_owned())]),
        }),
        is_preferred: Some(true),
        ..CodeAction::default()
    }
}

/// Build a code action that runs `uv add --dev <package>-stubs` for missing type stubs.
fn make_uv_add_stubs_action(diag: &Diagnostic, module: &str) -> CodeAction {
    let stubs_package = format!("{module}-stubs");
    CodeAction {
        title: format!("Install type stubs for '{module}' (uv add --dev)"),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        command: Some(Command {
            title: format!("uv add --dev {stubs_package}"),
            command: basilisk_common::commands::UV_ADD_DEV.to_owned(),
            arguments: Some(vec![serde_json::Value::String(stubs_package)]),
        }),
        ..CodeAction::default()
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test-only code: unwrap/expect/indexing acceptable in unit tests"
)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range, Url};

    fn pos(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    fn range_at(start: (u32, u32), end: (u32, u32)) -> Range {
        Range {
            start: pos(start.0, start.1),
            end: pos(end.0, end.1),
        }
    }

    fn make_diagnostic(severity: DiagnosticSeverity, code: &str, message: &str, range: Range) -> Diagnostic {
        Diagnostic {
            range,
            severity: Some(severity),
            code: Some(NumberOrString::String(code.to_owned())),
            code_description: None,
            source: Some("basilisk".to_owned()),
            message: message.to_owned(),
            tags: None,
            related_information: None,
            data: None,
        }
    }

    fn find_action_with_title<'a>(
        actions: &'a [CodeActionOrCommand],
        substring: &str,
    ) -> Option<&'a CodeActionOrCommand> {
        actions.iter().find(|a| match a {
            CodeActionOrCommand::CodeAction(ca) => ca.title.contains(substring),
            CodeActionOrCommand::Command(_) => false,
        })
    }

    #[test]
    fn test_fix_remove_redundant_annotation() {
        let uri = Url::parse("file:///test.py").unwrap();
        let diag = make_diagnostic(
            DiagnosticSeverity::WARNING,
            "BSK-W0050",
            "Redundant type annotation",
            range_at((0, 0), (0, 1)),
        );
        let source = "x: int = 42\n";
        let action = fixes::fix_remove_redundant_annotation(&uri, &diag, source);
        assert_eq!(action.title, "Remove redundant type annotation (basilisk)");
        assert!(action.edit.is_some());
        let edit = action.edit.unwrap();
        let changes = edit.changes.unwrap();
        let text_edits = changes.get(&uri).unwrap();
        assert_eq!(text_edits.len(), 1);
        let text_edit = text_edits.first().expect("expected at least one text edit");
        assert_eq!(text_edit.new_text, " ");
        assert_eq!(text_edit.range.start.line, 0);
        assert_eq!(text_edit.range.start.character, 1); // colon position
        assert_eq!(text_edit.range.end.line, 0);
        assert_eq!(text_edit.range.end.character, 7); // equals position — replacement " " yields `x = 42`
    }

    #[test]
    fn test_code_actions_includes_w0050() {
        let uri = Url::parse("file:///test.py").unwrap();
        let diag = make_diagnostic(
            DiagnosticSeverity::WARNING,
            "BSK-W0050",
            "Redundant type annotation",
            range_at((0, 0), (0, 1)),
        );
        let source = "x: int = 42\n";
        let range = range_at((0, 0), (0, 0));
        let actions = super::code_actions(&uri, &[diag], source, &range, None);
        assert!(actions.len() >= 2);
        assert!(
            find_action_with_title(&actions, "Remove redundant").is_some(),
            "Should have remove redundant action"
        );
    }

    #[test]
    fn test_extract_module_from_diagnostic_single_quotes() {
        let msg = "Cannot resolve import 'requests.auth'";
        assert_eq!(
            extract_module_from_diagnostic(msg),
            Some("requests".to_owned())
        );
    }

    #[test]
    fn test_extract_module_from_diagnostic_double_quotes() {
        let msg = "Cannot resolve import \"flask\"";
        assert_eq!(
            extract_module_from_diagnostic(msg),
            Some("flask".to_owned())
        );
    }

    #[test]
    fn test_extract_module_from_diagnostic_backticks() {
        let msg = "Cannot resolve import `six` \u{2014} no type information available";
        assert_eq!(extract_module_from_diagnostic(msg), Some("six".to_owned()));
    }

    #[test]
    fn test_extract_module_from_diagnostic_backticks_dotted() {
        let msg = "Cannot resolve import `agent_backend.db.session` \u{2014} no type information available";
        assert_eq!(
            extract_module_from_diagnostic(msg),
            Some("agent_backend".to_owned())
        );
    }

    #[test]
    fn test_extract_module_from_diagnostic_no_quotes() {
        let msg = "Something went wrong";
        assert_eq!(extract_module_from_diagnostic(msg), None);
    }

    fn assert_uv_add_action(
        actions: &[CodeActionOrCommand],
        title_substring: &str,
        expected_title: &str,
        expected_command: &str,
        expected_arg: &str,
    ) {
        let action = find_action_with_title(actions, title_substring)
            .unwrap_or_else(|| panic!("Should have action matching {title_substring:?}"));
        let CodeActionOrCommand::CodeAction(ca) = action else {
            panic!("Expected a CodeAction, got a Command");
        };
        assert_eq!(ca.title, expected_title);
        let cmd = ca.command.as_ref().expect("should have command");
        assert_eq!(cmd.command, expected_command);
        let args = cmd.arguments.as_ref().expect("should have arguments");
        assert_eq!(args[0], serde_json::Value::String(expected_arg.to_owned()));
    }

    #[test]
    fn test_bsk_e0010_code_action_includes_uv_add() {
        let diag = make_diagnostic(
            DiagnosticSeverity::ERROR,
            "BSK-E0010",
            "Cannot resolve import 'requests'",
            range_at((0, 0), (0, 8)),
        );
        let uri = Url::parse("file:///test.py").unwrap();
        let source = "import requests\n";
        let range = range_at((0, 0), (0, 0));
        let actions = super::code_actions(&uri, &[diag], source, &range, None);
        assert_uv_add_action(
            &actions,
            "uv add",
            "Add 'requests' dependency (uv add)",
            basilisk_common::commands::UV_ADD,
            "requests",
        );
    }

    #[test]
    fn test_bsk_w0010_code_action_includes_uv_add_dev() {
        let diag = make_diagnostic(
            DiagnosticSeverity::WARNING,
            "BSK-W0010",
            "Missing type stubs for 'requests'",
            range_at((0, 0), (0, 8)),
        );
        let uri = Url::parse("file:///test.py").unwrap();
        let source = "import requests\n";
        let range = range_at((0, 0), (0, 0));
        let actions = super::code_actions(&uri, &[diag], source, &range, None);
        assert_uv_add_action(
            &actions,
            "uv add --dev",
            "Install type stubs for 'requests' (uv add --dev)",
            basilisk_common::commands::UV_ADD_DEV,
            "requests-stubs",
        );
    }

    #[test]
    fn test_bsk_w0014_code_action_includes_uv_add_dev_pytest() {
        let diag = make_diagnostic(
            DiagnosticSeverity::WARNING,
            crate::server::test_handlers::PYTEST_NOT_FOUND_CODE,
            "pytest not found in uv.lock — use quick fix to install",
            range_at((0, 0), (0, 0)),
        );
        let uri = Url::parse("file:///test_example.py").unwrap();
        let source = "def test_hello() -> None:\n    pass\n";
        let range = range_at((0, 0), (0, 0));
        let actions = super::code_actions(&uri, &[diag], source, &range, None);
        assert_uv_add_action(
            &actions,
            "Install pytest",
            "Install pytest (uv add --dev pytest)",
            basilisk_common::commands::UV_ADD_DEV,
            "pytest",
        );
    }
}
