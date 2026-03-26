//! BSK-E0013: Return type mismatch — inferred return type incompatible with annotation.
//!
//! When a function has a return type annotation, the inferred return type must be
//! assignable to the declared type. This extends the original `-> None` check to
//! handle all return type mismatches using the inference system.

use crate::inference::infer_rhs;
use crate::span_util::slice_span;
use crate::types::InferredType;
use basilisk_resolver::{FunctionInfo, ResolvedModule, ReturnStmtInfo};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0013",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0013",
};

/// Emits BSK-E0013 for return type mismatches using inference system.
pub(crate) struct ReturnTypeMismatch;

impl Rule for ReturnTypeMismatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        module
            .functions
            .iter()
            .filter(|func| func.return_annotation.is_present())
            .for_each(|func| check_function(func, module, diagnostics));
    }
}

fn check_function(func: &FunctionInfo, module: &ResolvedModule, out: &mut Vec<Diagnostic>) {
    // Generator functions have their own return type validation (E0120).
    if func.is_generator {
        return;
    }

    // Dunder methods with special return-type semantics that E0013 cannot
    // properly validate (e.g. `__exit__` returns bool for exception
    // suppression, `__new__` returns the class or a union, `__call__` on
    // metaclasses returns instances).
    if is_special_return_dunder(&func.name) {
        return;
    }

    let Some(ann_span) = func.return_annotation_span else {
        return;
    };
    let Some(ann_text) = slice_span(&module.source, ann_span) else {
        return;
    };

    let declared_type = InferredType::from_annotation(ann_text);

    // Named types require structural subtyping or generic variance analysis
    // that E0013 cannot perform.  Skip to avoid FPs.
    if matches!(declared_type, InferredType::Named(_)) {
        return;
    }

    // Literal return types (e.g. `-> Literal[True]`) require value-level
    // inference that E0013 does not perform — `infer_rhs` returns the base
    // type (`Bool`) not the literal value.  Skip to avoid FPs.
    if contains_literal(&declared_type) {
        return;
    }

    // Special case for -> None functions: any valued return should be flagged
    if declared_type == InferredType::None_ {
        func.return_stmts
            .iter()
            .filter(|stmt| stmt.has_value)
            // Skip call expressions: without full type inference we cannot prove the
            // callee returns non-None (e.g. `return f(self)` where f: Callable[..., None]
            // is valid in a -> None function).
            .filter(|stmt| !stmt.value_is_call)
            .for_each(|stmt| {
                out.push(make_none_diagnostic(stmt, &func.name, &module.path));
            });
        return;
    }

    func.return_stmts
        .iter()
        .filter(|stmt| stmt.has_value)
        // Skip call expressions: without full type inference we cannot prove the
        // callee returns non-None (e.g. `return f(self)` where f: Callable[..., None]
        // is valid in a -> None function).
        .filter(|stmt| !stmt.value_is_call)
        .for_each(|stmt| {
            // Use inference system to get RHS type
            let inferred_type = infer_rhs(&stmt.rhs_kind);

            // Skip Unknown types - we can't prove they're incompatible
            if matches!(inferred_type, InferredType::Unknown) {
                return;
            }

            // Check assignability using inference system
            if !inferred_type.is_assignable_to(&declared_type) {
                out.push(make_diagnostic(
                    stmt,
                    &func.name,
                    &inferred_type,
                    &declared_type,
                    &module.path,
                ));
            }
        });
}

fn make_none_diagnostic(stmt: &ReturnStmtInfo, func_name: &str, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Function `{func_name}` is annotated `-> None` but has a `return` statement with a value"
        ),
        span: stmt.span,
        path: path.to_owned(),
        help: Some(
            "Either remove the return value or change the return type annotation".to_owned(),
        ),
        note: Some(
            "A function annotated `-> None` must only use bare `return` or fall off the end"
                .to_owned(),
        ),
    }
}

fn make_diagnostic(
    stmt: &ReturnStmtInfo,
    func_name: &str,
    inferred_type: &InferredType,
    declared_type: &InferredType,
    path: &str,
) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Function `{func_name}` return type mismatch: {inferred_type} is not assignable to {declared_type}"
        ),
        span: stmt.span,
        path: path.to_owned(),
        help: Some(
            "Either change the return value or update the return type annotation".to_owned(),
        ),
        note: Some(
            "Basilisk requires the inferred return type to be assignable to the declared type".to_owned(),
        ),
    }
}

/// Dunder methods whose return types have special semantics that simple
/// return-value inference cannot validate.
fn is_special_return_dunder(name: &str) -> bool {
    matches!(
        name,
        "__new__"
            | "__exit__"
            | "__aexit__"
            | "__call__"
            | "__init_subclass__"
            | "__class_getitem__"
    )
}

/// Returns `true` if the type contains a `Literal` variant anywhere.
fn contains_literal(ty: &InferredType) -> bool {
    match ty {
        InferredType::Literal(_) => true,
        InferredType::Union(types) => types.iter().any(contains_literal),
        InferredType::Optional(inner) => contains_literal(inner),
        InferredType::Tuple(elems) => elems.iter().any(contains_literal),
        _ => false,
    }
}
