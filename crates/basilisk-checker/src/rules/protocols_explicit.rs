//! Implements [`protocols_explicit`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! `protocols_explicit`: Direct instantiation of a Protocol class.
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
use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

const CODE: ErrorCode = ErrorCode {
    code: "protocols_explicit",
    docs_url: "https://www.basilisk-python.dev/errors/protocols_explicit",
};

/// Emits `protocols_explicit` for direct instantiation of Protocol classes.
pub(crate) struct ProtocolInstantiation;

impl Rule for ProtocolInstantiation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
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

            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                message,
                violation.span,
                &module.path,
                Some(
                    "Create a concrete class that implements the Protocol, then instantiate that"
                        .to_owned(),
                ),
                Some(
                    "PEP 544: Protocol classes are structural type definitions \
                     and cannot be instantiated directly"
                        .to_owned(),
                ),
            ));
        }
    }
}
