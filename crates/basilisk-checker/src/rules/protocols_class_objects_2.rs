//! Implements [`protocols_class_objects_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `protocols_class_objects_2`: Protocol class object violations.
//!
//! Detects two related violations involving Protocol classes and class objects:
//!
//! 1. A Protocol class itself is passed/assigned where `type[Proto]` is expected.
//!    Only concrete (non-Protocol) subtypes may be used.
//!
//! 2. A class object is assigned to a variable typed as a Protocol instance,
//!    but the class does not structurally satisfy the protocol when treated as
//!    an object (i.e. class-level access to protocol members gives incompatible
//!    types).
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
//! fun(Proto)      # E0146 — Protocol class itself passed to type[Proto]
//! fun(Concrete)   # OK
//!
//! var: type[Proto]
//! var = Proto     # E0146 — Protocol class assigned to type[Proto]
//! var = Concrete  # OK
//!
//! pa1: ProtoA1 = ConcreteA  # E0146 — class object can't satisfy instance protocol
//! pa2: ProtoA2 = ConcreteA  # OK    — protocol uses _self/self pattern
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// Emits `protocols_class_objects_2` for Protocol class object violations.
pub(crate) struct ProtocolClassObjectViolation;

impl Rule for ProtocolClassObjectViolation {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Detection deleted: the previous implementation recognised `Protocol`
        // and `ClassVar` by comparing source-text spellings against hardcoded
        // strings instead of resolving imports. Pending a resolver-based
        // reimplementation.
    }
}
