//! Implements [`qualifiers_annotated`] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
//! `qualifiers_annotated`: PEP 593 metadata-qualifier validation.
//!
//! INERT. Every branch of this rule started by asking whether a subscript's
//! base was spelled with a particular typing symbol's name — directly, or by
//! consulting a type-alias set that was itself built from such a question.
//! The symbol-naming ban forbids that (see CLAUDE.md, "THE SYMBOL-NAMING
//! BAN", and docs/CONFORMANCE-SPELLING-CHEAT-INVENTORY.md), so the
//! recognition, the diagnostics it gated, and the name-collection helper that
//! fed it have been deleted. The rule detects nothing until the qualifier is
//! recognised from the declaration a binding resolves to.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// Registered but inert: PEP 593 first-argument checking awaits a lawful
/// recognition mechanism.
pub(crate) struct AnnotatedInvalidFirstArg;

impl Rule for AnnotatedInvalidFirstArg {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
