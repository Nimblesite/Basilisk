//! Implements [`generics_variance_inference`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `generics_variance_inference`: PEP 484 type-parameter scoping validation.
//!
//! INERT. Every verdict this rule made was reached by scanning `module.source`
//! line by line — splitting the file into strings, masking triple quotes,
//! tracking indentation, and matching bracket text — instead of asking the
//! AST. That is the S5 class in docs/CONFORMANCE-SPELLING-CHEAT-INVENTORY.md
//! and the mechanism the project's first standing rule forbids outright:
//! recognition is a question about declarations, answered from the AST plus
//! resolution, never from the characters at the use site.
//!
//! The six submodules that performed the scan have been deleted. The rule
//! detects nothing until type-parameter scoping is rebuilt structurally.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// Registered but inert: PEP 484 type-parameter scoping awaits a lawful
/// recognition mechanism.
pub(crate) struct TypeVarScopeViolation;

impl Rule for TypeVarScopeViolation {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
