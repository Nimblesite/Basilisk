//! Implements [`typeddicts_required`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
//! `typeddicts_required`: `Required` / `NotRequired` used in an invalid context.
//!
//! PEP 655 and the typing spec restrict `Required[T]` and `NotRequired[T]` to:
//!
//! - Annotations of `TypedDict` fields
//!
//! Using them outside of a `TypedDict` body (in regular classes, function
//! parameters, variable annotations, etc.) is an error.
//!
//! Additionally, nesting `Required` or `NotRequired` inside each other is
//! forbidden even within a `TypedDict`.
//!
//! ```python
//! class NotTypedDict:
//!     x: Required[int]       # E0035 — not a TypedDict
//!
//! def func(x: NotRequired[int]) -> None:   # E0035 — not a TypedDict field
//!     ...
//!
//! class TD(TypedDict):
//!     a: Required[Required[int]]  # E0035 — nested Required
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

#[expect(
    dead_code,
    reason = "rule is registered but INERT: its text-matched verdict path was deleted under [ASTREBUILD-LAW] and no diagnostic is emitted until the semantic rebuild ([ASTREBUILD-PHASE-RESOLVER])"
)]
const CODE: ErrorCode = ErrorCode {
    code: "typeddicts_required",
    docs_url: "https://www.basilisk-python.dev/errors/typeddicts_required",
};

/// Emits `typeddicts_required` for `Required`/`NotRequired` used outside `TypedDict` or nested.
pub(crate) struct RequiredNotRequiredContext;

impl Rule for RequiredNotRequiredContext {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
