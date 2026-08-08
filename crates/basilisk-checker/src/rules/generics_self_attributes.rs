//! Implements [`generics_self_attributes`] from [CHKARCH-DIAG-OPTIONAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OPTIONAL
//! `generics_self_attributes`: Incompatible type for `Self`-typed attribute.
//!
//! When a class declares an attribute annotated with `Self` (or `Self | None`,
//! `Optional[Self]`, etc.), that attribute's type is bound to the *concrete*
//! subclass at each usage site.  Passing or assigning a parent-class instance
//! where the subclass is expected is a type error.
//!
//! ```python
//! from typing import Self, TypeVar, Generic
//! from dataclasses import dataclass
//!
//! T = TypeVar("T")
//!
//! @dataclass
//! class LinkedList(Generic[T]):
//!     value: T
//!     next: Self | None = None
//!
//! @dataclass
//! class OrdinalLinkedList(LinkedList[int]):
//!     def ordinal_value(self) -> str:
//!         return str(self.value)
//!
//! xs = OrdinalLinkedList(value=1, next=LinkedList[int](value=2))  # E
//! xs.next = LinkedList[int](value=3, next=None)                  # E
//! ```
//!
//! Specification: <https://typing.readthedocs.io/en/latest/spec/generics.html#use-in-attribute-annotations>

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

#[expect(
    dead_code,
    reason = "rule is registered but INERT: its text-matched verdict path was deleted under [ASTREBUILD-LAW] and no diagnostic is emitted until the semantic rebuild ([ASTREBUILD-PHASE-RESOLVER])"
)]
const CODE: ErrorCode = ErrorCode {
    code: "generics_self_attributes",
    docs_url: "https://www.basilisk-python.dev/errors/generics_self_attributes",
};

/// Emits `generics_self_attributes` when a parent-class instance is used where a `Self`-typed
/// attribute expects the concrete subclass.
pub(crate) struct SelfTypeAttributeIncompatible;

impl Rule for SelfTypeAttributeIncompatible {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
