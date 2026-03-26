//! BSK-E0053: `assert_type()` type mismatch.
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

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0053",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0053",
};

/// Emits BSK-E0053 when `assert_type(expr, T)` has a detectable type mismatch.
pub(crate) struct AssertTypeMismatch;

impl Rule for AssertTypeMismatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for call in module
            .assert_type_calls
            .iter()
            .filter(|c| c.arg_count == 2 && c.type_mismatch)
        {
            let actual = call.actual_type.as_deref().unwrap_or("unknown");
            let expected = call.expected_type.as_deref().unwrap_or("unknown");

            // Skip when either type is unknown — cannot verify.
            if actual == "unknown" || expected == "unknown" {
                continue;
            }

            // When both types are identical modulo case, skip (resolver might
            // have a case-sensitivity issue in matching).
            if actual.eq_ignore_ascii_case(expected) {
                continue;
            }

            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Type mismatch in `assert_type()`: expression has type `{actual}` but expected `{expected}`"
                ),
                span: call.span,
                path: module.path.clone(),
                help: Some(
                    "The type of the expression does not match the declared expected type"
                        .to_owned(),
                ),
                note: Some(
                    "assert_type(expr, T) requires the inferred type of expr to be exactly T"
                        .to_owned(),
                ),
            });
        }
    }
}
