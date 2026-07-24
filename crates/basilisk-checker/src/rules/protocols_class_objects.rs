//! Implements [`protocols_class_objects`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `protocols_class_objects`: Protocol class used where `type[Proto]` is expected.
//!
//! The typing spec states: "Variables and parameters annotated with
//! `Type[Proto]` accept only concrete (non-protocol) subtypes of Proto."
//!
//! Passing the Protocol class itself (rather than a concrete subtype) violates
//! this constraint.
//!
//! ```python
//! class Proto(Protocol):
//!     def meth(self) -> int: ...
//!
//! class Concrete:
//!     def meth(self) -> int: return 42
//!
//! def fun(cls: type[Proto]) -> int:
//!     return cls().meth()
//!
//! fun(Proto)      # E0106 — Protocol class passed to type[Proto]
//! fun(Concrete)   # OK — concrete subtype
//!
//! var: type[Proto]
//! var = Proto     # E0106 — Protocol class assigned to type[Proto]
//! var = Concrete  # OK
//! ```

use basilisk_resolver::ResolvedModule;

use super::Rule;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

const CODE: ErrorCode = ErrorCode {
    code: "protocols_class_objects",
    docs_url: "https://www.basilisk-python.dev/errors/protocols_class_objects",
};

/// Emits `protocols_class_objects` when a Protocol class is used where `type[Proto]` is expected.
pub(crate) struct ProtocolClassObject;

impl Rule for ProtocolClassObject {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for violation in &module.protocol_class_object_violations {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Protocol class `{}` cannot be used where `type[{}]` is expected; \
                     only concrete (non-protocol) subtypes are accepted",
                    violation.protocol_name, violation.protocol_name
                ),
                violation.span,
                &module.path,
                Some(format!(
                    "Pass a concrete class that implements `{}` instead",
                    violation.protocol_name
                )),
                Some(
                    "Variables and parameters annotated with `type[Proto]` accept only \
                     concrete (non-protocol) subtypes of Proto"
                        .to_owned(),
                ),
            ));
        }
    }
}
