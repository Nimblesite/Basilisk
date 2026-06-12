//! Implements [BSK-E0033] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-ownership
//! BSK-E0033: Invalid `reveal_type()` call.
//!
//! `reveal_type(expr)` must be called with exactly one positional argument.
//!
//! - `reveal_type()` — too few arguments (0 given)
//! - `reveal_type(a, b)` — too many arguments (2 given)

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0033",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0033",
};

/// Emits BSK-E0033 for invalid `reveal_type()` calls.
pub(crate) struct InvalidRevealTypeCall;

impl Rule for InvalidRevealTypeCall {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for call in module.reveal_type_calls.iter().filter(|c| c.arg_count != 1) {
            let arg_count = call.arg_count;
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "`reveal_type()` requires exactly 1 argument, got {arg_count}"
                ),
                call.span,
                &module.path,
                Some("Usage: `reveal_type(expression)`".to_owned()),
                Some(
                    "reveal_type() is a static-analysis directive that takes exactly one expression"
                        .to_owned(),
                ),
            ));
        }
    }
}
