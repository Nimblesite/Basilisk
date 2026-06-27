//! Implements [`match_exhaustiveness`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-typesafety
//! `match_exhaustiveness`: Non-exhaustive `match` statement.
//!
//! A value-dispatch `match` statement that has no irrefutable branch may fail
//! to handle certain runtime values, leading to a silent fall-through (Python
//! does not raise an error for unmatched `match` subjects).  Basilisk reports
//! this as an error.
//!
//! Two cases are *not* flagged, matching the reference checkers:
//!   * a bare capture `case name:` (no guard) is irrefutable — like `case _:`,
//!     it makes the match exhaustive;
//!   * a structural match (sequence/mapping patterns) decomposes open-ended
//!     shapes — e.g. narrowing a tuple union of mixed arity — where a catch-all
//!     is not required for correctness.

use basilisk_resolver::{MatchStmtInfo, ResolvedModule};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "match_exhaustiveness",
    docs_url: "https://www.basilisk-python.dev/errors/match_exhaustiveness",
};

/// Emits `match_exhaustiveness` for every `match` statement that lacks a wildcard branch.
pub(crate) struct NonExhaustiveMatch;

impl Rule for NonExhaustiveMatch {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        module
            .match_stmts
            .iter()
            .filter(|stmt| !stmt.has_wildcard && !stmt.has_structural_pattern)
            .for_each(|stmt| diagnostics.push(make_diagnostic(stmt, &module.path)));
    }
}

fn make_diagnostic(stmt: &MatchStmtInfo, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        "Non-exhaustive `match` statement — no wildcard `case _:` branch".to_owned(),
        stmt.span,
        path,
        Some("Add a `case _: ...` branch to handle all remaining cases".to_owned()),
        Some(
            "Python does not raise an error for unmatched subjects; \
             a wildcard branch makes exhaustiveness explicit"
                .to_owned(),
        ),
    )
}
