//! Implements [LSPARCH-FEATURES-CODEACTIONS]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-CODEACTIONS
//!
//! Code Actions handler: quick fixes for diagnostics.

use std::collections::HashMap;

use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, Command, Diagnostic, NumberOrString, Position,
    Range, TextEdit, Url, WorkspaceEdit,
};

mod fixes;
mod imports;
pub mod mass_fix;
pub(crate) mod refactor;
mod stubs;
mod suppress;

/// Compute the LSP range covering the entire document.
pub(crate) fn full_document_range(source: &str) -> Range {
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

/// Return the 0-based line number after the last import statement.
pub(super) fn last_import_line(source: &str) -> u32 {
    let mut insert_line: u32 = 0;
    for (idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("import ") || trimmed.starts_with("from ") {
            insert_line = u32::try_from(idx + 1).unwrap_or(u32::MAX);
        }
    }
    insert_line
}

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

/// Build a `QUICKFIX` [`CodeAction`] attached to `diag` that applies `changes`.
///
/// Like [`code_action_with_changes`] but always `QUICKFIX` kind and carries the
/// originating diagnostic — the shape every fix/suppression action shares.
pub(super) fn quickfix_action(
    title: String,
    diag: &Diagnostic,
    changes: HashMap<Url, Vec<TextEdit>>,
    is_preferred: bool,
) -> CodeAction {
    CodeAction {
        title,
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
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
pub(crate) use mass_fix::{fix_all_by_rule, fix_all_quickfix, fix_filtered_in_file};

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
            "BSK-0001" => Some(fixes::fix_missing_param_annotation(uri, diag)),
            "BSK-0002" => Some(fixes::fix_missing_return_annotation(uri, diag)),
            "BSK-0003" => Some(fixes::fix_missing_variable_annotation(uri, diag)),
            "BSK-0050" => Some(fixes::fix_remove_redundant_annotation(uri, diag, source)),
            "BSK-0005" => Some(fixes::fix_missing_attribute_annotation(uri, diag)),
            _ => None,
        };

        // uv-based quick fixes for unresolved imports and missing stubs.
        if code == "imports_unresolved" {
            if let Some(module) = extract_module_from_diagnostic(&diag.message) {
                // Only offer `uv add` when the name is a plausible PyPI
                // distribution. Internal/vendored modules like `_pydevd_bundle`
                // are not installable and uv rejects them outright, so the fix
                // could only ever fail. [LSPUV-ACTIONS-QUICK-FIXES], issue #84.
                if is_valid_pypi_distribution(&module) {
                    actions.push(CodeActionOrCommand::CodeAction(make_uv_add_action(
                        diag, &module,
                    )));
                }
            }
        }
        if code == "BSK-0152" {
            if let Some(module) = extract_module_from_diagnostic(&diag.message) {
                // Install a published typeshed stub when one exists...
                let install = make_uv_add_stubs_action(diag, &module);
                // ...and always offer a local stub: the only fix when typeshed
                // has nothing, a valid fallback when it does. [STUBRES-CREATE-LOCAL]
                let local_preferred = install.is_none();
                if let Some(action) = install {
                    actions.push(CodeActionOrCommand::CodeAction(action));
                }
                actions.push(CodeActionOrCommand::CodeAction(
                    stubs::make_create_local_stub_action(diag, &module, local_preferred),
                ));
            }
        }
        if code == "imports_module_attribute" {
            // "Create method/attribute" quick fix — add the undeclared member
            // to the local stub. [STUBRES-ADD-MEMBER]
            if let Some(action) = stubs::make_add_member_action(diag, source) {
                actions.push(CodeActionOrCommand::CodeAction(action));
            }
        }
        if code == "BSK-0013" {
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
        // Offer to disable the rule in the active project configuration.
        actions.push(CodeActionOrCommand::CodeAction(
            suppress::disable_in_project_config(uri, diag, code),
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

/// Return `true` when `name` is a plausible `PyPI` distribution name.
///
/// Per PEP 508/503 a distribution name consists of ASCII letters, digits, `.`,
/// `-`, or `_`, and must start and end with an alphanumeric character. `uv`
/// enforces the leading-alphanumeric rule and rejects names such as
/// `_pydevd_bundle` (a debugpy-internal submodule), so we must not offer a
/// `uv add` quick-fix that can only fail (issue #84).
fn is_valid_pypi_distribution(name: &str) -> bool {
    let bytes = name.as_bytes();
    let (Some(first), Some(last)) = (bytes.first(), bytes.last()) else {
        return false;
    };
    first.is_ascii_alphanumeric()
        && last.is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
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

/// Build a code action that runs `uv add --dev types-<dist>` for missing type
/// stubs — but only when typeshed publishes a stub distribution for `module`.
///
/// Returns `None` when no stub distribution is known (the package ships inline
/// `py.typed` types or no stubs exist), so we never offer a quick-fix that would
/// fail to resolve on `PyPI` (e.g. the nonexistent `pydantic_ai-stubs`).
///
/// Implements [LSPUV-DIAGNOSTICS-MISSING-STUBS] — the `basilisk.uv.addDev`
/// quick fix, gated on the bundled typeshed index.
fn make_uv_add_stubs_action(diag: &Diagnostic, module: &str) -> Option<CodeAction> {
    let stubs_package = basilisk_stubs::typeshed_stub_distribution(module)?;
    Some(CodeAction {
        title: format!("Install type stubs for '{module}' (uv add --dev)"),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        command: Some(Command {
            title: format!("uv add --dev {stubs_package}"),
            command: basilisk_common::commands::UV_ADD_DEV.to_owned(),
            arguments: Some(vec![serde_json::Value::String(stubs_package.to_owned())]),
        }),
        ..CodeAction::default()
    })
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test-only code: unwrap/expect/indexing/panic acceptable in unit tests"
)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::{
        Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range, Url,
    };

    fn pos(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    fn range_at(start: (u32, u32), end: (u32, u32)) -> Range {
        Range {
            start: pos(start.0, start.1),
            end: pos(end.0, end.1),
        }
    }

    fn make_diagnostic(
        severity: DiagnosticSeverity,
        code: &str,
        message: &str,
        range: Range,
    ) -> Diagnostic {
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
            "BSK-0050",
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
            "BSK-0050",
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

    /// Assert a create-local-stub action is present for `module`, dispatches the
    /// `STUBS_CREATE_LOCAL` command with the module arg, and has the expected
    /// `is_preferred` flag.
    fn assert_create_local_stub_action(
        actions: &[CodeActionOrCommand],
        module: &str,
        expected_preferred: bool,
    ) {
        let action = find_action_with_title(actions, "Create local type stub")
            .expect("should offer a create-local-stub action");
        let CodeActionOrCommand::CodeAction(ca) = action else {
            panic!("Expected a CodeAction, got a Command");
        };
        let cmd = ca.command.as_ref().expect("should have command");
        assert_eq!(cmd.command, basilisk_common::commands::STUBS_CREATE_LOCAL);
        let args = cmd.arguments.as_ref().expect("should have arguments");
        assert_eq!(args[0], serde_json::Value::String(module.to_owned()));
        assert_eq!(ca.is_preferred, Some(expected_preferred));
    }

    #[test]
    fn test_bsk_e0010_code_action_includes_uv_add() {
        let diag = make_diagnostic(
            DiagnosticSeverity::ERROR,
            "imports_unresolved",
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

    /// Regression for issue #84: the `imports_unresolved` "add dependency" quick-fix must
    /// NOT offer `uv add` for a module name that is not a valid `PyPI`
    /// distribution. `_pydevd_bundle` is a debugpy-internal submodule whose
    /// name starts with `_`; `uv` rejects it ("Expected package name starting
    /// with an alphanumeric character"), so offering the fix proposes an action
    /// that can only fail.
    #[test]
    fn test_bsk_e0010_no_uv_add_for_non_pypi_module() {
        let diag = make_diagnostic(
            DiagnosticSeverity::ERROR,
            "imports_unresolved",
            "Cannot resolve import `_pydevd_bundle`",
            range_at((0, 0), (0, 20)),
        );
        let uri = Url::parse("file:///test.py").unwrap();
        let source = "import _pydevd_bundle\n";
        let range = range_at((0, 0), (0, 0));
        let actions = super::code_actions(&uri, &[diag], source, &range, None);
        assert!(
            find_action_with_title(&actions, "dependency (uv add)").is_none(),
            "must not offer `uv add` for a name that is not a valid PyPI distribution"
        );
    }

    /// Regression for issue #46: typeshed publishes stubs as `types-<dist>`,
    /// not `<module>-stubs`. For `requests` the real package is
    /// `types-requests`; offering `requests-stubs` fails to resolve on `PyPI`.
    #[test]
    fn test_bsk_e0152_code_action_includes_uv_add_dev() {
        let diag = make_diagnostic(
            DiagnosticSeverity::ERROR,
            "BSK-0152",
            "Package `requests` is installed but has no type stubs available",
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
            "types-requests",
        );
    }

    /// Regression for issue #46: `make_uv_add_stubs_action` must NOT offer a
    /// quick-fix when no stub package is known for the module. `pydantic_ai`
    /// ships inline `py.typed` and has no typeshed stub — suggesting
    /// `pydantic_ai-stubs` (or any name) leads to a broken `uv add` that 404s.
    ///
    /// But the user must never be left with *no* fix ([STUBRES-CODEACTIONS]):
    /// the create-local-stub action takes over as the preferred quick fix.
    #[test]
    fn test_bsk_e0152_action_suppressed_for_unknown_stub() {
        let diag = make_diagnostic(
            DiagnosticSeverity::ERROR,
            "BSK-0152",
            "Package `pydantic_ai` is installed but has no type stubs available",
            range_at((0, 0), (0, 12)),
        );
        let uri = Url::parse("file:///test.py").unwrap();
        let source = "import pydantic_ai\n";
        let range = range_at((0, 0), (0, 0));
        let actions = super::code_actions(&uri, &[diag], source, &range, None);
        assert!(
            find_action_with_title(&actions, "Install type stubs").is_none(),
            "must not offer a stub-install quick-fix for a package with no known stub package"
        );
        assert_create_local_stub_action(&actions, "pydantic_ai", true);
    }

    /// Even when typeshed publishes stubs, the create-local action is offered as
    /// a fallback alongside the install fix — so both paths are one click away.
    #[test]
    fn test_bsk_e0152_offers_local_stub_alongside_install_for_typeshed() {
        let diag = make_diagnostic(
            DiagnosticSeverity::ERROR,
            "BSK-0152",
            "Package `requests` is installed but has no type stubs available",
            range_at((0, 0), (0, 8)),
        );
        let uri = Url::parse("file:///test.py").unwrap();
        let source = "import requests\n";
        let range = range_at((0, 0), (0, 0));
        let actions = super::code_actions(&uri, &[diag], source, &range, None);
        assert!(
            find_action_with_title(&actions, "Install type stubs").is_some(),
            "typeshed-backed package must still offer the install quick-fix"
        );
        // Not preferred here — install is the highlighted default.
        assert_create_local_stub_action(&actions, "requests", false);
    }

    /// `imports_module_attribute` offers a one-click "add member to stub" fix that dispatches the
    /// `STUBS_ADD_MEMBER` command with the stub path and the inferred snippet.
    #[test]
    fn test_bsk_e0154_offers_add_member_action() {
        let diag = make_diagnostic(
            DiagnosticSeverity::ERROR,
            "imports_module_attribute",
            "Module `cowsay` has no attribute `get_output_string`\n\nhelp: declare \
             `get_output_string` in the local stub `/w/.basilisk/stubs/cowsay.pyi`, or fix the typo.",
            range_at((1, 4), (1, 28)),
        );
        let uri = Url::parse("file:///test.py").unwrap();
        let source = "import cowsay\nx = cowsay.get_output_string(\"c\", m)\n";
        let range = range_at((0, 0), (0, 0));
        let actions = super::code_actions(&uri, &[diag], source, &range, None);
        let action = find_action_with_title(&actions, "Add method")
            .expect("E0154 must offer an add-member action");
        let CodeActionOrCommand::CodeAction(ca) = action else {
            panic!("expected a CodeAction");
        };
        let cmd = ca.command.as_ref().expect("should have a command");
        assert_eq!(cmd.command, basilisk_common::commands::STUBS_ADD_MEMBER);
        let args = cmd.arguments.as_ref().unwrap();
        assert_eq!(args[0].as_str().unwrap(), "/w/.basilisk/stubs/cowsay.pyi");
        assert_eq!(
            args[1].as_str().unwrap(),
            "def get_output_string(arg0: Any, arg1: Any) -> Any: ..."
        );
    }

    #[test]
    fn pytest_not_found_code_action_includes_uv_add_dev_pytest() {
        let diag = make_diagnostic(
            DiagnosticSeverity::WARNING,
            crate::server::test_handlers::PYTEST_NOT_FOUND_CODE,
            "Test runner \"pytest\" is not installed. Run \"uv add --dev pytest\" to install.",
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
