//! BSK-E0013: Return type mismatch — inferred return type incompatible with annotation.
//!
//! When a function has a return type annotation, the inferred return type must be
//! assignable to the declared type. This extends the original `-> None` check to
//! handle all return type mismatches using the inference system.

use basilisk_resolver::{FunctionInfo, ResolvedModule, ReturnStmtInfo};
use crate::inference::infer_rhs;
use crate::types::InferredType;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0013",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0013",
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
    let Some(ann_span) = func.return_annotation_span else {
        return;
    };
    let Some(ann_text) = module
        .source
        .get(ann_span.start as usize..ann_span.end as usize)
    else {
        return;
    };

    // Parse annotation text to InferredType
    let declared_type = InferredType::from_annotation(ann_text);

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
            out.push(make_diagnostic(stmt, &func.name, &inferred_type, &declared_type, &module.path));
        }
        });
}

fn make_none_diagnostic(
    stmt: &ReturnStmtInfo, 
    func_name: &str, 
    path: &str
) -> Diagnostic {
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
    path: &str
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
