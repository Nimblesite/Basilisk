//! BSK-E0045: Invalid first argument to `Annotated[...]`.
//!
//! PEP 593 requires that the first argument to `Annotated[...]` be a valid type
//! expression. The following are errors:
//!
//! - List literals: `Annotated[[int, str], ""]`
//! - Tuple literals: `Annotated[((int, str),), ""]`
//! - Dict literals: `Annotated[{"a": "b"}, ""]`
//! - List comprehensions: `Annotated[[x for x in ...], ""]`
//! - Lambda calls: `Annotated[(lambda: int)(), ""]`
//! - Conditional expressions: `Annotated[int if cond else str, ""]`
//! - Boolean literals: `Annotated[True, ""]`
//! - Integer literals: `Annotated[1, ""]`
//! - Binary boolean operators: `Annotated[list or set, ""]`
//! - F-strings: `Annotated[f"...", ""]`
//! - Subscript-into-subscript: `Annotated[[int][0], ""]`
//!
//! Additionally, `Annotated[int]` with fewer than 2 arguments is an error,
//! and calling `Annotated` directly (bare or parameterized) is always invalid.
//!
//! ```python
//! Bad1: Annotated[[int, str], ""]   # E — list literal not valid type
//! Bad9: Annotated[True, ""]          # E — bool literal not valid type
//! Bad13: Annotated[int]              # E — requires at least two arguments
//! Annotated()                        # E — Annotated is not callable
//! SmallInt(1)                        # E — TypeAlias is not callable
//! ```

use std::collections::HashSet;

use basilisk_resolver::{CallSite, ClassInfo, FunctionInfo, ImportInfo, ImportKind, ResolvedModule, Span, VariableInfo};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0045",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0045",
};

fn span_text(source: &str, span: Option<Span>) -> Option<&str> {
    let span = span?;
    source.get(span.start as usize..span.end as usize)
}

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message,
        span,
        path: path.to_owned(),
        help: Some(
            "The first argument to `Annotated[...]` must be a valid type expression".to_owned(),
        ),
        note: Some(
            "PEP 593: `Annotated[T, metadata...]` requires T to be a type, not a literal or expression"
                .to_owned(),
        ),
    }
}

/// Extract the inner content of `Annotated[...]`.
///
/// Returns `None` if the annotation does not start with `Annotated[`.
fn annotated_inner(ann: &str) -> Option<&str> {
    let ann = ann.trim();
    if !ann.starts_with("Annotated[") {
        return None;
    }
    let inner_start = "Annotated[".len();
    let inner_end = ann.rfind(']')?;
    if inner_end <= inner_start {
        return None;
    }
    Some(&ann[inner_start..inner_end])
}

/// Extract just the first argument from the inner content of `Annotated[T, ...]`.
///
/// Handles nested brackets correctly by tracking depth.
fn first_arg(inner: &str) -> &str {
    let mut depth = 0i32;
    let mut end = inner.len();
    for (i, ch) in inner.char_indices() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => {
                depth -= 1;
            }
            ',' if depth == 0 => {
                end = i;
                break;
            }
            _ => {}
        }
    }
    inner[..end].trim()
}

/// Count top-level arguments in `Annotated[...]` inner content.
fn count_args(inner: &str) -> usize {
    if inner.trim().is_empty() {
        return 0;
    }
    let mut depth = 0i32;
    let mut count = 1usize;
    for ch in inner.chars() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth -= 1,
            ',' if depth == 0 => count += 1,
            _ => {}
        }
    }
    count
}

/// Returns `true` when the first-argument text is an invalid type expression.
fn is_invalid_type_expr(first: &str) -> bool {
    let first = first.trim();

    // Boolean literals: True, False
    if first == "True" || first == "False" {
        return true;
    }

    // Integer or float literals: starts with a digit
    if first.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return true;
    }

    // Negative numeric literals: -1, -3.14
    if first.starts_with('-')
        && first[1..]
            .trim()
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
    {
        return true;
    }

    // F-string: starts with f" or f'
    if first.starts_with("f\"") || first.starts_with("f'") {
        return true;
    }

    // List literal: starts with [ but not [int][0] (subscript) — detect list literal by
    // checking if it opens with [ and contains elements that look like a list.
    // A list literal starts with `[` and the content is not a subscript operation.
    if first.starts_with('[') && !is_subscript_expression(first) {
        return true;
    }

    // Dict literal: starts with {
    if first.starts_with('{') {
        return true;
    }

    // Tuple literal: starts with ( and contains a trailing comma or is a tuple
    // e.g. `((int, str),)` — outer parens wrapping a tuple
    if first.starts_with('(') && is_tuple_literal(first) {
        return true;
    }

    // Conditional expression: `X if cond else Y` — detect `if` keyword at depth 0
    if has_top_level_if(first) {
        return true;
    }

    // Boolean binary operator `or` / `and` at depth 0 (not `|` which is valid union)
    if has_top_level_bool_op(first) {
        return true;
    }

    // Lambda call: `(lambda: ...)()`
    if first.contains("lambda") {
        return true;
    }

    // Subscript-into-subscript: `[int][0]` — list literal then subscript
    // Detected by starting with `[` and having `][` pattern
    if first.contains("][") {
        return true;
    }

    false
}

/// Returns `true` when the expression is a subscript like `list[int]` (NOT `[int][0]`).
fn is_subscript_expression(s: &str) -> bool {
    // A subscript expression starts with a name, not `[`
    s.chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_')
}

/// Returns `true` when the expression looks like a tuple literal.
fn is_tuple_literal(s: &str) -> bool {
    // A tuple literal has a trailing comma before the closing paren,
    // or contains commas at depth 0 inside parens.
    if !s.starts_with('(') || !s.ends_with(')') {
        return false;
    }
    let inner = &s[1..s.len() - 1];
    let mut depth = 0i32;
    for ch in inner.chars() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth -= 1,
            ',' if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

/// Returns `true` when the expression has an `if` keyword at depth 0 — a conditional expr.
fn has_top_level_if(s: &str) -> bool {
    let mut depth = 0i32;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'[' | b'(' | b'{' => depth += 1,
            b']' | b')' | b'}' => depth -= 1,
            b'i' if depth == 0 => {
                // Check for ` if ` at this position
                if bytes.get(i..i + 4) == Some(b" if ")
                    || (i > 0 && bytes.get(i - 1..i + 3) == Some(b" if"))
                {
                    return true;
                }
            }
            _ => {}
        }
        i += 1;
    }
    // Also use a char-by-char walk for robustness
    let mut depth2 = 0i32;
    let chars: Vec<char> = s.chars().collect();
    let mut j = 0;
    while j < chars.len() {
        match chars.get(j).copied() {
            Some('[' | '(' | '{') => depth2 += 1,
            Some(']' | ')' | '}') => depth2 -= 1,
            Some(_) if depth2 == 0 => {
                // Look for " if " starting at j
                let rest: String = chars[j..].iter().collect();
                if rest.starts_with(" if ") {
                    return true;
                }
            }
            _ => {}
        }
        j += 1;
    }
    false
}

/// Returns `true` when the expression has `or` or `and` at depth 0 — boolean binary op.
fn has_top_level_bool_op(s: &str) -> bool {
    let mut depth = 0i32;
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars.get(i).copied() {
            Some('[' | '(' | '{') => depth += 1,
            Some(']' | ')' | '}') => depth -= 1,
            Some(_) if depth == 0 => {
                let rest: String = chars[i..].iter().collect();
                if rest.starts_with(" or ") || rest.starts_with(" and ") {
                    return true;
                }
            }
            _ => {}
        }
        i += 1;
    }
    false
}

/// Python built-in type names and common typing constructs that are always valid as types.
const BUILTIN_TYPE_NAMES: &[&str] = &[
    "int", "str", "float", "bool", "bytes", "bytearray", "list", "dict", "set", "frozenset",
    "tuple", "type", "object", "None", "complex", "memoryview", "range", "slice",
    "Exception", "BaseException", "ValueError", "TypeError", "KeyError", "IndexError",
    "AttributeError", "RuntimeError", "StopIteration", "NotImplementedError",
    "OverflowError", "ZeroDivisionError", "NameError", "ImportError", "OSError",
    "IOError", "FileNotFoundError", "PermissionError", "TimeoutError",
    // typing module names
    "Any", "Union", "Optional", "Tuple", "List", "Dict", "Set", "FrozenSet",
    "Callable", "Type", "ClassVar", "Final", "Literal", "Annotated", "TypeVar",
    "TypeVarTuple", "ParamSpec", "Generic", "Protocol", "TypedDict", "NamedTuple",
    "NewType", "TypeAlias", "Never", "NoReturn", "Self", "LiteralString", "Unpack",
    "Required", "NotRequired", "ReadOnly", "TypeGuard", "TypeIs", "Concatenate",
    "Awaitable", "Coroutine", "AsyncGenerator", "AsyncIterable", "AsyncIterator",
    "Generator", "Iterable", "Iterator", "Sequence", "MutableSequence", "Mapping",
    "MutableMapping", "MutableSet", "AbstractSet", "Hashable", "Sized", "Container",
    "Collection", "Reversible", "SupportsInt", "SupportsFloat", "SupportsComplex",
    "SupportsBytes", "SupportsAbs", "SupportsRound",
    // common builtins
    "T", "KT", "VT", "AnyStr",
];

/// Collect all names that are defined in module scope.
///
/// Returns a set of names that can be used as valid references in annotations.
/// This includes:
/// - Module-level variable names (including `TypeVar`, `TypeAlias`, etc.)
/// - Names imported via `from X import Y` or `import X`
/// - Class names
/// - Function names
fn collect_defined_names(
    vars: &[VariableInfo],
    imports: &[ImportInfo],
    classes: &[ClassInfo],
    functions: &[FunctionInfo],
) -> HashSet<String> {
    let mut names: HashSet<String> = BUILTIN_TYPE_NAMES.iter().map(|s| (*s).to_string()).collect();

    for var in vars {
        names.insert(var.name.clone());
    }

    for import in imports {
        match import.kind {
            ImportKind::Plain => {
                // `import os` binds `os`
                if let Some(first) = import.module.split('.').next() {
                    names.insert(first.to_owned());
                }
            }
            ImportKind::From => {
                // `from typing import Annotated` binds `Annotated`
                for name in &import.names {
                    names.insert(name.clone());
                }
            }
            ImportKind::Star => {
                // `from typing import *` — we can't know what's imported, so skip
            }
        }
    }

    for cls in classes {
        names.insert(cls.name.clone());
    }

    for func in functions {
        names.insert(func.name.clone());
    }

    names
}

/// Returns `true` when the first argument is a bare identifier that is not defined
/// in module scope and not a known built-in type.
///
/// This catches cases like `Annotated[var1, ""]` where `var1` is never defined.
fn is_undefined_bare_name(first: &str, defined_names: &HashSet<String>) -> bool {
    let first = first.trim();
    // Only match bare identifiers (no subscripts, dot access, call exprs, operators)
    if first.is_empty() {
        return false;
    }
    // Must be a valid identifier: all chars are alphanumeric or underscore, starts with letter/underscore
    let mut chars = first.chars();
    let Some(fc) = chars.next() else { return false };
    if !fc.is_alphabetic() && fc != '_' {
        return false;
    }
    if !chars.all(|c| c.is_alphanumeric() || c == '_') {
        return false;
    }
    // It's a bare identifier — check if it's in the defined names
    !defined_names.contains(first)
}

/// Build a set of variable names whose annotations are `TypeAlias` (or `typing.TypeAlias`).
///
/// A `TypeAlias`-annotated variable holds a type alias value, not a type constructor.
/// Calling such a variable (e.g. `SmallInt(1)` where `SmallInt: TypeAlias = ...`) is
/// invalid because type aliases are not callable.
fn collect_type_alias_names(vars: &[VariableInfo], source: &str) -> HashSet<String> {
    vars.iter()
        .filter_map(|var| {
            let ann = span_text(source, var.annotation_span)?;
            let ann = ann.trim();
            if ann == "TypeAlias" || ann == "typing.TypeAlias" {
                Some(var.name.clone())
            } else {
                None
            }
        })
        .collect()
}

/// Emit E0045 for module-level calls where the callee is a known `TypeAlias` name.
///
/// A `TypeAlias` variable holds a type expression, not a callable. Calling it is always
/// an error (`SmallInt(1)` where `SmallInt: TypeAlias = Annotated[int, ""]`).
fn check_type_alias_calls(
    calls: &[CallSite],
    type_alias_names: &HashSet<String>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for call in calls {
        if type_alias_names.contains(&call.callee) {
            diagnostics.push(make_diagnostic(
                format!(
                    "`{}` is a type alias and cannot be called as a function",
                    call.callee
                ),
                call.span,
                path,
            ));
        }
    }
}

/// Returns `true` when the annotation text represents `type[...]` (e.g. `type[Any]`).
///
/// This includes `type[Any]`, `type[int]`, `type[T]` etc. but NOT plain `type`.
fn annotation_is_type_subscript(ann: &str) -> bool {
    let ann = ann.trim();
    ann.starts_with("type[") || ann.starts_with("typing.Type[") || ann.starts_with("Type[")
}

/// Emit E0045 for module variables annotated `type[...]` whose RHS is an `Annotated[...]`
/// expression or a known `TypeAlias` name.
///
/// PEP 593: `Annotated[T, ...]` is not compatible with `type[T]` — it is a value that
/// carries metadata, not a type constructor.
fn check_vars_type_annotation_incompatible(
    vars: &[basilisk_resolver::VariableInfo],
    source: &str,
    path: &str,
    type_alias_names: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for var in vars {
        let Some(ann) = span_text(source, var.annotation_span) else {
            continue;
        };
        if !annotation_is_type_subscript(ann.trim()) {
            continue;
        }
        let Some(rhs) = span_text(source, var.rhs_span) else {
            continue;
        };
        let rhs = rhs.trim();
        if rhs.starts_with("Annotated[") {
            diagnostics.push(make_diagnostic(
                format!(
                    "`Annotated[...]` is not compatible with `type[...]` for `{}`",
                    var.name
                ),
                var.name_span,
                path,
            ));
        } else if type_alias_names.contains(rhs) {
            diagnostics.push(make_diagnostic(
                format!(
                    "Type alias `{rhs}` (an `Annotated[...]` alias) is not compatible with `type[...]` for `{}`",
                    var.name
                ),
                var.name_span,
                path,
            ));
        }
    }
}

/// Emit E0045 for module-level call sites where a positional argument is an `Annotated[...]`
/// subscript expression, or a known `TypeAlias` name.
///
/// PEP 593: `Annotated[T, ...]` is not type-compatible with `type[T]` — passing it where
/// a `type[T]` value is expected is always a type error.
fn check_calls_with_annotated_args(
    calls: &[CallSite],
    source: &str,
    path: &str,
    type_alias_names: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for call in calls {
        // Skip calls whose callee is already a TypeAlias name (handled by check_type_alias_calls).
        if type_alias_names.contains(&call.callee) {
            continue;
        }
        for (_kind, arg_span) in &call.args {
            let Some(arg_text) = span_text(source, Some(*arg_span)) else {
                continue;
            };
            let arg_text = arg_text.trim();
            if arg_text.starts_with("Annotated[") {
                diagnostics.push(make_diagnostic(
                    format!(
                        "`Annotated[...]` is not compatible with `type[T]` — \
                         `{arg_text}` cannot be used where a type constructor is expected"
                    ),
                    call.span,
                    path,
                ));
            } else if type_alias_names.contains(arg_text) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Type alias `{arg_text}` (an `Annotated[...]` alias) is not \
                         compatible with `type[T]`"
                    ),
                    call.span,
                    path,
                ));
            }
        }
    }
}

/// Emits BSK-E0045 when `Annotated[...]` has an invalid first argument, too few args,
/// or when `Annotated` (or a `TypeAlias`) is called directly as a function.
pub(crate) struct AnnotatedInvalidFirstArg;

impl Rule for AnnotatedInvalidFirstArg {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        let defined_names = collect_defined_names(
            &module.module_vars,
            &module.imports,
            &module.classes,
            &module.functions,
        );

        check_annotated_in_vars(&module.module_vars, source, path, &defined_names, diagnostics);

        for cls in &module.classes {
            check_annotated_in_attrs(&cls.attributes, source, path, &defined_names, diagnostics);
        }

        check_annotated_in_functions(&module.functions, source, path, &defined_names, diagnostics);

        // Detect direct calls to `Annotated` or `Annotated[...]` — always invalid.
        for span in &module.annotated_direct_call_spans {
            let call_text = span_text(source, Some(*span)).unwrap_or("Annotated");
            diagnostics.push(make_diagnostic(
                format!(
                    "`Annotated` is not callable — `{call_text}` must not be called as a function"
                ),
                *span,
                path,
            ));
        }

        // Detect calls to TypeAlias names (e.g. `SmallInt(1)` where
        // `SmallInt: TypeAlias = Annotated[int, ""]`).
        let type_alias_names = collect_type_alias_names(&module.module_vars, source);
        check_type_alias_calls(&module.calls, &type_alias_names, path, diagnostics);

        // Detect `type[...] = Annotated[...]` and `type[...] = <TypeAlias>` assignments.
        // PEP 593: Annotated is not type-compatible with `type` or `type[T]`.
        check_vars_type_annotation_incompatible(
            &module.module_vars,
            source,
            path,
            &type_alias_names,
            diagnostics,
        );

        // Detect `func(Annotated[...])` and `func(TypeAlias)` call arguments.
        // Passing an Annotated expression or TypeAlias where `type[T]` is expected is invalid.
        check_calls_with_annotated_args(&module.calls, source, path, &type_alias_names, diagnostics);
    }
}

fn check_annotated_in_vars(
    vars: &[basilisk_resolver::VariableInfo],
    source: &str,
    path: &str,
    defined_names: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for var in vars {
        let Some(ann) = span_text(source, var.annotation_span) else {
            continue;
        };
        check_annotated_annotation(ann.trim(), var.name_span, &var.name, path, defined_names, diagnostics);
    }
}

fn check_annotated_in_attrs(
    attrs: &[basilisk_resolver::AttributeInfo],
    source: &str,
    path: &str,
    defined_names: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for attr in attrs {
        let Some(ann) = span_text(source, attr.annotation_span) else {
            continue;
        };
        check_annotated_annotation(ann.trim(), attr.name_span, &attr.name, path, defined_names, diagnostics);
    }
}

fn check_annotated_in_functions(
    funcs: &[basilisk_resolver::FunctionInfo],
    source: &str,
    path: &str,
    defined_names: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for func in funcs {
        for param in func
            .parameters
            .iter()
            .chain(func.vararg.iter())
            .chain(func.kwarg.iter())
        {
            let Some(ann) = span_text(source, param.annotation_span) else {
                continue;
            };
            check_annotated_annotation(ann.trim(), param.name_span, &param.name, path, defined_names, diagnostics);
        }
    }
}

fn check_annotated_annotation(
    ann: &str,
    span: Span,
    name: &str,
    path: &str,
    defined_names: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(inner) = annotated_inner(ann) else {
        return;
    };

    let arg_count = count_args(inner);

    // Annotated[int] — too few arguments
    if arg_count < 2 {
        diagnostics.push(make_diagnostic(
            format!("`Annotated` requires at least two arguments for `{name}`"),
            span,
            path,
        ));
        return;
    }

    // Check that the first argument is a valid type expression
    let first = first_arg(inner);
    if is_invalid_type_expr(first) || is_undefined_bare_name(first, defined_names) {
        diagnostics.push(make_diagnostic(
            format!("Invalid type expression as first argument to `Annotated` for `{name}`"),
            span,
            path,
        ));
    }
}
