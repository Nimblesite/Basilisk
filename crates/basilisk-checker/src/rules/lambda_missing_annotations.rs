//! Implements [BSK-0040] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
//! BSK-0040: Lambda function missing type annotations.
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
    code: "BSK-0040",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-0040",
};

/// Emits BSK-0040 when lambda functions are assigned to unannotated variables.
// Implements [TYPEINF-FUNC-LAMBDA] / [TYPEINF-EXCEEDS-LAMBDA] — warn (BSK-0040)
// when a lambda's parameter types cannot be inferred from an expected type;
// an annotated target supplies that context and is accepted.
pub(crate) struct LambdaMissingAnnotations;

impl Rule for LambdaMissingAnnotations {
    fn opt_in_spec(&self) -> Option<crate::rule_tags::OptInSpec> {
        Some(crate::rule_tags::OptInSpec {
            code: CODE.code,
            tags: &["strictness"],
        })
    }

    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
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
            // Enum bodies legitimately assign bare lambdas as non-member
            // callables (e.g. a `converter`); the typing spec discourages
            // annotating them, so a missing-annotation nudge here is a false
            // positive (conformance enums_members.py).
            if class.is_enum {
                continue;
            }
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
