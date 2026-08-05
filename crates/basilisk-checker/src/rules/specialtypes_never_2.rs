//! Implements [`specialtypes_never_2`] from [CHKARCH-DIAG-OPTIONAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OPTIONAL
//! `specialtypes_never_2`: currently inert.
//!
//! Both violation shapes this rule reported — the invariant local assignment
//! and the invariant return — started by recognising the bottom type from the
//! name written at the use site. That mechanism is banned permanently (see the
//! symbol-naming ban in `CLAUDE.md` and
//! `docs/CONFORMANCE-SPELLING-CHEAT-INVENTORY.md`), and with the bottom type
//! unidentifiable every branch below it was dead, so the whole body has been
//! deleted rather than re-expressed behind another spelling comparison.
//!
//! The rule stays registered and emits nothing. It will be rebuilt on
//! definition resolution — following each binding through the module's imports
//! to the declaration it actually resolves to — in a later phase.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// Registered placeholder for `specialtypes_never_2`. Emits no diagnostics
/// until the rule is rebuilt on resolved definitions.
pub(crate) struct NeverTypeCompatibility;

impl Rule for NeverTypeCompatibility {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
