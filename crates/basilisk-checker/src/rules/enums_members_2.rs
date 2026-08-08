//! Implements [`enums_members_2`] from [CHKARCH-DIAG-COERCION]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-COERCION
//! `enums_members_2`: Non-member referenced in `Literal[EnumClass.X]` annotation.
//!
//! The `Literal[EnumClass.X]` type is only valid when `X` is an actual enum
//! member. Using it with a non-member (a method, property, lambda, nested
//! class, private attribute, or `nonmember()`-wrapped attribute) is a type error.
//!
//! ```python
//! from enum import Enum, nonmember
//! from typing import Literal
//!
//! class Pet4(Enum):
//!     CAT = 1
//!     converter = lambda x: str(x)  # Non-member (lambda)
//!
//!     def speak(self) -> None: ...  # Non-member (method)
//!
//! converter: Literal[Pet4.converter]  # E — converter is not an enum member
//! speak: Literal[Pet4.speak]          # E — speak is not an enum member
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

#[expect(
    dead_code,
    reason = "rule is registered but INERT: its text-matched verdict path was deleted under [ASTREBUILD-LAW] and no diagnostic is emitted until the semantic rebuild ([ASTREBUILD-PHASE-RESOLVER])"
)]
const CODE: ErrorCode = ErrorCode {
    code: "enums_members_2",
    docs_url: "https://www.basilisk-python.dev/errors/enums_members_2",
};

/// Emits `enums_members_2` when a non-member is referenced in `Literal[EnumClass.X]`.
pub(crate) struct EnumNonMemberInLiteral;

impl Rule for EnumNonMemberInLiteral {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
