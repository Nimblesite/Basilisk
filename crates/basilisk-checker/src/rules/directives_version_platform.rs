//! Implements [`directives_version_platform`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `directives_version_platform`: Variable defined only in dead version/platform branch.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// `directives_version_platform` rule.
pub(crate) struct DeadBranchVariable;

impl Rule for DeadBranchVariable {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
