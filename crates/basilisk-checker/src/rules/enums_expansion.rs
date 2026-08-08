//! Implements [`enums_expansion`] from [CHKARCH-DIAG-COERCION]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-COERCION
//! `enums_expansion`: `assert_type` with `Literal[Enum.MEMBER]` on enum-typed param.
//!
//! This rule detects when `assert_type()` is used with a `Literal[Enum.MEMBER]` type
//! on a parameter that is already typed as the enum itself. This is redundant and
//! indicates a misunderstanding of enum typing semantics.
//!
//! ```python
//! from enum import Enum
//! from typing import assert_type, Literal
//!
//! class Status(Enum):
//!     ACTIVE = 1
//!     INACTIVE = 2
//!
//! def process(status: Status) -> None:
//!     assert_type(status, Literal[Status.ACTIVE])  # E0061 — redundant narrowing
//!     assert_type(status, Status)                  # OK — correct usage
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

#[expect(
    dead_code,
    reason = "rule is registered but INERT: its text-matched verdict path was deleted under [ASTREBUILD-LAW] and no diagnostic is emitted until the semantic rebuild ([ASTREBUILD-PHASE-RESOLVER])"
)]
const CODE: ErrorCode = ErrorCode {
    code: "enums_expansion",
    docs_url: "https://www.basilisk-python.dev/errors/enums_expansion",
};

/// Emits `enums_expansion` for `assert_type` with `Literal[Enum.MEMBER]` on enum-typed param.
pub(crate) struct AssertTypeEnumLiteralMismatch;

impl Rule for AssertTypeEnumLiteralMismatch {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
