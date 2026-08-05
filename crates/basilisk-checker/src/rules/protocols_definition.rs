//! Implements [`protocols_definition`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `protocols_definition`: Protocol method sets self-attributes not declared in the Protocol.
//!
//! When a Protocol class defines a method (including `__init__`/`__new__`) that
//! assigns to `self.attr` where `attr` is not a declared member of the Protocol,
//! this is a violation: per the typing spec, "additional attributes only defined
//! in the body of a method by assignment via self are not allowed". Protocol
//! members must be explicitly declared at the class level.
//!
//! ```python
//! from typing import Protocol
//!
//! class MyProto(Protocol):
//!     x: int
//!     def __init__(self) -> None:
//!         self.y = 0  # E — `y` is not declared in the Protocol
//!     def method(self) -> None:
//!         self.z: int = 0  # E — `z` is not declared in the Protocol
//! ```
//!
//! `@staticmethod`/`@classmethod` members have no instance receiver, so their
//! first parameter is not `self` and is not analysed here.

use basilisk_resolver::ResolvedModule;

use super::Rule;
use crate::diagnostic::Diagnostic;

/// Emits `protocols_definition` when a Protocol `__new__`/`__init__` assigns to undeclared self-attributes.
pub(crate) struct ProtocolNewSelfAttrViolation;

impl Rule for ProtocolNewSelfAttrViolation {
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
