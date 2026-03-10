//! BSK-E0088: `TypedDict` runtime violation.
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

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0088",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0088",
};

/// Emits BSK-E0088 for `TypedDict` runtime violations.
pub(crate) struct TypedDictRuntimeViolation;

impl Rule for TypedDictRuntimeViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for span in &module.isinstance_typeddict_violations {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: "TypedDict type objects cannot be used in `isinstance()` tests"
                    .to_owned(),
                span: *span,
                path: module.path.clone(),
                help: Some(
                    "Use a regular class or Protocol for isinstance checks".to_owned(),
                ),
                note: Some(
                    "PEP 589: TypedDict classes exist only at type-checking time;                      they are plain dicts at runtime"
                        .to_owned(),
                ),
            });
        }
    }
}
