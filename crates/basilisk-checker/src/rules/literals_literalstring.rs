//! Implements [`literals_literalstring`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `literals_literalstring`: currently inert.
//!
//! Every verdict this rule produced was gated on recognising a typing special
//! form by the name written at the use site. That mechanism is banned
//! permanently (see the symbol-naming ban in `CLAUDE.md` and
//! `docs/CONFORMANCE-SPELLING-CHEAT-INVENTORY.md`), so the entire body has been
//! deleted rather than re-expressed behind another spelling comparison.
//!
//! The rule stays registered and emits nothing. It will be rebuilt on
//! definition resolution — following each binding through the module's imports
//! to the declaration it actually resolves to — in a later phase.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// Registered placeholder for `literals_literalstring`. Emits no diagnostics
/// until the rule is rebuilt on resolved definitions.
pub(crate) struct LiteralStringAssignment;

impl Rule for LiteralStringAssignment {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
