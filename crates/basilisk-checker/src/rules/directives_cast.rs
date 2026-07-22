//! Implements [`directives_cast`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
//! `directives_cast`: Invalid `cast()` call.
//!
//! `typing.cast(typ, val)` must be called with exactly two positional arguments,
//! and the first argument must be a type expression, not a value literal. A
//! *quoted* first argument (`cast("Widget", x)`) is NOT a value literal — it is
//! the standard PEP 484 forward-reference spelling, which typeshed admits
//! directly (`cast(typ: type[_T] | str | Any, val)`) and which ruff's `TC006`
//! actively requires — so only genuine non-string value literals are rejected
//! (issue #335).
//!
//! - `cast()` — too few arguments
//! - `cast(1, x)` — first argument is a value literal, not a type
//! - `cast("Widget", x)` — OK: string forward reference
//! - `cast(int, x, y)` — too many arguments

use basilisk_resolver::{ResolvedModule, RhsKind};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "directives_cast",
    docs_url: "https://www.basilisk-python.dev/errors/directives_cast",
};

/// Emits `directives_cast` for invalid `cast()` calls.
pub(crate) struct InvalidCastCall;

impl Rule for InvalidCastCall {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for call in module.calls.iter().filter(|c| c.callee == "cast") {
            let arg_count = call.args.len();
            if arg_count == 2 {
                // Exactly 2 args: reject a first argument that is a genuine value
                // literal (number, bool, bytes, or None). A string is deliberately
                // NOT in this set — `cast("Widget", x)` is a forward reference, the
                // spelling typeshed admits (`type[_T] | str | Any`) and ruff `TC006`
                // requires, so flagging it would be a false positive (issue #335).
                let Some((first_kind, first_span)) = call.args.first() else {
                    continue;
                };
                if matches!(
                    first_kind,
                    RhsKind::IntLiteral
                        | RhsKind::FloatLiteral
                        | RhsKind::BoolLiteral
                        | RhsKind::BytesLiteral
                        | RhsKind::NoneValue
                ) {
                    diagnostics.push(error_diagnostic_owned(
                        CODE.clone(),
                        "First argument of `cast()` must be a type, not a value literal".to_owned(),
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
                    format!("`cast()` requires exactly 2 arguments (type, value), got {arg_count}"),
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
