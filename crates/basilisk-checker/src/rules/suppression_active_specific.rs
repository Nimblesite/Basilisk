//! Implements [BSK-I0060] from [CHKARCH-STRICTNESS-SUPPRESSION-DIAGNOSTICS].
//! BSK-I0060: Active code-specific suppression.
//!
//! Reports a valid source directive that names one or more Basilisk rules and
//! actively suppresses a diagnostic or changes its effective severity.

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{info_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-I0060",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-I0060",
};

/// Registry identity for active code-specific suppression auditing.
pub(crate) struct ActiveSpecificSuppression;

impl Rule for ActiveSpecificSuppression {
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

pub(crate) fn make_diagnostic(path: &str, span: Span, matched: usize) -> Diagnostic {
    info_diagnostic_owned(
        CODE,
        format!("Code-specific directive actively changes {matched} diagnostic(s)"),
        span,
        path,
        Some("Keep the directive only while the selected exception is intentional".to_owned()),
        Some("Suppression auditing is opt-in and does not change the directive itself".to_owned()),
    )
}
