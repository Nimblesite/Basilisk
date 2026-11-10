//! Implements [`qualifiers_final_annotation`] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
//! `qualifiers_final_annotation`: PEP 591 qualifier-position validation.
//!
//! INERT. Every verdict this rule made was reached by asking whether an
//! annotation node was spelled with a particular typing symbol's name, which
//! the symbol-naming ban forbids (see CLAUDE.md, "THE SYMBOL-NAMING BAN", and
//! docs/CONFORMANCE-SPELLING-CHEAT-INVENTORY.md). The predicates and the
//! diagnostics they gated have been deleted; the rule detects nothing until
//! qualifier recognition is rebuilt from declarations rather than from the
//! characters at the use site.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// Registered but inert: PEP 591 qualifier-position checking awaits a lawful
/// recognition mechanism.
pub(crate) struct FinalInvalidPosition;

impl Rule for FinalInvalidPosition {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
