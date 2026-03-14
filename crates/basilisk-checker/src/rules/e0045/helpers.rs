//! Helper types and pure functions for BSK-E0045.
//!
//! Contains expression-validity checks, the built-in type name list,
//! and collection helpers used by the main rule.

use std::collections::HashSet;

use basilisk_resolver::{ClassInfo, FunctionInfo, ImportInfo, ImportKind, VariableInfo};

use crate::span_util::slice_span;
use basilisk_resolver::Span;

// ---------------------------------------------------------------------------
// Built-in type name list
// ---------------------------------------------------------------------------

/// Python built-in type names and common typing constructs that are always valid as types.
pub(super) const BUILTIN_TYPE_NAMES: &[&str] = &[
    "int",
    "str",
    "float",
    "bool",
    "bytes",
    "bytearray",
    "list",
    "dict",
    "set",
    "frozenset",
    "tuple",
    "type",
    "object",
    "None",
    "complex",
    "memoryview",
    "range",
    "slice",
    "Exception",
    "BaseException",
    "ValueError",
    "TypeError",
    "KeyError",
    "IndexError",
    "AttributeError",
    "RuntimeError",
    "StopIteration",
    "NotImplementedError",
    "OverflowError",
    "ZeroDivisionError",
    "NameError",
    "ImportError",
    "OSError",
    "IOError",
    "FileNotFoundError",
    "PermissionError",
    "TimeoutError",
    // typing module names
    "Any",
    "Union",
    "Optional",
    "Tuple",
    "List",
    "Dict",
    "Set",
    "FrozenSet",
    "Callable",
    "Type",
    "ClassVar",
    "Final",
    "Literal",
    "Annotated",
    "TypeVar",
    "TypeVarTuple",
    "ParamSpec",
    "Generic",
    "Protocol",
    "TypedDict",
    "NamedTuple",
    "NewType",
    "TypeAlias",
    "Never",
    "NoReturn",
    "Self",
    "LiteralString",
    "Unpack",
    "Required",
    "NotRequired",
    "ReadOnly",
    "TypeGuard",
    "TypeIs",
    "Concatenate",
    "Awaitable",
    "Coroutine",
    "AsyncGenerator",
    "AsyncIterable",
    "AsyncIterator",
    "Generator",
    "Iterable",
    "Iterator",
    "Sequence",
    "MutableSequence",
    "Mapping",
    "MutableMapping",
    "MutableSet",
    "AbstractSet",
    "Hashable",
    "Sized",
    "Container",
    "Collection",
    "Reversible",
    "SupportsInt",
    "SupportsFloat",
    "SupportsComplex",
    "SupportsBytes",
    "SupportsAbs",
    "SupportsRound",
    // common builtins
    "T",
    "KT",
    "VT",
    "AnyStr",
];

// ---------------------------------------------------------------------------
// Annotated[...] parsing
// ---------------------------------------------------------------------------

/// Extract the inner content of `Annotated[...]`.
///
/// Returns `None` if the annotation does not start with `Annotated[`.
pub(super) fn annotated_inner(ann: &str) -> Option<&str> {
    let ann = ann.trim();
    if !ann.starts_with("Annotated[") {
        return None;
    }
    let inner_start = "Annotated[".len();
    let inner_end = ann.rfind(']')?;
    if inner_end <= inner_start {
        return None;
    }
    ann.get(inner_start..inner_end)
}

/// Extract just the first argument from the inner content of `Annotated[T, ...]`.
///
/// Handles nested brackets correctly by tracking depth.
pub(super) fn first_arg(inner: &str) -> &str {
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
pub(super) fn count_args(inner: &str) -> usize {
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

// ---------------------------------------------------------------------------
// Expression validity checks
// ---------------------------------------------------------------------------

/// Returns `true` when the first-argument text is an invalid type expression.
pub(super) fn is_invalid_type_expr(first: &str) -> bool {
    let first = first.trim();

    if first == "True" || first == "False" {
        return true;
    }

    if first.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return true;
    }

    if first.starts_with('-')
        && first[1..]
            .trim()
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
    {
        return true;
    }

    if first.starts_with("f\"") || first.starts_with("f'") {
        return true;
    }

    if first.starts_with('[') && !is_subscript_expression(first) {
        return true;
    }

    if first.starts_with('{') {
        return true;
    }

    if first.starts_with('(') && is_tuple_literal(first) {
        return true;
    }

    if has_top_level_if(first) {
        return true;
    }

    if has_top_level_bool_op(first) {
        return true;
    }

    if first.contains("lambda") {
        return true;
    }

    if first.contains("][") {
        return true;
    }

    false
}

/// Returns `true` when the expression is a subscript like `list[int]` (NOT `[int][0]`).
fn is_subscript_expression(s: &str) -> bool {
    s.chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_')
}

/// Returns `true` when the expression looks like a tuple literal.
pub(super) fn is_tuple_literal(s: &str) -> bool {
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

/// Returns `true` when the expression has an `if` keyword at depth 0.
pub(super) fn has_top_level_if(s: &str) -> bool {
    let mut depth = 0i32;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes.get(i).copied() {
            Some(b'[' | b'(' | b'{') => depth += 1,
            Some(b']' | b')' | b'}') => depth -= 1,
            Some(b'i') if depth == 0 => {
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
    // Char-by-char walk for robustness.
    let mut depth2 = 0i32;
    let chars: Vec<char> = s.chars().collect();
    let mut j = 0;
    while j < chars.len() {
        match chars.get(j).copied() {
            Some('[' | '(' | '{') => depth2 += 1,
            Some(']' | ')' | '}') => depth2 -= 1,
            Some(_) if depth2 == 0 => {
                let rest: String = chars.get(j..).unwrap_or_default().iter().collect();
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

/// Returns `true` when the expression has `or` or `and` at depth 0.
pub(super) fn has_top_level_bool_op(s: &str) -> bool {
    let mut depth = 0i32;
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars.get(i).copied() {
            Some('[' | '(' | '{') => depth += 1,
            Some(']' | ')' | '}') => depth -= 1,
            Some(_) if depth == 0 => {
                let rest: String = chars.get(i..).unwrap_or_default().iter().collect();
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

// ---------------------------------------------------------------------------
// Module-scope name collection
// ---------------------------------------------------------------------------

/// Collect all names that are defined in module scope (vars, imports, classes, functions).
pub(super) fn collect_defined_names(
    vars: &[VariableInfo],
    imports: &[ImportInfo],
    classes: &[ClassInfo],
    functions: &[FunctionInfo],
) -> HashSet<String> {
    let mut names: HashSet<String> = BUILTIN_TYPE_NAMES
        .iter()
        .map(|s| (*s).to_string())
        .collect();

    for var in vars {
        let _ = names.insert(var.name.clone());
    }

    for import in imports {
        match import.kind {
            ImportKind::Plain => {
                if let Some(first) = import.module.split('.').next() {
                    let _ = names.insert(first.to_owned());
                }
            }
            ImportKind::From => {
                for name in &import.names {
                    let _ = names.insert(name.clone());
                }
            }
            ImportKind::Star => {}
        }
    }

    for cls in classes {
        let _ = names.insert(cls.name.clone());
    }

    for func in functions {
        let _ = names.insert(func.name.clone());
    }

    names
}

/// Returns `true` when the first argument is a bare identifier that is not defined
/// in module scope and not a known built-in type.
pub(super) fn is_undefined_bare_name(first: &str, defined_names: &HashSet<String>) -> bool {
    let first = first.trim();
    if first.is_empty() {
        return false;
    }
    let mut chars = first.chars();
    let Some(fc) = chars.next() else { return false };
    if !fc.is_alphabetic() && fc != '_' {
        return false;
    }
    if !chars.all(|c| c.is_alphanumeric() || c == '_') {
        return false;
    }
    !defined_names.contains(first)
}

// ---------------------------------------------------------------------------
// TypeAlias detection
// ---------------------------------------------------------------------------

/// Build a set of variable names whose annotations are `TypeAlias` (or `typing.TypeAlias`).
///
/// A `TypeAlias`-annotated variable holds a type alias value, not a type constructor.
/// Calling such a variable (e.g. `SmallInt(1)` where `SmallInt: TypeAlias = ...`) is invalid.
pub(super) fn collect_type_alias_names(vars: &[VariableInfo], source: &str) -> HashSet<String> {
    vars.iter()
        .filter_map(|var| {
            let ann = slice_span(source, var.annotation_span?)?;
            let ann = ann.trim();
            if ann == "TypeAlias" || ann == "typing.TypeAlias" {
                Some(var.name.clone())
            } else {
                None
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// type[...] annotation detection
// ---------------------------------------------------------------------------

/// Returns `true` when the annotation text represents `type[...]`.
pub(super) fn annotation_is_type_subscript(ann: &str) -> bool {
    let ann = ann.trim();
    ann.starts_with("type[") || ann.starts_with("typing.Type[") || ann.starts_with("Type[")
}

/// Emit E0045 for module variables annotated `type[...]` whose RHS is an `Annotated[...]`
/// expression or a known `TypeAlias` name.
pub(super) fn check_vars_type_annotation_incompatible(
    vars: &[VariableInfo],
    source: &str,
    path: &str,
    type_alias_names: &HashSet<String>,
    diagnostics: &mut Vec<crate::diagnostic::Diagnostic>,
    make_diag: &impl Fn(String, Span, &str) -> crate::diagnostic::Diagnostic,
) {
    for var in vars {
        let Some(ann_span) = var.annotation_span else {
            continue;
        };
        let Some(ann) = slice_span(source, ann_span) else {
            continue;
        };
        if !annotation_is_type_subscript(ann.trim()) {
            continue;
        }
        let Some(rhs_span) = var.rhs_span else {
            continue;
        };
        let Some(rhs) = slice_span(source, rhs_span) else {
            continue;
        };
        let rhs = rhs.trim();
        if rhs.starts_with("Annotated[") {
            diagnostics.push(make_diag(
                format!(
                    "`Annotated[...]` is not compatible with `type[...]` for `{}`",
                    var.name
                ),
                var.name_span,
                path,
            ));
        } else if type_alias_names.contains(rhs) {
            diagnostics.push(make_diag(
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
