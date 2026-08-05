//! Implements [`literals_semantics_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `literals_semantics_2`: currently inert.
//!
//! Both violation shapes this rule reported started from the set of parameters
//! whose annotation was recognised as a typing special form by the name written
//! at the use site. That mechanism is banned permanently (see the symbol-naming
//! ban in `CLAUDE.md` and `docs/CONFORMANCE-SPELLING-CHEAT-INVENTORY.md`), and
//! with the parameter set unobtainable every branch below it was dead, so the
//! whole body has been deleted rather than re-expressed behind another spelling
//! comparison.
//!
//! The rule stays registered and emits nothing. It will be rebuilt on
//! definition resolution — following each binding through the module's imports
//! to the declaration it actually resolves to — in a later phase.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// Registered placeholder for `literals_semantics_2`. Emits no diagnostics
/// until the rule is rebuilt on resolved definitions.
pub(crate) struct LiteralValueIncompatible;

impl Rule for LiteralValueIncompatible {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
