//! Implements [`qualifiers_annotated`] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-immutability
//! Helper functions for `qualifiers_annotated`: Invalid first argument to `Annotated[...]`.
//!
//! Contains annotation parsing utilities, type expression validity checks,
//! and name collection helpers used by the main rule implementation.

use std::collections::HashSet;

use basilisk_resolver::{ClassInfo, FunctionInfo, ImportInfo, ImportKind, Span, VariableInfo};

use crate::span_util::slice_span;

// ---------------------------------------------------------------------------
// Span utilities
// ---------------------------------------------------------------------------

/// Slice a source string to get the text at a given span.
pub(super) fn span_text(source: &str, span: Option<Span>) -> Option<&str> {
    slice_span(source, span?)
}

// ---------------------------------------------------------------------------
// Annotated inner parsing
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
// Type expression validity
// ---------------------------------------------------------------------------

/// Returns `true` when the first-argument text is an invalid type expression.
pub(super) fn is_invalid_type_expr(first: &str) -> bool {
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
    crate::rules::shared::paren_has_top_level_comma(s)
}

/// Returns `true` when the expression has an `if` keyword at depth 0 — a conditional expr.
fn has_top_level_if(s: &str) -> bool {
    let mut depth = 0i32;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes.get(i).copied() {
            Some(b'[' | b'(' | b'{') => depth += 1,
            Some(b']' | b')' | b'}') => depth -= 1,
            Some(b'i')
                if depth == 0
                // Check for ` if ` at this position
                && (bytes.get(i..i + 4) == Some(b" if ")
                    || (i > 0 && bytes.get(i - 1..i + 3) == Some(b" if"))) =>
            {
                return true;
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
// Name collection
// ---------------------------------------------------------------------------

/// Python built-in type names and common typing constructs that are always valid as types.
const BUILTIN_TYPE_NAMES: &[&str] = &[
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

/// Collect all names that are defined in module scope.
///
/// Returns a set of names that can be used as valid references in annotations.
/// This includes:
/// - Module-level variable names (including `TypeVar`, `TypeAlias`, etc.)
/// - Names imported via `from X import Y` or `import X`
/// - Class names
/// - Function names
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
                // `import os` binds `os`
                if let Some(first) = import.module.split('.').next() {
                    let _ = names.insert(first.to_owned());
                }
            }
            ImportKind::From => {
                // `from typing import Annotated` binds `Annotated`
                for name in &import.names {
                    let _ = names.insert(name.clone());
                }
            }
            ImportKind::Star => {
                // `from typing import *` — we can't know what's imported, so skip
            }
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
///
/// This catches cases like `Annotated[var1, ""]` where `var1` is never defined.
pub(super) fn is_undefined_bare_name(first: &str, defined_names: &HashSet<String>) -> bool {
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
pub(super) fn collect_type_alias_names(vars: &[VariableInfo], source: &str) -> HashSet<String> {
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

/// Returns `true` when the annotation text represents `type[...]` (e.g. `type[Any]`).
///
/// This includes `type[Any]`, `type[int]`, `type[T]` etc. but NOT plain `type`.
pub(super) fn annotation_is_type_subscript(ann: &str) -> bool {
    let ann = ann.trim();
    ann.starts_with("type[") || ann.starts_with("typing.Type[") || ann.starts_with("Type[")
}
