//! BSK-E0103: Tuple index out of bounds.
//!
//! When a fixed-length `tuple[T1, T2, ...]` variable is indexed with a literal
//! integer or a `Literal[N]`-typed variable that is outside the valid range
//! `[-len, len)`, this is a static error.
//!
//! ```python
//! v: tuple[int, str, list[bool]] = (3, "hi", [True])
//! v[4]   # E0103 — index 4 out of range for 3-element tuple
//! v[-4]  # E0103 — index -4 out of range for 3-element tuple
//! ```

use basilisk_resolver::ResolvedModule;

use super::Rule;
use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0103",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0103",
};

/// Emits BSK-E0103 for out-of-bounds tuple indexing with literal integers.
pub(crate) struct TupleIndexOutOfBounds;

impl Rule for TupleIndexOutOfBounds {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for violation in &module.tuple_index_violations {
            let len = i64::try_from(violation.tuple_length).unwrap_or(i64::MAX);
            let detail = if violation.index_value >= 0 {
                format!(
                    "index {} is out of range for `tuple` of length {}",
                    violation.index_value, violation.tuple_length
                )
            } else {
                format!(
                    "index {} is out of range for `tuple` of length {} (minimum is {})",
                    violation.index_value, violation.tuple_length, -len
                )
            };

            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Tuple index out of range on `{}`: {}",
                    violation.tuple_var_name, detail
                ),
                span: violation.span,
                path: module.path.clone(),
                help: Some(format!(
                    "Valid indices for a {}-element tuple are {} to {}",
                    violation.tuple_length,
                    -len,
                    len - 1
                )),
                note: Some(
                    "Fixed-length tuples only support integer indices within \
                     the range [-length, length)"
                        .to_owned(),
                ),
            });
        }
    }
}
