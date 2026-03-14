//! Argument validation helpers for BSK-E0036: `ClassVar` argument correctness
//! checks and type-mismatch detection between the `ClassVar` inner type and the RHS.

use basilisk_resolver::Span;

use crate::diagnostic::Diagnostic;

use super::helpers::{contains_word, make_diagnostic, TypeParamKind};

/// Extract the content between the outer `[...]` of a `ClassVar[...]` or `CV[...]`
/// annotation text.  Returns `None` when there is no subscript.
pub(super) fn extract_classvar_inner(ann: &str) -> Option<&str> {
    // Find the start: "ClassVar[" or "CV["
    let prefix_len = if ann.starts_with("ClassVar[") {
        "ClassVar[".len()
    } else if ann.starts_with("CV[") {
        "CV[".len()
    } else if ann.starts_with("Annotated[ClassVar[") {
        // Skip Annotated wrapper — valid per spec
        return None;
    } else {
        return None;
    };

    // Find the matching closing bracket by counting nesting
    let bytes = ann.as_bytes();
    let mut depth: u32 = 1;
    let mut end_idx = None;
    for (idx, &byte) in bytes.iter().enumerate().skip(prefix_len) {
        match byte {
            b'[' => depth = depth.saturating_add(1),
            b']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    end_idx = Some(idx);
                    break;
                }
            }
            _ => {}
        }
    }

    end_idx.and_then(|end| ann.get(prefix_len..end))
}

/// Count the number of top-level comma-separated arguments in a bracket body.
pub(super) fn count_top_level_args(inner: &str) -> usize {
    if inner.trim().is_empty() {
        return 0;
    }
    let mut depth: u32 = 0;
    let mut count: usize = 1;
    for byte in inner.as_bytes() {
        match byte {
            b'[' | b'(' => depth = depth.saturating_add(1),
            b']' | b')' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => count = count.saturating_add(1),
            _ => {}
        }
    }
    count
}

/// Returns `true` when the argument text looks like a numeric literal (e.g. `3`, `3.14`).
pub(super) fn is_numeric_literal(arg: &str) -> bool {
    let trimmed = arg.trim();
    if trimmed.is_empty() {
        return false;
    }
    // A numeric literal: all digits, optionally with a single dot
    trimmed
        .chars()
        .all(|ch| ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+')
        && trimmed.chars().any(|ch| ch.is_ascii_digit())
}

/// Known built-in type names that are valid as `ClassVar` arguments even though
/// they start with lowercase (e.g. `int`, `str`, `float`, `bool`, `bytes`, `list`,
/// `dict`, `set`, `tuple`, `type`, `object`, `complex`, `range`, `slice`,
/// `frozenset`, `bytearray`, `memoryview`).
const LOWERCASE_TYPE_NAMES: &[&str] = &[
    "int",
    "str",
    "float",
    "bool",
    "bytes",
    "list",
    "dict",
    "set",
    "tuple",
    "type",
    "object",
    "complex",
    "range",
    "slice",
    "frozenset",
    "bytearray",
    "memoryview",
    "property",
    "staticmethod",
    "classmethod",
    "super",
];

/// Returns `true` when the argument text looks like a runtime variable reference
/// (a simple identifier that is not a known type name).
///
/// A bare identifier that starts with a lowercase letter and is NOT one of the
/// known built-in types is considered a runtime variable.
pub(super) fn is_runtime_variable(arg: &str, module_var_names: &[String]) -> bool {
    let trimmed = arg.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Must be a simple identifier (no brackets, dots, etc.)
    if !trimmed.chars().all(|ch| ch.is_alphanumeric() || ch == '_') {
        return false;
    }
    // Check if it's a known module-level variable (runtime value)
    if module_var_names.iter().any(|name| name == trimmed) {
        return true;
    }
    // A bare lowercase identifier that is not a known type is likely a runtime variable
    let first_char = trimmed.chars().next();
    if first_char.is_some_and(|ch| ch.is_ascii_lowercase()) {
        return !LOWERCASE_TYPE_NAMES.contains(&trimmed);
    }
    false
}

/// Check if an annotation's `ClassVar` argument contains any of the given type
/// parameter names (`TypeVar`, `ParamSpec`, `TypeVarTuple` names).
pub(super) fn contains_type_param(
    ann_inner: &str,
    type_param_names: &[(String, TypeParamKind)],
) -> Option<TypeParamKind> {
    for (name, kind) in type_param_names {
        // Check for the name appearing as a standalone word or as part of a subscript
        // e.g. `T` in `list[T]`, `P` in `Callable[P, Any]`
        if contains_word(ann_inner, name) {
            return Some(*kind);
        }
    }
    None
}

/// Validate `ClassVar` arguments for correctness.
///
/// Emits diagnostics for too many arguments, numeric literal arguments,
/// runtime variable arguments, and type parameter arguments.
pub(super) fn check_classvar_args(
    inner: &str,
    attr_name: &str,
    name_span: Span,
    path: &str,
    type_param_names: &[(String, TypeParamKind)],
    module_var_names: &[String],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let arg_count = count_top_level_args(inner);

    // Too many arguments (ClassVar accepts at most 1)
    if arg_count > 1 {
        diagnostics.push(make_diagnostic(
            format!(
                "`ClassVar` accepts at most one type argument, but `{attr_name}` has {arg_count}",
            ),
            name_span,
            path,
        ));
        return;
    }

    let trimmed = inner.trim();

    // Check for numeric literal argument (e.g. ClassVar[3])
    if is_numeric_literal(trimmed) {
        diagnostics.push(make_diagnostic(
            format!(
                "Invalid `ClassVar` argument for `{attr_name}`: `{trimmed}` is not a valid type",
            ),
            name_span,
            path,
        ));
        return;
    }

    // Check for runtime variable argument (e.g. ClassVar[var])
    if is_runtime_variable(trimmed, module_var_names) {
        diagnostics.push(make_diagnostic(
            format!(
                "Invalid `ClassVar` argument for `{attr_name}`: `{trimmed}` is a runtime variable, not a type",
            ),
            name_span,
            path,
        ));
        return;
    }

    // Check for TypeVar/ParamSpec/TypeVarTuple in ClassVar argument
    if let Some(kind) = contains_type_param(trimmed, type_param_names) {
        let kind_name = match kind {
            TypeParamKind::TypeVar => "TypeVar",
            TypeParamKind::ParamSpec => "ParamSpec",
            TypeParamKind::TypeVarTuple => "TypeVarTuple",
        };
        diagnostics.push(make_diagnostic(
            format!("`ClassVar` parameter for `{attr_name}` cannot contain {kind_name}",),
            name_span,
            path,
        ));
    }
}

/// Check for type mismatch between a `ClassVar` annotation's inner type and the
/// RHS value.  For example, `ClassVar[list[str]] = {}` is a mismatch because `{}`
/// is a dict literal but the annotation expects a list.
pub(super) fn check_classvar_type_mismatch(
    inner: &str,
    rhs_text: &str,
    attr_name: &str,
    name_span: Span,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let trimmed_inner = inner.trim();
    let trimmed_rhs = rhs_text.trim();

    if trimmed_rhs.is_empty() || trimmed_inner.is_empty() {
        return;
    }

    // Detect dict literal `{}` or `{...}` assigned to a list/set/tuple type
    let rhs_is_dict = trimmed_rhs.starts_with('{');
    let rhs_is_list = trimmed_rhs.starts_with('[');

    let inner_is_list = trimmed_inner.starts_with("list") || trimmed_inner.starts_with("List");
    let inner_is_dict = trimmed_inner.starts_with("dict") || trimmed_inner.starts_with("Dict");
    let inner_is_set = trimmed_inner.starts_with("set")
        || trimmed_inner.starts_with("Set")
        || trimmed_inner.starts_with("frozenset")
        || trimmed_inner.starts_with("FrozenSet");

    // Dict literal assigned to list/set/tuple type
    if rhs_is_dict && (inner_is_list || inner_is_set) {
        diagnostics.push(make_diagnostic(
            format!(
                "Type mismatch in `ClassVar` attribute `{attr_name}`: \
                 annotated as `{trimmed_inner}` but initialized with a dict literal",
            ),
            name_span,
            path,
        ));
        return;
    }

    // List literal assigned to dict type
    if rhs_is_list && inner_is_dict {
        diagnostics.push(make_diagnostic(
            format!(
                "Type mismatch in `ClassVar` attribute `{attr_name}`: \
                 annotated as `{trimmed_inner}` but initialized with a list literal",
            ),
            name_span,
            path,
        ));
    }
}
