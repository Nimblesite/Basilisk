//! Implements [BSK-E0070] from [CHKARCH-DIAG-OPTIONAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-optional
//! BSK-E0070: `Never` type compatibility violations.
//!
//! Detects type compatibility errors involving the `Never` bottom type:
//!
//! 1. Assigning a parameter typed `Container[Never]` to a local annotated
//!    `Container[T]` where `T` is not `Never` or `Any` (invariant violation)
//! 2. Returning `ClassC[Never]()` from a function annotated `-> ClassC[U]`
//!    where the class's type parameter is invariant (not covariant)
//!
//! ```python
//! from typing import Never, Any, Generic, TypeVar
//!
//! T = TypeVar("T")
//! U = TypeVar("U")
//!
//! def func(c: list[Never]):
//!     v: list[int] = c  # E0070 — list is invariant, list[Never] != list[int]
//!
//! class ClassC(Generic[T]):
//!     pass
//!
//! def func2(x: U) -> ClassC[U]:
//!     return ClassC[Never]()  # E0070 — ClassC is invariant
//! ```

use basilisk_resolver::{FunctionInfo, ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0070",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0070",
};

/// Emits BSK-E0070 for Never type compatibility violations.
pub(crate) struct NeverTypeCompatibility;

impl Rule for NeverTypeCompatibility {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        // Collect covariant TypeVar names so we can exclude covariant contexts.
        let covariant_tvars: Vec<&str> =
            basilisk_resolver::collect_names_where(&module.typevar_calls, |tv| tv.is_covariant);

        // Check function bodies for annotated local assignments and return stmts.
        for func in &module.functions {
            check_local_assignments(func, source, path, diagnostics);
            check_return_stmts(func, source, path, &covariant_tvars, module, diagnostics);
        }
    }
}

/// Scan source lines for annotated local variable assignments where the RHS is
/// a parameter typed `Container[Never]` and the LHS declares `Container[T]`
/// with `T` not being `Never` or `Any`.
fn check_local_assignments(
    func: &FunctionInfo,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Build a list of (parameter_name, annotation_text) pairs.
    let param_annotations: Vec<(&str, &str)> = func
        .parameters
        .iter()
        .filter_map(|param| {
            let ann_span = param.annotation_span?;
            let ann_text = slice_span(source, ann_span)?;
            Some((param.name.as_str(), ann_text.trim()))
        })
        .collect();

    if param_annotations.is_empty() {
        return;
    }

    // Scan source lines for annotated assignments.
    for (idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        // Quick pre-filter: must contain both `: ` and ` = `.
        if !trimmed.contains(": ") || !trimmed.contains(" = ") {
            continue;
        }

        // Parse: `var_name: annotation = rhs_name`
        let Some((var_name, rest)) = trimmed.split_once(": ") else {
            continue;
        };
        let var_name = var_name.trim();

        if !is_simple_identifier(var_name) {
            continue;
        }

        let Some((annotation, rhs_part)) = rest.split_once(" = ") else {
            continue;
        };
        let annotation = annotation.trim();

        // Strip trailing comments from the RHS.
        let rhs_name = rhs_part.split('#').next().unwrap_or(rhs_part).trim();

        // RHS must be a simple identifier (a parameter reference).
        if !is_simple_identifier(rhs_name) {
            continue;
        }

        // Look up the RHS name among the function's parameters.
        let Some((_, param_annotation)) =
            param_annotations.iter().find(|(name, _)| *name == rhs_name)
        else {
            continue;
        };

        // Check for invariant Never mismatch.
        // E.g. annotation = "list[int]", param_annotation = "list[Never]"
        if let (Some(target_inner), Some(source_inner)) = (
            extract_generic_inner(annotation),
            extract_generic_inner(param_annotation),
        ) {
            let target_base = extract_generic_base(annotation);
            let source_base = extract_generic_base(param_annotation);

            if target_base == source_base
                && source_inner == "Never"
                && target_inner != "Never"
                && target_inner != "Any"
            {
                let line_offset = line_byte_offset(source, idx);
                let name_start_in_line = line.find(var_name).unwrap_or(0);
                let span_start = u32::try_from(line_offset + name_start_in_line).unwrap_or(0);
                let span_end = span_start + u32::try_from(var_name.len()).unwrap_or(0);

                diagnostics.push(make_assignment_diagnostic(
                    Span {
                        start: span_start,
                        end: span_end,
                    },
                    var_name,
                    annotation,
                    param_annotation,
                    path,
                ));
            }
        }
    }
}

/// Check return statements for invariant Never violations.
fn check_return_stmts(
    func: &FunctionInfo,
    source: &str,
    path: &str,
    covariant_tvars: &[&str],
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(ann_span) = func.return_annotation_span else {
        return;
    };
    let Some(ann_text) = slice_span(source, ann_span) else {
        return;
    };
    let ann_text = ann_text.trim();

    // Only handle generic return types like `ClassC[U]`.
    let Some(ann_inner) = extract_generic_inner(ann_text) else {
        return;
    };
    let ann_base = extract_generic_base(ann_text);

    for ret_stmt in &func.return_stmts {
        if !ret_stmt.has_value {
            continue;
        }
        let Some(ret_full) = slice_span(source, ret_stmt.span) else {
            continue;
        };
        let ret_full = ret_full.trim();

        // Strip "return " prefix.
        let ret_expr = ret_full.strip_prefix("return ").unwrap_or(ret_full).trim();

        // Handle call expressions: `ClassC[Never]()` -> `ClassC[Never]`.
        let ret_type_text = strip_call_parens(ret_expr);

        let Some(ret_inner) = extract_generic_inner(ret_type_text) else {
            continue;
        };
        let ret_base = extract_generic_base(ret_type_text);

        // The generic bases must match.
        if ret_base != ann_base {
            continue;
        }

        // The return expression uses `Never` as a type argument.
        if ret_inner != "Never" {
            continue;
        }

        // The target type parameter is not `Never` or `Any`.
        if ann_inner == "Never" || ann_inner == "Any" {
            continue;
        }

        // Check if the annotation's type parameter is a covariant TypeVar.
        if covariant_tvars.contains(&ann_inner) {
            continue;
        }

        // Check if the class itself declares a covariant type parameter.
        if is_class_param_covariant(ann_base, module, covariant_tvars) {
            continue;
        }

        diagnostics.push(make_return_diagnostic(
            ret_stmt.span,
            &func.name,
            ann_text,
            ret_type_text,
            path,
        ));
    }
}

/// Check if a class's generic type parameter is covariant.
fn is_class_param_covariant(
    class_name: &str,
    module: &ResolvedModule,
    covariant_tvars: &[&str],
) -> bool {
    module
        .classes
        .iter()
        .filter(|cls| cls.name == class_name)
        .flat_map(|cls| &cls.generic_params)
        .any(|param| covariant_tvars.contains(&param.name.as_str()))
}

/// Extract the inner type parameter from a generic annotation.
///
/// `"list[int]"` -> `Some("int")`, `"ClassC[Never]"` -> `Some("Never")`
fn extract_generic_inner(text: &str) -> Option<&str> {
    let bracket_pos = text.find('[')?;
    let close_bracket = text.rfind(']')?;
    if close_bracket <= bracket_pos {
        return None;
    }
    Some(text.get(bracket_pos + 1..close_bracket)?.trim())
}

/// Extract the base name from a generic annotation.
///
/// `"list[int]"` -> `"list"`, `"ClassC[Never]"` -> `"ClassC"`
fn extract_generic_base(text: &str) -> &str {
    text.find('[')
        .map_or(text, |pos| text.get(..pos).unwrap_or(text).trim())
}

/// Strip trailing `()` call from an expression.
///
/// `"ClassC[Never]()"` -> `"ClassC[Never]"`
/// `"ClassC[Never](1, 2)"` -> `"ClassC[Never]"`
fn strip_call_parens(text: &str) -> &str {
    if let Some(stripped) = text.strip_suffix("()") {
        return stripped;
    }
    if let Some(pos) = text.find("](") {
        return text.get(..=pos).unwrap_or(text);
    }
    text
}

/// Check if a string looks like a simple Python identifier.
fn is_simple_identifier(text: &str) -> bool {
    basilisk_resolver::is_simple_ascii_python_identifier(text)
}

/// Get the byte offset of the start of line number `line_idx` (0-indexed).
fn line_byte_offset(source: &str, line_idx: usize) -> usize {
    let mut offset = 0;
    for (i, line) in source.lines().enumerate() {
        if i == line_idx {
            return offset;
        }
        offset += line.len() + 1; // +1 for '\n'
    }
    offset
}

fn make_assignment_diagnostic(
    span: Span,
    var_name: &str,
    annotation: &str,
    source_type: &str,
    path: &str,
) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Cannot assign `{source_type}` to `{var_name}` annotated `{annotation}`: \
             `Never` is only compatible with `Never` and `Any` in invariant contexts"
        ),
        span,
        path,
        Some("Change the annotation to `Never` or `Any`, or change the assigned value".to_owned()),
        Some(
            "PEP 484: `Never` is a bottom type and cannot be assigned to other types \
             except in covariant contexts or when the target is `Any`"
                .to_owned(),
        ),
    )
}

fn make_return_diagnostic(
    span: Span,
    func_name: &str,
    return_annotation: &str,
    return_expr: &str,
    path: &str,
) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Cannot return `{return_expr}` from `{func_name}` annotated \
             `-> {return_annotation}`: `Never` is only compatible with `Never` \
             and `Any` in invariant contexts"
        ),
        span,
        path,
        Some("Change the return type annotation or the returned value".to_owned()),
        Some(
            "PEP 484: `Never` is a bottom type and cannot substitute for invariant \
             type parameters"
                .to_owned(),
        ),
    )
}
