//! Implements [BSK-0063] from [CHKARCH-STRICTNESS-SUPPRESSION-DIAGNOSTICS].
//! BSK-0063: Malformed suppression directive.
//!
//! Reports malformed directives, unknown Basilisk rule codes, conflicting
//! directives, and unmatched block boundaries.

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-0063",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-0063",
};

/// Registry identity for malformed suppression auditing.
pub(crate) struct MalformedSuppression;

impl Rule for MalformedSuppression {
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

pub(crate) fn make_diagnostic(path: &str, span: Span, problem: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE,
        format!("Malformed suppression directive: {problem}"),
        span,
        path,
        Some(
            "Use a supported directive with balanced brackets and live Basilisk rule codes"
                .to_owned(),
        ),
        Some(
            "Malformed directives may fail to suppress the diagnostic they were intended to handle"
                .to_owned(),
        ),
    )
}
