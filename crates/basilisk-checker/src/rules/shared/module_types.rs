//! Implements [NARROWPLAN-INTEGRATION]. See
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-INTEGRATION
//!
//! The module's shared TYPE CONTEXT: the annotation cascade, the
//! bidirectional-inference oracle, and the nominal subtyping table, built once
//! per module and handed to every rule that reasons about types.
//!
//! Each of the three costs a full walk of the module — the cascade builds its
//! tables and span index, the oracle indexes every expression and seeds the
//! engine, the subtyping context registers every class. A rule that builds its
//! own pays that walk again, and a dozen such rules made the walks the dominant
//! cost of checking a file ([CHKARCH-TESTING-BENCH]). One context, one
//! set of walks, one answer per expression.

use basilisk_resolver::ResolvedModule;

use crate::annotation::AnnotationResolver;

use super::nominal::NominalHierarchy;
use super::oracle::ModuleOracle;

/// Everything a rule needs to answer "what type is this, and does it fit?".
pub(crate) struct ModuleTypes<'m> {
    annotations: Option<AnnotationResolver<'m>>,
    oracle: Option<ModuleOracle<'m>>,
    nominal: NominalHierarchy<'m>,
}

impl<'m> ModuleTypes<'m> {
    /// Build the context for `module`. The cascade and the oracle are `None`
    /// when the module does not parse — that failure is reported as its own
    /// diagnostic, and every type judgment then abstains rather than guessing.
    pub(crate) fn build(module: &'m ResolvedModule) -> Self {
        let annotations = AnnotationResolver::for_module(module);
        let oracle = annotations
            .as_ref()
            .and_then(|resolver| ModuleOracle::build(module, resolver));
        Self {
            annotations,
            oracle,
            nominal: NominalHierarchy::build(module),
        }
    }

    /// The module's annotation cascade ([TYPEINF-ANNOTATION-RESOLUTION]).
    pub(crate) fn annotations(&self) -> Option<&AnnotationResolver<'m>> {
        self.annotations.as_ref()
    }

    /// The module's bidirectional-inference oracle
    /// ([TYPEINF-TARGET-BIDIRECTIONAL]).
    pub(crate) fn oracle(&self) -> Option<&ModuleOracle<'m>> {
        self.oracle.as_ref()
    }

    /// The module's nominal class hierarchy ([TYPEINF-SUBTYPING-NOMINAL]).
    ///
    /// REPLACES the deleted `subtyping()` accessor, which handed out a
    /// hierarchy keyed on strings harvested from rendered annotation text —
    /// every rule that took it inherited a verdict about spelling. This one is
    /// keyed on class definition sites; see [`NominalHierarchy`].
    pub(crate) fn nominal(&self) -> &NominalHierarchy<'m> {
        &self.nominal
    }
}
