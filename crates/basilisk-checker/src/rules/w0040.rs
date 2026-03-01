//! BSK-W0040: Lambda function missing type annotations.
//!
//! Emitted when a lambda function has parameters or return values without type annotations.
//! This is a warning rather than an error since lambda functions are often used for simple
//! operations where type annotations might be considered verbose.
//!
//! ```python
//! # BAD (warning)
//! f = lambda x: x + 1  # W: lambda parameter 'x' has no type annotation
//!
//! # GOOD
//! f = lambda x: int: x + 1  # OK: parameter has type annotation
//! ```

use basilisk_resolver::{LambdaInfo, ResolvedModule};
use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::{guards::is_stub_context, Rule};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-W0040",
    docs_url: "https://basilisk-lang.org/warnings/BSK-W0040",
};

/// Emits BSK-W0040 when lambda functions have missing type annotations.
pub(crate) struct LambdaMissingAnnotations;

impl Rule for LambdaMissingAnnotations {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for lambda in &module.lambdas {
            if is_stub_context(lambda, &module.classes) {
                continue;
            }
            
            check_lambda_annotations(lambda, &module.path, diagnostics);
        }
    }
}

fn check_lambda_annotations(lambda: &LambdaInfo, path: &str, out: &mut Vec<Diagnostic>) {
    // Check for unannotated parameters
    for param in &lambda.params {
        if !param.has_annotation {
            out.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Warning,
                message: format!("lambda parameter '{}' has no type annotation", param.name),
                span: param.name_span,
                path: path.to_owned(),
                help: Some("Consider adding a type annotation to improve code clarity".to_owned()),
                note: Some("Lambda functions are exempt from strict type requirements but annotations help readability".to_owned()),
            });
        }
    }
    
    // Check for missing return annotation (if the lambda has a return type hint)
    // Note: Lambdas don't typically have explicit return annotations in Python,
    // but we can warn if there's an opportunity to add clarity
    if lambda.has_return_hint && !lambda.return_annotation.is_present() {
        out.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Warning,
            message: "lambda function return type could be annotated for clarity".to_owned(),
            span: lambda.span,
            path: path.to_owned(),
            help: Some("Consider adding a return type hint if the lambda's purpose isn't immediately clear".to_owned()),
            note: Some("This is a warning only - lambda functions are often used for simple operations".to_owned()),
        });
    }
}