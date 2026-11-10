//! Implements [`directives_reveal_type`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
//! `directives_reveal_type`: Invalid `reveal_type()` call.
//!
//! `reveal_type(expr)` must be called with exactly one positional argument.
//! The canonical [typing directive specification](https://typing.python.org/en/latest/spec/directives.html#reveal-type),
//! built on [PEP 484](https://peps.python.org/pep-0484/), also requires a valid
//! call to emit a diagnostic containing the inferred static type. This module
//! currently owns only arity validation; the missing reporting behavior is
//! pinned by [#418](https://github.com/Nimblesite/Basilisk/issues/418) in
//! `tests/runtime_type_safety_regressions_tests.rs`.
//!
//! - `reveal_type()` — too few arguments (0 given)
//! - `reveal_type(a, b)` — too many arguments (2 given)

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "directives_reveal_type",
    docs_url: "https://www.basilisk-python.dev/errors/directives_reveal_type",
};

/// Emits `directives_reveal_type` for invalid `reveal_type()` calls.
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
