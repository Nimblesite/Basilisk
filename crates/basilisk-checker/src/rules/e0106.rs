//! BSK-E0106: Protocol class used where `type[Proto]` is expected.
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
use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0106",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0106",
};

/// Emits BSK-E0106 when a Protocol class is used where `type[Proto]` is expected.
pub(crate) struct ProtocolClassObject;

impl Rule for ProtocolClassObject {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for violation in &module.protocol_class_object_violations {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Protocol class `{}` cannot be used where `type[{}]` is expected; \
                     only concrete (non-protocol) subtypes are accepted",
                    violation.protocol_name, violation.protocol_name
                ),
                span: violation.span,
                path: module.path.clone(),
                help: Some(format!(
                    "Pass a concrete class that implements `{}` instead",
                    violation.protocol_name
                )),
                note: Some(
                    "Variables and parameters annotated with `type[Proto]` accept only \
                     concrete (non-protocol) subtypes of Proto"
                        .to_owned(),
                ),
                provenance: None,
            });
        }
    }
}
