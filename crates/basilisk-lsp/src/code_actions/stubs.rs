//! Implements [STUBRES-CREATE-LOCAL]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#stubres-create-local
//!
//! Quick fix for BSK-E0152 (missing type stubs): scaffold a local `.pyi` stub.
//!
//! Unlike the typeshed quick fix (`make_uv_add_stubs_action`), this action is
//! offered for *every* untyped package — including those with no published
//! stub distribution, which previously had no fix at all. The action dispatches
//! the [`basilisk_common::commands::STUBS_CREATE_LOCAL`] command; the server
//! handler writes the skeleton and re-resolves so the diagnostic clears.

use tower_lsp::lsp_types::{CodeAction, CodeActionKind, Command, Diagnostic};

/// Build a code action that scaffolds a local stub for `module`.
///
/// `is_preferred` marks this as the highlighted fix — set when no typeshed stub
/// exists, so the user's only fix is the lightbulb default.
pub(super) fn make_create_local_stub_action(
    diag: &Diagnostic,
    module: &str,
    is_preferred: bool,
) -> CodeAction {
    CodeAction {
        title: format!("Create local type stub for '{module}' (.basilisk/stubs/{module}.pyi)"),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        command: Some(Command {
            title: format!("Create {module}.pyi"),
            command: basilisk_common::commands::STUBS_CREATE_LOCAL.to_owned(),
            arguments: Some(vec![serde_json::Value::String(module.to_owned())]),
        }),
        is_preferred: Some(is_preferred),
        ..CodeAction::default()
    }
}

/// Build the "Add member to stub" quick fix for a `imports_module_attribute` diagnostic
/// (a `module.attr` access the local stub does not declare).
///
/// Parses the module/attribute and stub path out of the diagnostic message,
/// inspects how the attribute is used at the source site (a call → a method
/// stub with parameters inferred from the call arguments; a plain access → an
/// attribute), and dispatches [`basilisk_common::commands::STUBS_ADD_MEMBER`]
/// with the stub path and the snippet line to append. Implements [STUBRES-ADD-MEMBER].
pub(super) fn make_add_member_action(diag: &Diagnostic, source: &str) -> Option<CodeAction> {
    let member = extract_backticked_after(&diag.message, "has no attribute `")?;
    let stub_path = extract_backticked_after(&diag.message, "the local stub `")?;

    let usage = analyze_usage(source, diag.range.end);
    let (snippet, label) = match usage {
        Usage::Call(params) => (
            format!("def {member}({params}) -> Any: ..."),
            format!("Add method `{member}` to the stub"),
        ),
        Usage::Attribute => (
            format!("{member}: Any"),
            format!("Add attribute `{member}` to the stub"),
        ),
    };

    Some(CodeAction {
        title: label,
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        command: Some(Command {
            title: format!("Add `{member}` to stub"),
            command: basilisk_common::commands::STUBS_ADD_MEMBER.to_owned(),
            arguments: Some(vec![
                serde_json::Value::String(stub_path.to_owned()),
                serde_json::Value::String(snippet),
            ]),
        }),
        is_preferred: Some(true),
        ..CodeAction::default()
    })
}

/// How a `module.attr` site is used, with parameters for a call.
enum Usage {
    /// `module.attr(...)` — a method; carries the inferred parameter list.
    Call(String),
    /// `module.attr` used as a value — an attribute.
    Attribute,
}

/// Extract the text between the first backtick after `marker` and the next backtick.
fn extract_backticked_after<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let start = text.find(marker)? + marker.len();
    let rest = text.get(start..)?;
    let end = rest.find('`')?;
    rest.get(..end)
}

/// Inspect the source just after the `module.attr` expression (whose end is at
/// `end` in LSP coordinates) to decide whether it is a call, and if so infer the
/// parameter list from the call arguments.
fn analyze_usage(source: &str, end: tower_lsp::lsp_types::Position) -> Usage {
    let offset = crate::util::position_to_byte_offset(source, end);
    let Some(rest) = source.get(offset..) else {
        return Usage::Attribute;
    };
    let after = rest.trim_start();
    let Some(args) = after.strip_prefix('(').and_then(extract_call_args) else {
        return Usage::Attribute;
    };
    Usage::Call(infer_params(&args))
}

/// Given the text immediately after a call's opening `(`, return the argument
/// text up to (excluding) the matching `)`, or `None` if unbalanced.
fn extract_call_args(after_open: &str) -> Option<String> {
    let mut depth: i32 = 1;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut out = String::new();
    for ch in after_open.chars() {
        if let Some(q) = quote {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => {
                quote = Some(ch);
                out.push(ch);
            }
            '(' | '[' | '{' => {
                depth += 1;
                out.push(ch);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                if ch == ')' && depth == 0 {
                    return Some(out);
                }
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    None
}

/// Infer a `.pyi` parameter list from a call's argument text. Positional args
/// become `argN: Any`; keyword args keep their name (`kw: Any`); a `*`/`**`
/// splat or anything unparseable falls back to `*args: Any, **kwargs: Any`.
fn infer_params(args: &str) -> String {
    if args.trim().is_empty() {
        return String::new();
    }
    let parts = split_top_level_commas(args);
    let mut params = Vec::with_capacity(parts.len());
    for (idx, raw) in parts.iter().enumerate() {
        let arg = raw.trim();
        if arg.is_empty() || arg.starts_with('*') {
            return "*args: Any, **kwargs: Any".to_owned();
        }
        match keyword_name(arg) {
            Some(name) => params.push(format!("{name}: Any")),
            None => params.push(format!("arg{idx}: Any")),
        }
    }
    params.join(", ")
}

/// Split `text` on top-level commas (ignoring commas inside brackets/strings).
fn split_top_level_commas(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth: i32 = 0;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut current = String::new();
    for ch in text.chars() {
        if let Some(q) = quote {
            current.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => {
                quote = Some(ch);
                current.push(ch);
            }
            '(' | '[' | '{' => {
                depth += 1;
                current.push(ch);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                parts.push(std::mem::take(&mut current));
            }
            _ => current.push(ch),
        }
    }
    parts.push(current);
    parts
}

/// If `arg` is a `name=value` keyword argument, return `name`; else `None`.
/// Comparison operators (`==`, `>=`, `!=`, …) are not keyword arguments.
fn keyword_name(arg: &str) -> Option<&str> {
    let eq = arg.find('=')?;
    if arg.as_bytes().get(eq + 1) == Some(&b'=') {
        return None; // `==`
    }
    let before = arg.get(..eq)?.trim_end();
    if before
        .chars()
        .last()
        .is_some_and(|c| "<>!+-*/%&|^~".contains(c))
    {
        return None; // augmented / comparison operator
    }
    let name = before.trim();
    let mut chars = name.chars();
    let starts_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    if starts_ok && chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        Some(name)
    } else {
        None
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test-only code: unwrap/indexing acceptable in unit tests"
)]
mod tests {
    use super::make_create_local_stub_action;
    use tower_lsp::lsp_types::{CodeAction, Diagnostic, NumberOrString, Range};

    fn diag() -> Diagnostic {
        Diagnostic {
            range: Range::default(),
            code: Some(NumberOrString::String("BSK-E0152".to_owned())),
            message: "Package `acme` is installed but has no type stubs available".to_owned(),
            ..Diagnostic::default()
        }
    }

    #[test]
    fn dispatches_create_local_command_with_module_arg() {
        let action = make_create_local_stub_action(&diag(), "acme", true);
        let command = action.command.unwrap();
        assert_eq!(
            command.command,
            basilisk_common::commands::STUBS_CREATE_LOCAL
        );
        assert_eq!(
            command.arguments.unwrap(),
            vec![serde_json::Value::String("acme".to_owned())]
        );
        assert!(action.is_preferred.unwrap());
        assert!(action.title.contains("acme.pyi"));
    }

    #[test]
    fn not_preferred_when_typeshed_stub_exists() {
        let action = make_create_local_stub_action(&diag(), "acme", false);
        assert!(!action.is_preferred.unwrap());
    }

    // ── Add-member quick fix (imports_module_attribute) ────────────────────────────────────
    use super::{infer_params, keyword_name, make_add_member_action};

    /// Build an E0154 diagnostic whose range ends right after `expr` in `source`.
    fn e0154(source: &str, expr: &str, member: &str, stub_path: &str) -> Diagnostic {
        let end_off = source.find(expr).unwrap() + expr.len();
        let end = crate::util::byte_offset_to_position(source, end_off);
        Diagnostic {
            range: Range {
                start: crate::util::byte_offset_to_position(source, source.find(expr).unwrap()),
                end,
            },
            code: Some(NumberOrString::String("imports_module_attribute".to_owned())),
            message: format!(
                "Module `cowsay` has no attribute `{member}`\n\nhelp: declare `{member}` in the local stub `{stub_path}`, or fix the typo.",
            ),
            ..Diagnostic::default()
        }
    }

    fn snippet_of(action: &CodeAction) -> String {
        let args = action.command.as_ref().unwrap().arguments.as_ref().unwrap();
        args[1].as_str().unwrap().to_owned()
    }

    #[test]
    fn add_member_for_call_infers_positional_params() {
        let source = "import cowsay\nx = cowsay.get_output_string(\"cow\", msg)\n";
        let diag = e0154(
            source,
            "cowsay.get_output_string",
            "get_output_string",
            "/w/.basilisk/stubs/cowsay.pyi",
        );
        let action = make_add_member_action(&diag, source).unwrap();
        let cmd = action.command.as_ref().unwrap();
        assert_eq!(cmd.command, basilisk_common::commands::STUBS_ADD_MEMBER);
        assert_eq!(
            cmd.arguments.as_ref().unwrap()[0].as_str().unwrap(),
            "/w/.basilisk/stubs/cowsay.pyi"
        );
        assert_eq!(
            snippet_of(&action),
            "def get_output_string(arg0: Any, arg1: Any) -> Any: ..."
        );
        assert!(action.title.contains("method"));
    }

    #[test]
    fn add_member_for_plain_access_is_an_attribute() {
        let source = "import cowsay\nv = cowsay.tagline\n";
        let diag = e0154(source, "cowsay.tagline", "tagline", "/w/s/cowsay.pyi");
        let action = make_add_member_action(&diag, source).unwrap();
        assert_eq!(snippet_of(&action), "tagline: Any");
        assert!(action.title.contains("attribute"));
    }

    #[test]
    fn add_member_keeps_keyword_argument_names() {
        let source = "import cowsay\ncowsay.render(text, width=80)\n";
        let diag = e0154(source, "cowsay.render", "render", "/w/s/cowsay.pyi");
        let action = make_add_member_action(&diag, source).unwrap();
        assert_eq!(
            snippet_of(&action),
            "def render(arg0: Any, width: Any) -> Any: ..."
        );
    }

    #[test]
    fn add_member_for_no_arg_call() {
        let source = "import cowsay\ncowsay.banner()\n";
        let diag = e0154(source, "cowsay.banner", "banner", "/w/s/cowsay.pyi");
        let action = make_add_member_action(&diag, source).unwrap();
        assert_eq!(snippet_of(&action), "def banner() -> Any: ...");
    }

    #[test]
    fn infer_params_falls_back_on_splat() {
        assert_eq!(infer_params("*args"), "*args: Any, **kwargs: Any");
        assert_eq!(infer_params(""), "");
        assert_eq!(infer_params("a, b"), "arg0: Any, arg1: Any");
    }

    #[test]
    fn keyword_name_rejects_comparisons() {
        assert_eq!(keyword_name("width=80"), Some("width"));
        assert_eq!(keyword_name("x == 1"), None);
        assert_eq!(keyword_name("x >= 1"), None);
        assert_eq!(keyword_name("f(a)"), None);
    }
}
