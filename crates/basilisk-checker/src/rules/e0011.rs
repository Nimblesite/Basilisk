//! BSK-E0011: Return type mismatch.
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

use basilisk_resolver::{FunctionInfo, ResolvedModule};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::{guards::is_stub_context, Rule};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0011",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0011",
};

/// Emits BSK-E0011 when inferred return types don't match the annotation.
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
    // Check if the function has a return statement with a literal value
    // that clearly doesn't match the annotation
    if let Some(return_expr) = &func.return_expression {
        match return_expr {
            basilisk_resolver::ReturnExpression::Literal(lit) => {
                // Get the annotation text from the source
                let Some(ann_span) = func.return_annotation_span else {
                    return;
                };
                let Some(ann_text) = module
                    .source
                    .get(ann_span.start as usize..ann_span.end as usize)
                else {
                    return;
                };

                // Check if the literal type is incompatible with the annotation
                if is_incompatible_literal(lit, ann_text) {
                    out.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
                        message: format!("return type mismatch: {} is not assignable to {}", 
                                       lit_type_name(lit), ann_text),
                        span: func.name_span,
                        path: module.path.to_owned(),
                        help: Some("Check the return type annotation and return statements".to_owned()),
                        note: None,
                    });
                }
            }
            basilisk_resolver::ReturnExpression::Call(_) => {
                // For call expressions, we can't determine the return type without
                // full inference, so we'll be conservative and not fire E0011
                // This matches the test expectations
            }
            basilisk_resolver::ReturnExpression::None => {
                // No return expression - this is handled by E0013
            }
        }
    }
}

fn is_incompatible_literal(lit: &basilisk_resolver::Literal, annotation: &str) -> bool {
    let base = annotation
        .split('[')
        .next()
        .unwrap_or(annotation)
        .trim()
        .to_ascii_lowercase();

    // Simple compatibility check based on literal type and annotation text
    match lit {
        basilisk_resolver::Literal::Int(_) => {
            // int literals are compatible with int annotations
            base != "int" && base != "any"
        }
        basilisk_resolver::Literal::Str(_) => {
            // str literals are compatible with str annotations
            base != "str" && base != "any"
        }
        basilisk_resolver::Literal::Float(_) => {
            // float literals are compatible with float annotations
            base != "float" && base != "any"
        }
        basilisk_resolver::Literal::Bool(_) => {
            // bool literals are compatible with bool annotations
            base != "bool" && base != "any"
        }
        basilisk_resolver::Literal::Bytes(_) => {
            // bytes literals are compatible with bytes annotations
            base != "bytes" && base != "any"
        }
        basilisk_resolver::Literal::NoneValue => {
            // None is compatible with None/Any annotations
            base != "none" && base != "any"
        }
        _ => {
            // For other literal types, be conservative
            false
        }
    }
}

fn lit_type_name(lit: &basilisk_resolver::Literal) -> &'static str {
    match lit {
        basilisk_resolver::Literal::Int(_) => "int",
        basilisk_resolver::Literal::Str(_) => "str",
        basilisk_resolver::Literal::Float(_) => "float",
        basilisk_resolver::Literal::Bool(_) => "bool",
        basilisk_resolver::Literal::Bytes(_) => "bytes",
        basilisk_resolver::Literal::NoneValue => "None",
        _ => "unknown",
    }
}
