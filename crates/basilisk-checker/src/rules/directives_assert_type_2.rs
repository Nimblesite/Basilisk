//! Implements [`directives_assert_type_2`] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
//! `directives_assert_type_2`: `assert_type()` type mismatch.
//!
//! `assert_type(expr, Type)` is a static-analysis directive that verifies the
//! inferred type of `expr` equals `Type`.  When the resolver can determine both
//! sides and they do not match, this rule emits an error.
//!
//! ```python
//! from typing import assert_type
//!
//! def f(a: int | str) -> None:
//!     assert_type(a, int)  # E — int | str is not int
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "directives_assert_type_2",
    docs_url: "https://www.basilisk-python.dev/errors/directives_assert_type_2",
};

/// Emits `directives_assert_type_2` when `assert_type(expr, T)` has a detectable type mismatch.
///
/// Currently disabled — requires full type inference to avoid false positives.
/// Re-enable in `mod.rs` `run_all()` once the type engine is in place.
pub(crate) struct AssertTypeMismatch;

impl Rule for AssertTypeMismatch {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for call in module
            .assert_type_calls
            .iter()
            .filter(|c| c.arg_count == 2 && c.type_mismatch)
        {
            let actual = call.actual_type.as_deref().unwrap_or("unknown");
            let expected = call.expected_type.as_deref().unwrap_or("unknown");
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Type mismatch in `assert_type()`: expression has type `{actual}` but expected `{expected}`"
                ),
                call.span,
                &module.path,
                Some(
                    "The type of the expression does not match the declared expected type"
                        .to_owned(),
                ),
                Some(
                    "assert_type(expr, T) requires the inferred type of expr to be exactly T"
                        .to_owned(),
                ),
            ));
        }
    }
}
