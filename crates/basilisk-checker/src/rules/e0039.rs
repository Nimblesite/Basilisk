//! BSK-E0039: Invalid `assert_type()` call.
//!
//! `assert_type(expr, Type)` must be called with exactly 2 positional arguments.
//!
//! - `assert_type()` — too few arguments (0 given)
//! - `assert_type(x)` — too few arguments (1 given)
//! - `assert_type(x, int, extra)` — too many arguments (3 given)

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0039",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0039",
};

/// Emits BSK-E0039 for invalid `assert_type()` calls.
pub(crate) struct InvalidAssertTypeCall;

impl Rule for InvalidAssertTypeCall {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for call in module.assert_type_calls.iter().filter(|c| c.arg_count != 2) {
            let arg_count = call.arg_count;
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "`assert_type()` requires exactly 2 arguments (value, type), got {arg_count}"
                ),
                span: call.span,
                path: module.path.clone(),
                help: Some("Usage: `assert_type(expression, ExpectedType)`".to_owned()),
                note: Some(
                    "assert_type() is a static-analysis directive that takes an expression \
                     and its expected type"
                        .to_owned(),
                ),
            });
        }
    }
}
