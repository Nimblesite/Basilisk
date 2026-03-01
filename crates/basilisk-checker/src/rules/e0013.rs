//! BSK-E0013: Return type mismatch.
//!
//! Emitted when the inferred return type from function body expressions
//! is not assignable to the declared return type annotation.
//!
//! ```python
//! # BAD
//! def func() -> int:
//!     return "hello"  # E: inferred return type str is not assignable to int
//!
//! # GOOD  
//! def func() -> int:
//!     return 42  # OK: inferred return type int matches annotation
//! ```

use basilisk_resolver::{FunctionInfo, ResolvedModule, RhsKind};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::{guards::is_stub_context, Rule};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0013",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0013",
};

/// Emits BSK-E0013 when inferred return types don't match the annotation.
pub(crate) struct ReturnTypeMismatch;

impl Rule for ReturnTypeMismatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for func in &module.functions {
            if is_stub_context(func, &module.classes) {
                continue;
            }
            
            // Skip if no return annotation
            if !func.return_annotation.is_present() {
                continue;
            }
            
            check_function_return_types(func, module, diagnostics);
        }
    }
}

fn check_function_return_types(func: &FunctionInfo, module: &ResolvedModule, out: &mut Vec<Diagnostic>) {
    // For now, we'll implement a conservative approach that only fires E0013
    // when we can clearly detect a mismatch based on simple heuristics
    
    // Check if the function has return statements with literal values
    // that clearly don't match the annotation
    for return_stmt in &func.return_stmts {
        if !return_stmt.has_value {
            continue; // Skip return statements without values
        }
        
        // Get the annotation text from the source
        let Some(ann_span) = func.return_annotation_span else {
            continue;
        };
        let Some(ann_text) = module
            .source
            .get(ann_span.start as usize..ann_span.end as usize)
        else {
            continue;
        };

        // Check if the return statement has a literal value that's incompatible
        if is_incompatible_rhs_kind(&return_stmt.rhs_kind, ann_text) {
            out.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!("return type mismatch: {} is not assignable to {}", 
                               rhs_kind_type_name(&return_stmt.rhs_kind), ann_text),
                span: func.name_span,
                path: module.path.clone(),
                help: Some("Check the return type annotation and return statements".to_owned()),
                note: None,
            });
        }
    }
}

fn is_incompatible_rhs_kind(rhs_kind: &RhsKind, annotation: &str) -> bool {
    let base = annotation
        .split('[')
        .next()
        .unwrap_or(annotation)
        .trim()
        .to_ascii_lowercase();

    // Simple compatibility check based on rhs kind and annotation text
    match rhs_kind {
        RhsKind::IntLiteral => {
            // int literals are compatible with int annotations
            base != "int" && base != "any"
        }
        RhsKind::StrLiteral => {
            // str literals are compatible with str annotations
            base != "str" && base != "any"
        }
        RhsKind::FloatLiteral => {
            // float literals are compatible with float annotations
            base != "float" && base != "any"
        }
        RhsKind::BoolLiteral => {
            // bool literals are compatible with bool annotations
            base != "bool" && base != "any"
        }
        RhsKind::BytesLiteral => {
            // bytes literals are compatible with bytes annotations
            base != "bytes" && base != "any"
        }
        RhsKind::NoneValue => {
            // None is compatible with None/Any annotations
            base != "none" && base != "any"
        }
        _ => {
            // For other rhs kinds, be conservative
            false
        }
    }
}

fn rhs_kind_type_name(rhs_kind: &RhsKind) -> &'static str {
    match rhs_kind {
        RhsKind::IntLiteral => "int",
        RhsKind::StrLiteral => "str",
        RhsKind::FloatLiteral => "float",
        RhsKind::BoolLiteral => "bool",
        RhsKind::BytesLiteral => "bytes",
        RhsKind::NoneValue => "None",
        _ => "unknown",
    }
}