//! Implements [`protocols_definition_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `protocols_definition_2`: Protocol conformance violation in annotated assignment.

use basilisk_resolver::ResolvedModule;

use super::Rule;
use crate::diagnostic::{Diagnostic, ErrorCode};

#[expect(
    dead_code,
    reason = "AST scaffolding preserved for its rebuilt consumer ([ASTREBUILD-PHASE-RESOLVER]); the text-matched caller was deleted under [ASTREBUILD-LAW]"
)]
mod ast_index;
mod call_args;
mod conformance;

#[expect(
    dead_code,
    reason = "rule is registered but INERT: its text-matched verdict path was deleted under [ASTREBUILD-LAW] and no diagnostic is emitted until the semantic rebuild ([ASTREBUILD-PHASE-RESOLVER])"
)]
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
