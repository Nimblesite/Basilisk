//! Implements [`historical_positional`] from [CHKARCH-DIAG-OPTIONAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-optional
//! `historical_positional`: Historical positional-only parameter violations.
//!
//! Before PEP 570 (Python 3.8), the convention for marking parameters as
//! positional-only was to prefix their names with `__` (double underscore)
//! without a trailing `__`. Type checkers must support this historical
//! mechanism.
//!
//! Two violations are detected:
//!
//! 1. **`PositionalOnlyAfterKeyword`**: A `__`-prefixed positional-only
//!    parameter appears after a regular positional-or-keyword parameter
//!    in a function that does not use PEP 570 `/` syntax.
//!
//! 2. **`KeywordPassedToPositionalOnly`**: A `__`-prefixed keyword argument
//!    is passed at a call site (e.g. `f(__x=3)`), which is invalid because
//!    `__x` is positional-only and cannot be passed by keyword.
//!
//! ```python
//! def f1(__x: int) -> None: ...
//!
//! f1(__x=3)  # E — __x is positional-only
//!
//! def f2(x: int, __y: int) -> None: ...  # E — __y after positional-or-keyword x
//! ```

use basilisk_resolver::{scope::HistoricalPositionalViolationKind, ResolvedModule};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "historical_positional",
    docs_url: "https://www.basilisk-python.dev/errors/historical_positional",
};

/// Emits `historical_positional` for historical positional-only parameter violations.
pub(crate) struct HistoricalPositionalViolation;

impl Rule for HistoricalPositionalViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let path = &module.path;
        for violation in &module.historical_positional_violations {
            let diagnostic = match violation.kind {
                HistoricalPositionalViolationKind::KeywordPassedToPositionalOnly => {
                    error_diagnostic_owned(
                        CODE.clone(),
                        format!(
                            "`{}` is a positional-only parameter and cannot be passed as a keyword argument",
                            violation.name
                        ),
                        violation.span,
                        path,
                        Some(format!(
                            "Pass `{}` positionally instead of as a keyword argument",
                            violation.name
                        )),
                        Some(
                            "Parameters prefixed with `__` (but not ending with `__`) are positional-only \
                             by the historical convention (pre-PEP 570)"
                                .to_owned(),
                        ),
                    )
                }
                HistoricalPositionalViolationKind::PositionalOnlyAfterKeyword => {
                    error_diagnostic_owned(
                        CODE.clone(),
                        format!(
                            "Positional-only parameter `{}` appears after a positional-or-keyword parameter",
                            violation.name
                        ),
                        violation.span,
                        path,
                        Some(
                            "Move positional-only (`__`-prefixed) parameters before any \
                             positional-or-keyword parameters, or use PEP 570 `/` syntax"
                                .to_owned(),
                        ),
                        Some(
                            "Parameters prefixed with `__` (but not ending with `__`) are treated as \
                             positional-only by the historical convention; they cannot follow \
                             positional-or-keyword parameters"
                                .to_owned(),
                        ),
                    )
                }
            };
            diagnostics.push(diagnostic);
        }
    }
}
