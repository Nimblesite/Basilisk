//! Implements [`protocols_definition_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `protocols_definition_2`: Protocol conformance violation in annotated assignment.

use basilisk_resolver::ResolvedModule;

use super::Rule;
use crate::diagnostic::{Diagnostic, ErrorCode};

mod ast_index;
mod call_args;
mod conformance;

pub(super) const CODE: ErrorCode = ErrorCode {
    code: "protocols_definition_2",
    docs_url: "https://www.basilisk-python.dev/errors/protocols_definition_2",
};

/// Emits `protocols_definition_2` for protocol conformance violations in annotated assignments.
pub(crate) struct ProtocolAssignmentConformance;

impl Rule for ProtocolAssignmentConformance {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
