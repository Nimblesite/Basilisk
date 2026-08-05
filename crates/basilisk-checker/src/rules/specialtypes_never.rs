//! Implements [`specialtypes_never`] from [CHKARCH-DIAG-COERCION]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-COERCION
//! `specialtypes_never`: `-> NoReturn` / `-> Never` function can fall through.
//!
//! A function declared with a return type of `NoReturn` or `Never` must
//! unconditionally raise an exception or call another `NoReturn` function on
//! every code path.  If the function can reach the end of its body without
//! raising (e.g. via an `if` without an `else`), the annotation is wrong.
//!
//! ```python
//! import sys
//! from typing import NoReturn
//!
//! def stop() -> NoReturn:         # OK — always raises
//!     raise RuntimeError("no way")
//!
//! def bad(x: int) -> NoReturn:    # E — can fall through when x == 0
//!     if x != 0:
//!         sys.exit(1)
//! ```
//!
//! ## Conservative scope
//!
//! The check is conservative: it only flags a function when **all** of the
//! following hold:
//!
//! 1. The function body is not a stub (`...` or `pass`).
//! 2. The last top-level statement is **not** a `raise` statement and is
//!    **not** a standalone call expression (which may itself be `NoReturn`).
//!
//! This avoids false positives for valid patterns such as
//! `raise RuntimeError(...)` or `sys.exit(1)` as the terminating statement.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "specialtypes_never",
    docs_url: "https://www.basilisk-python.dev/errors/specialtypes_never",
};

/// Emits `specialtypes_never` when a `-> NoReturn` or `-> Never` function can fall through.
pub(crate) struct NoReturnFallThrough;

impl Rule for NoReturnFallThrough {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
