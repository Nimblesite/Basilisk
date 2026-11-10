//! Implements [`dataclasses_transform_class`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `dataclasses_transform_class`: currently inert.
//!
//! Every check this rule performed — frozen inheritance, frozen attribute
//! assignment, keyword-only construction, ordering comparisons, and field
//! `converter=` validation — started from the set of classes carrying the
//! transform decorator, which was found by comparing the decorator against a
//! member name written into this source. That mechanism is banned permanently
//! (see the symbol-naming ban in `CLAUDE.md` and
//! `docs/CONFORMANCE-SPELLING-CHEAT-INVENTORY.md`), and with the base-class set
//! unobtainable every branch below it was dead, so the whole body — helpers and
//! converter support included — has been deleted rather than re-expressed
//! behind another spelling comparison.
//!
//! The rule stays registered and emits nothing. It will be rebuilt on
//! definition resolution — following each binding through the module's imports
//! to the declaration it actually resolves to — in a later phase.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// Registered placeholder for `dataclasses_transform_class`. Emits no
/// diagnostics until the rule is rebuilt on resolved definitions.
pub(crate) struct DataclassTransformClassViolation;

impl Rule for DataclassTransformClassViolation {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
