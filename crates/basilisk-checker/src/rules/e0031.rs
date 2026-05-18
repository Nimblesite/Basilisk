//! BSK-E0031: Invalid `cast()` call.
//!
//! `typing.cast(typ, val)` must be called with exactly two positional arguments,
//! and the first argument must be a type expression, not a value literal.
//!
//! - `cast()` — too few arguments
//! - `cast(1, x)` — first argument is a literal, not a type
//! - `cast(int, x, y)` — too many arguments

use basilisk_resolver::{ResolvedModule, RhsKind};

use crate::diagnostic::{Diagnostic, ErrorCode, error_diagnostic_owned};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0031",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0031",
};

/// Emits BSK-E0031 for invalid `cast()` calls.
pub(crate) struct InvalidCastCall;

impl Rule for InvalidCastCall {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for call in module.calls.iter().filter(|c| c.callee == "cast") {
            let arg_count = call.args.len();
            if arg_count == 2 {
                // Exactly 2 args: check that first arg is not a plain value literal.
                let Some((first_kind, first_span)) = call.args.first() else {
                    continue;
                };
                if matches!(
                    first_kind,
                    RhsKind::IntLiteral
                        | RhsKind::FloatLiteral
                        | RhsKind::StrLiteral
                        | RhsKind::BoolLiteral
                        | RhsKind::BytesLiteral
                        | RhsKind::NoneValue
                ) {
                    diagnostics.push(error_diagnostic_owned(
                        CODE.clone(),
                        "First argument of `cast()` must be a type, not a value literal"
                            .to_owned(),
                        *first_span,
                        &module.path,
                        Some(
                            "Replace the literal with a type expression, e.g. `cast(int, val)`"
                                .to_owned(),
                        ),
                        Some(
                            "PEP 484: the first argument to `cast()` must be a type expression"
                                .to_owned(),
                        ),
                    ));
                }
            } else {
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "`cast()` requires exactly 2 arguments (type, value), got {arg_count}"
                    ),
                    call.span,
                    &module.path,
                    Some("Usage: `cast(Type, expression)`".to_owned()),
                    Some(
                        "PEP 484: `cast(typ, val)` takes exactly two positional arguments"
                            .to_owned(),
                    ),
                ));
            }
        }
    }
}
