//! BSK-E0097: Protocol `__new__`/`__init__` sets self-attributes not declared in Protocol.
//!
//! When a Protocol class defines `__new__` or `__init__` that assigns to
//! `self.attr` where `attr` is not a declared member of the Protocol, this is
//! a violation: Protocol members must be explicitly declared.
//!
//! ```python
//! from typing import Protocol
//!
//! class MyProto(Protocol):
//!     x: int
//!     def __init__(self) -> None:
//!         self.y = 0  # E — `y` is not declared in the Protocol
//! ```

use basilisk_resolver::ResolvedModule;

use super::Rule;
use crate::diagnostic::Diagnostic;

/// Emits BSK-E0097 when a Protocol `__new__`/`__init__` assigns to undeclared self-attributes.
pub(crate) struct ProtocolNewSelfAttrViolation;

impl Rule for ProtocolNewSelfAttrViolation {
    fn check(&self, _module: &ResolvedModule, _diagnostics: &mut Vec<Diagnostic>) {
        // Stub — will be implemented when the resolver collects the necessary data.
    }
}
