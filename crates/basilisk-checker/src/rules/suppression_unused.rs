//! Implements [BSK-W0062] from [CHKARCH-STRICTNESS-SUPPRESSION-DIAGNOSTICS].
//! BSK-W0062: Unused suppression directive.
//!
//! Reports a syntactically valid directive that matches no diagnostic or does
//! not change the effective severity of anything it matches.

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{warning_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-W0062",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-W0062",
};

/// Registry identity for unused suppression auditing.
pub(crate) struct UnusedSuppression;

impl Rule for UnusedSuppression {
    fn opt_in_spec(&self) -> Option<crate::rule_tags::OptInSpec> {
        Some(crate::rule_tags::OptInSpec {
            code: CODE.code,
            tags: &["suppressions"],
        })
    }

    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}

pub(crate) fn make_diagnostic(path: &str, span: Span) -> Diagnostic {
    warning_diagnostic_owned(
        CODE,
        "Suppression directive is unused".to_owned(),
        span,
        path,
        Some("Remove the directive or update it to select a diagnostic that still exists".to_owned()),
        Some("Unused suppressions conceal whether an exception is still necessary".to_owned()),
    )
}
