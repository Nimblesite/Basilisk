//! Implements [`protocols_modules`] from [CHKARCH-DIAG-OPTIONAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OPTIONAL
//! `protocols_modules`: Module assigned to incompatible protocol type.
//!
//! When a module object is assigned to a variable typed as a `Protocol`, the
//! module's public interface must be compatible with the protocol.  This rule
//! detects assignments of the form:
//!
//! ```python
//! import some_module
//!
//! class MyProtocol(Protocol):
//!     timeout: str
//!
//! x: MyProtocol = some_module  # E — some_module.timeout is int, not str
//! ```
//!
//! Specification: <https://typing.readthedocs.io/en/latest/spec/protocol.html#modules-as-implementations-of-protocols>

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// Emits `protocols_modules` when a module is assigned to a protocol-typed variable
/// but the module does not satisfy the protocol.
pub(crate) struct ModuleProtocolIncompatible;

impl Rule for ModuleProtocolIncompatible {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Detection deleted: the previous implementation recognised `Protocol`
        // bases by comparing source-text spellings against a hardcoded string
        // instead of resolving imports. Pending a resolver-based
        // reimplementation.
    }
}
