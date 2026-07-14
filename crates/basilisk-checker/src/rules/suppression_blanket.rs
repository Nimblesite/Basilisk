//! Implements [BSK-W0061] from [CHKARCH-STRICTNESS-SUPPRESSION-DIAGNOSTICS].
//! BSK-W0061: Active blanket suppression.
//!
//! Reports a valid source directive that actively changes diagnostics without
//! selecting individual Basilisk rule codes.

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{warning_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-W0061",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-W0061",
};

/// Registry identity for active blanket suppression auditing.
pub(crate) struct ActiveBlanketSuppression;

impl Rule for ActiveBlanketSuppression {
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
    warning_diagnostic_owned(
        CODE,
        format!("Blanket directive actively changes {matched} diagnostic(s)"),
        span,
        path,
        Some("Prefer naming the exact Basilisk rule codes that need an exception".to_owned()),
        Some("Blanket directives can hide unrelated diagnostics added later".to_owned()),
    )
}
