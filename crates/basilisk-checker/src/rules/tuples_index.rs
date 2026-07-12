//! Implements [`tuples_index`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! `tuples_index`: Tuple index out of bounds.
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
//!
//! The parameter of a `key=` lambda passed to `sorted`/`min`/`max`/`list.sort`
//! receives one element of the iterable, so when the iterable is provably a
//! collection of fixed-length tuples — from its annotation or from a literal
//! of uniform tuples — the same range check applies inside the lambda:
//!
//! ```python
//! items = [("a", 1, 2), ("b", 3, 4)]
//! sorted(items, key=lambda pair: pair[4])  # E — 4 out of range for 3-tuple
//! ```

use basilisk_resolver::ResolvedModule;

use super::Rule;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

const CODE: ErrorCode = ErrorCode {
    code: "tuples_index",
    docs_url: "https://www.basilisk-python.dev/errors/tuples_index",
};

/// Emits `tuples_index` for out-of-bounds tuple indexing with literal integers.
pub(crate) struct TupleIndexOutOfBounds;

impl Rule for TupleIndexOutOfBounds {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
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

            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Tuple index out of range on `{}`: {}",
                    violation.tuple_var_name, detail
                ),
                violation.span,
                &module.path,
                Some(format!(
                    "Valid indices for a {}-element tuple are {} to {}",
                    violation.tuple_length,
                    -len,
                    len - 1
                )),
                Some(
                    "Fixed-length tuples only support integer indices within \
                     the range [-length, length)"
                        .to_owned(),
                ),
            ));
        }
    }
}
