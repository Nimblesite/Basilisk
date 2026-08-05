//! Implements [`classes_classvar`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
//! `classes_classvar`: PEP 526 class-variable qualifier validation.
//!
//! INERT. Every check here — invalid qualifier position, nesting, argument
//! validity, initializer kind, instance assignment, and protocol conformance —
//! began by asking whether an annotation node or a base-class expression was
//! spelled with a particular typing symbol's name. The symbol-naming ban
//! forbids that (see CLAUDE.md, "THE SYMBOL-NAMING BAN", and
//! docs/CONFORMANCE-SPELLING-CHEAT-INVENTORY.md), so the recognition helpers,
//! the argument and initializer validators, the instance-assignment walker,
//! and the protocol-conformance pass have all been deleted along with the
//! diagnostics they gated. The rule detects nothing until class-variable
//! recognition is rebuilt from the declaration a binding resolves to.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// Registered but inert: PEP 526 class-variable checking awaits a lawful
/// recognition mechanism.
pub(crate) struct ClassVarInvalidContext;

impl Rule for ClassVarInvalidContext {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
