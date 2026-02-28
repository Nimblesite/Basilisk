//! BSK-E0070: `Never` type compatibility violations.
//!
//! Detects type compatibility errors involving the `Never` bottom type:
//!
//! 1. Assigning `list[Never]` to `list[T]` where `T` is not `Never` or `Any`
//! 2. Using `ClassC[Never]` where `ClassC[T]` is invariant (not covariant)
//!
//! ```python
//! from typing import Never, Any, Generic, TypeVar
//!
//! T = TypeVar("T")
//! U = TypeVar("U")
//!
//! def func(c: list[Never]):
//!     v: list[int] = c  # E0070 — Never is not compatible with int
//!
//! class ClassC(Generic[T]):
//!     pass
//!
//! def func2(x: U) -> ClassC[U]:
//!     return ClassC[Never]()  # E0070 — ClassC is invariant
//! ```

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0070",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0070",
};

/// Emits BSK-E0070 for Never type compatibility violations.
pub(crate) struct NeverTypeCompatibility;

impl Rule for NeverTypeCompatibility {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        // Check annotated variable assignments for Never compatibility issues
        for var in &module.module_vars {
            if let Some((ann_span, rhs_span)) = var.annotation_span.zip(var.rhs_span) {
                if let (Some(ann_text), Some(rhs_text)) = (
                    source.get(ann_span.start as usize..ann_span.end as usize),
                    source.get(rhs_span.start as usize..rhs_span.end as usize),
                ) {
                    check_assignment_compatibility(
                        ann_text.trim(),
                        rhs_text.trim(),
                        var.name_span,
                        var.name.as_str(),
                        path,
                        diagnostics,
                    );
                }
            }
        }

        // Check return statements for Never compatibility issues
        for func in &module.functions {
            for ret_stmt in &func.return_stmts {
                if let Some(ret_text) = source.get(ret_stmt.span.start as usize..ret_stmt.span.end as usize) {
                    if let Some(ann_text) = func.return_annotation_span.and_then(|span| {
                        source.get(span.start as usize..span.end as usize)
                    }) {
                        check_return_compatibility(
                            ann_text.trim(),
                            ret_text.trim(),
                            ret_stmt.span,
                            func.name.as_str(),
                            path,
                            diagnostics,
                        );
                    }
                }
            }
        }
    }
}

/// Check if an assignment involves Never type compatibility issues
fn check_assignment_compatibility(
    annotation: &str,
    rhs: &str,
    span: Span,
    var_name: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Check for list[Never] assignment to list[T] where T != Never
    if let Some(target_type) = extract_list_type(annotation) {
        if let Some(source_type) = extract_list_type(rhs) {
            if source_type == "Never" && target_type != "Never" && target_type != "Any" {
                diagnostics.push(make_assignment_diagnostic(
                    span,
                    var_name,
                    annotation,
                    rhs,
                    path,
                ));
            }
        }
    }

    // Check for generic type with Never assignment to generic type with different parameter
    if let Some((target_generic, target_param)) = extract_generic_type(annotation) {
        if let Some((source_generic, source_param)) = extract_generic_type(rhs) {
            if source_generic == target_generic && source_param == "Never" && target_param != "Never" && target_param != "Any" {
                diagnostics.push(make_assignment_diagnostic(
                    span,
                    var_name,
                    annotation,
                    rhs,
                    path,
                ));
            }
        }
    }
}

/// Check if a return statement involves Never type compatibility issues
fn check_return_compatibility(
    return_annotation: &str,
    return_expr: &str,
    span: Span,
    func_name: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Check for generic type with Never return to generic type with different parameter
    if let Some((target_generic, target_param)) = extract_generic_type(return_annotation) {
        if let Some((source_generic, source_param)) = extract_generic_type(return_expr) {
            if source_generic == target_generic && source_param == "Never" && target_param != "Never" && target_param != "Any" {
                diagnostics.push(make_return_diagnostic(
                    span,
                    func_name,
                    return_annotation,
                    return_expr,
                    path,
                ));
            }
        }
    }
}

/// Extract the inner type from a list annotation like "list[int]" -> "int"
fn extract_list_type(annotation: &str) -> Option<&str> {
    if annotation.starts_with("list[") && annotation.ends_with(']') {
        let inner = &annotation[5..annotation.len() - 1];
        Some(inner.trim())
    } else {
        None
    }
}

/// Extract generic type and parameter like "ClassC[Never]" -> ("ClassC", "Never")
fn extract_generic_type(annotation: &str) -> Option<(&str, &str)> {
    if let Some(bracket_pos) = annotation.find('[') {
        if annotation.ends_with(']') {
            let generic_name = &annotation[..bracket_pos];
            let param = &annotation[bracket_pos + 1..annotation.len() - 1];
            Some((generic_name.trim(), param.trim()))
        } else {
            None
        }
    } else {
        None
    }
}

fn make_assignment_diagnostic(
    span: Span,
    var_name: &str,
    annotation: &str,
    rhs: &str,
    path: &str,
) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Cannot assign `{rhs}` to `{var_name}` annotated `{annotation}`: \
             `Never` is only compatible with `Never` and `Any` in invariant contexts"
        ),
        span,
        path: path.to_owned(),
        help: Some(
            "Change the annotation to `Never` or `Any`, or change the assigned value".to_owned(),
        ),
        note: Some(
            "PEP 484: `Never` is a bottom type and cannot be assigned to other types \
             except in covariant contexts or when the target is `Any`".to_owned(),
        ),
    }
}

fn make_return_diagnostic(
    span: Span,
    func_name: &str,
    return_annotation: &str,
    return_expr: &str,
    path: &str,
) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Cannot return `{return_expr}` from `{func_name}` annotated `{return_annotation}`: \
             `Never` is only compatible with `Never` and `Any` in invariant contexts"
        ),
        span,
        path: path.to_owned(),
        help: Some(
            "Change the return type annotation to `Never` or `Any`, or change the returned value".to_owned(),
        ),
        note: Some(
            "PEP 484: `Never` is a bottom type and cannot be returned as other types \
             except in covariant contexts or when the target is `Any`".to_owned(),
        ),
    }
}