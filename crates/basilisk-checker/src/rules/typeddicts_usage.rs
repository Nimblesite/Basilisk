//! Implements [`typeddicts_usage`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! `typeddicts_usage`: `TypedDict` runtime violation.
//!
//! PEP 589 defines constraints on what you can do with `TypedDict` type objects at runtime:
//!
//! - `TypedDict` type objects cannot be used in `isinstance()` tests.
//!
//! ```python
//! from typing import TypedDict
//!
//! class Movie(TypedDict):
//!     name: str
//!     year: int
//!
//! movie: Movie = {"name": "Blade Runner", "year": 1982}
//!
//! if isinstance(movie, Movie):  # E — TypedDict cannot be used in isinstance
//!     ...
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "typeddicts_usage",
    docs_url: "https://www.basilisk-python.dev/errors/typeddicts_usage",
};

/// Emits `typeddicts_usage` for `TypedDict` runtime violations.
pub(crate) struct TypedDictRuntimeViolation;

impl Rule for TypedDictRuntimeViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for span in &module.isinstance_typeddict_violations {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                "TypedDict type objects cannot be used in `isinstance()` tests"
                    .to_owned(),
                *span,
                &module.path,
                Some(
                    "Use a regular class or Protocol for isinstance checks".to_owned(),
                ),
                Some(
                    "PEP 589: TypedDict classes exist only at type-checking time;                      they are plain dicts at runtime"
                        .to_owned(),
                ),
            ));
        }
    }
}
