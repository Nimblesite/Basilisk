//! Implements [BSK-E0077] from [CHKARCH-DIAG-OPTIONAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-optional
//! BSK-E0077: Protocol `Self`-return conformance violation.
//!
//! When a `Protocol` declares a method returning `Self`, any class passed where
//! that protocol is expected must have the corresponding method return `Self` or
//! the class itself.  If the method returns a completely different type (e.g.
//! `int` or a different class), the class does not satisfy the protocol.
//!
//! ```python
//! class ShapeProtocol(Protocol):
//!     def set_scale(self, scale: float) -> Self: ...
//!
//! class BadReturn:
//!     def set_scale(self, scale: float) -> int:
//!         return 42
//!
//! def accepts(s: ShapeProtocol) -> None: ...
//!
//! def main(bad: BadReturn) -> None:
//!     accepts(bad)  # E — BadReturn.set_scale returns int, not Self
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0077",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0077",
};

/// Emits BSK-E0077 for classes passed where a `Protocol` with `Self`-returning
/// methods is expected, but the class's corresponding method returns a different type.
pub(crate) struct ProtocolSelfViolation;

impl Rule for ProtocolSelfViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for violation in &module.protocol_self_violations {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Class `{}` is not compatible with protocol `{}`: \
                     method `{}` returns `{}` instead of `Self`",
                    violation.class_name,
                    violation.protocol_name,
                    violation.method_name,
                    violation.actual_return_type
                ),
                violation.span,
                &module.path,
                Some(format!(
                    "Change `{}.{}` to return `Self` or `{}`",
                    violation.class_name, violation.method_name, violation.class_name
                )),
                Some(
                    "Protocol methods returning `Self` require implementing classes to \
                     return `Self` or the concrete class type, not an unrelated type"
                        .to_owned(),
                ),
            ));
        }
    }
}
