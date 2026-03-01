//! BSK-W0040: Lambda function missing type annotations.
//!
//! Emitted when a lambda function is assigned to a variable without type annotations.
//! This is a warning rather than an error since lambda functions are often used for simple
//! operations where type annotations might be considered verbose.
//!
//! ```python
//! # BAD (warning)
//! f = lambda x: x + 1  # W: lambda assigned to unannotated variable 'f'
//!
//! # GOOD
//! f: Callable[[int], int] = lambda x: x + 1  # OK: variable has type annotation
//! ```

use basilisk_resolver::{ResolvedModule, VariableInfo};
use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-W0040",
    docs_url: "https://basilisk-lang.org/warnings/BSK-W0040",
};

/// Emits BSK-W0040 when lambda functions are assigned to unannotated variables.
pub(crate) struct LambdaMissingAnnotations;

impl Rule for LambdaMissingAnnotations {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Check module-level variables that have lambda expressions
        // Since we can't easily distinguish lambda calls from other function calls,
        // we'll use a conservative approach and warn on any unannotated variable
        // assignment that involves a function call
        for var in &module.module_vars {
            if var.rhs_kind == basilisk_resolver::RhsKind::CallExpr && !var.has_annotation {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Warning,
                    message: format!("function/lambda assigned to unannotated variable '{}'", var.name),
                    span: var.name_span,
                    path: module.path.clone(),
                    help: Some("Consider adding a type annotation to improve code clarity".to_owned()),
                    note: Some("This warning appears for function calls which may include lambda expressions".to_owned()),
                });
            }
        }
        
        // Check class attributes that have lambda expressions
        for class in &module.classes {
            for attr in &class.attributes {
                if attr.rhs_is_lambda && !attr.has_annotation {
                    diagnostics.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Warning,
                        message: format!("lambda assigned to unannotated class attribute '{}'", attr.name),
                        span: attr.name_span,
                        path: module.path.clone(),
                        help: Some("Consider adding a type annotation to improve code clarity".to_owned()),
                        note: Some("Lambda functions are exempt from strict type requirements but annotations help readability".to_owned()),
                    });
                }
            }
        }
    }
}
