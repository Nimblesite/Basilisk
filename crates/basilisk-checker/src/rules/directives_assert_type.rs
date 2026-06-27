//! Implements [directives_assert_type] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-ownership
//! directives_assert_type: Invalid `assert_type()` call.
//!
//! `assert_type(expr, Type)` must be called with exactly 2 positional arguments.
//!
//! - `assert_type()` — too few arguments (0 given)
//! - `assert_type(x)` — too few arguments (1 given)
//! - `assert_type(x, int, extra)` — too many arguments (3 given)

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "directives_assert_type",
    docs_url: "https://www.basilisk-python.dev/errors/directives_assert_type",
};

/// Emits directives_assert_type for invalid `assert_type()` calls.
pub(crate) struct InvalidAssertTypeCall;

impl Rule for InvalidAssertTypeCall {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for call in module.assert_type_calls.iter().filter(|c| c.arg_count != 2) {
            let arg_count = call.arg_count;
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "`assert_type()` requires exactly 2 arguments (value, type), got {arg_count}"
                ),
                call.span,
                &module.path,
                Some("Usage: `assert_type(expression, ExpectedType)`".to_owned()),
                Some(
                    "assert_type() is a static-analysis directive that takes an expression \
                     and its expected type"
                        .to_owned(),
                ),
            ));
        }
    }
}
