//! Implements [BSK-E0114] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! BSK-E0114: Protocol `isinstance`/`issubclass` violations.
//!
//! Per PEP 544:
//! - A protocol can be used as the second argument to `isinstance()` or
//!   `issubclass()` **only** if it is decorated with `@runtime_checkable`.
//! - `issubclass()` can only be used with **non-data** protocols (protocols
//!   that define only methods, not data attributes).
//!
//! ```python
//! from typing import Protocol, runtime_checkable
//!
//! class Proto1(Protocol):
//!     name: str
//!
//! @runtime_checkable
//! class Proto2(Protocol):
//!     name: str
//!     def method(self) -> int: ...
//!
//! isinstance(x, Proto1)            # E — not @runtime_checkable
//! issubclass(x, Proto2)            # E — data protocol in issubclass
//! issubclass(x, (Proto2, Proto1))  # E — tuple contains violating protocol
//! ```

use basilisk_resolver::scope::ProtocolRtcViolationKind;
use basilisk_resolver::ResolvedModule;

use super::Rule;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0114",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0114",
};

/// Emits BSK-E0114 for `isinstance`/`issubclass` calls violating protocol
/// runtime-checkable constraints.
pub(crate) struct ProtocolRuntimeCheckableViolation;

impl Rule for ProtocolRuntimeCheckableViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for violation in &module.protocol_runtime_checkable_violations {
            let (message, help, note) = match &violation.kind {
                ProtocolRtcViolationKind::NotRuntimeCheckable {
                    protocol_name,
                    call_name,
                } => (
                    format!(
                        "Protocol `{protocol_name}` cannot be used with `{call_name}()` \
                         because it is not decorated with `@runtime_checkable`"
                    ),
                    Some(format!(
                        "Add `@runtime_checkable` to the definition of `{protocol_name}`"
                    )),
                    Some(
                        "PEP 544: a Protocol can only be used as the second argument in \
                         isinstance() or issubclass() if it has the @runtime_checkable decorator"
                            .to_owned(),
                    ),
                ),
                ProtocolRtcViolationKind::IssubclassDataProtocol { protocol_name } => (
                    format!("`issubclass()` cannot be used with data protocol `{protocol_name}`"),
                    Some(format!(
                        "Remove the data attributes from `{protocol_name}` or \
                         use `isinstance()` instead"
                    )),
                    Some(
                        "PEP 544: issubclass() can only be used with non-data protocols \
                         (protocols that define only methods, not data attributes)"
                            .to_owned(),
                    ),
                ),
            };

            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                message,
                violation.span,
                &module.path,
                help,
                note,
            ));
        }
    }
}
