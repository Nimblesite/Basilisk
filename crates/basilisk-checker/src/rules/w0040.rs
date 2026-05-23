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

use basilisk_resolver::{ResolvedModule, RhsKind};

use crate::diagnostic::{warning_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-W0040",
    docs_url: "https://www.basilisk-python.dev/warnings/BSK-W0040",
};

/// Emits BSK-W0040 when lambda functions are assigned to unannotated variables.
pub(crate) struct LambdaMissingAnnotations;

impl Rule for LambdaMissingAnnotations {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Module-level: `f = lambda x: x + 1` without annotation
        for var in &module.module_vars {
            if var.rhs_kind == RhsKind::Lambda && !var.has_annotation {
                diagnostics.push(warning_diagnostic_owned(
                    CODE.clone(),
                    format!("lambda assigned to unannotated variable '{}'", var.name),
                    var.name_span,
                    &module.path,
                    Some(
                        "Add a type annotation such as `Callable[[int], int]` to improve type safety"
                            .to_owned(),
                    ),
                    None,
                ));
            }
        }

        // Class attributes: `converter = lambda x: str(x)` without annotation
        for class in &module.classes {
            for attr in &class.attributes {
                if attr.rhs_is_lambda && !attr.has_annotation {
                    diagnostics.push(warning_diagnostic_owned(
                        CODE.clone(),
                        format!(
                            "lambda assigned to unannotated class attribute '{}'",
                            attr.name
                        ),
                        attr.name_span,
                        &module.path,
                        Some(
                            "Add a type annotation such as `Callable[[int], int]` to improve type safety"
                                .to_owned(),
                        ),
                        None,
                    ));
                }
            }
        }
    }
}
