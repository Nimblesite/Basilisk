//! BSK-E0023: Non-exhaustive `match` statement.
//!
//! A `match` statement that has no wildcard `case _:` branch may fail to
//! handle certain runtime values, leading to a silent fall-through (Python
//! does not raise an error for unmatched `match` subjects).  Basilisk treats
//! this as an error in strict mode.

use basilisk_resolver::{MatchStmtInfo, ResolvedModule};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0023",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0023",
};

/// Emits BSK-E0023 for every `match` statement that lacks a wildcard branch.
pub(crate) struct NonExhaustiveMatch;

impl Rule for NonExhaustiveMatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        module
            .match_stmts
            .iter()
            .filter(|stmt| !stmt.has_wildcard)
            .for_each(|stmt| diagnostics.push(make_diagnostic(stmt, &module.path)));
    }
}

fn make_diagnostic(stmt: &MatchStmtInfo, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: "Non-exhaustive `match` statement — no wildcard `case _:` branch".to_owned(),
        span: stmt.span,
        path: path.to_owned(),
        help: Some("Add a `case _: ...` branch to handle all remaining cases".to_owned()),
        note: Some(
            "Python does not raise an error for unmatched subjects; \
             a wildcard branch makes exhaustiveness explicit"
                .to_owned(),
        ),
    }
}
