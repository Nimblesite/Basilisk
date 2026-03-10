//! BSK-E0099: Direct instantiation of a Protocol class.
//!
//! Protocol classes define structural interfaces and cannot be instantiated
//! directly. Only concrete classes that satisfy the protocol may be
//! instantiated.
//!
//! ```python
//! from typing import Protocol
//!
//! class MyProto(Protocol):
//!     def method(self) -> int: ...
//!
//! obj = MyProto()  # E — cannot instantiate a Protocol
//! ```

use basilisk_resolver::ResolvedModule;

use super::Rule;
use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0099",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0099",
};

/// Emits BSK-E0099 for direct instantiation of Protocol classes.
pub(crate) struct ProtocolInstantiation;

impl Rule for ProtocolInstantiation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for violation in &module.protocol_instantiation_violations {
            let message = if violation.is_abstract {
                format!(
                    "Cannot instantiate `{}`; it does not implement all required protocol members",
                    violation.class_name
                )
            } else {
                format!(
                    "Cannot instantiate Protocol class `{}`; \
                     Protocols define interfaces, not concrete implementations",
                    violation.class_name
                )
            };

            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message,
                span: violation.span,
                path: module.path.clone(),
                help: Some(
                    "Create a concrete class that implements the Protocol, then instantiate that"
                        .to_owned(),
                ),
                note: Some(
                    "PEP 544: Protocol classes are structural type definitions \
                     and cannot be instantiated directly"
                        .to_owned(),
                ),
            });
        }
    }
}
