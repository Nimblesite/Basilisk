// ############################################################################
// # BROKEN — THIS FILE DOES NOT COMPILE. DO NOT "FIX" IT BY RESTORING TEXT   #
// # MATCHING.                                                                #
// #                                                                          #
// # Deleted helper this file called:                                         #
// #   crate::subtyping (SubtypingContext::is_subtype / name_subtype)
// #                                                                          #
// # That helper decided types from the SPELLING of source text (lowercased   #
// # annotation strings, `"int"`/`"str"`/`"object"` literal matching, `|`     #
// # splitting, `starts_with("tuple[")`). It was deleted, not replaced.       #
// #                                                                          #
// # The call sites below are LEFT BROKEN ON PURPOSE. They are the map of     #
// # what must be rebuilt on the resolved AST — resolved bindings, canonical  #
// # `TypeNode`, and `assignable`/`equivalent` — or made to abstain.          #
// #                                                                          #
// # Restoring the deleted helper, vendoring a copy of it, or re-deriving a   #
// # type from source text anywhere below is FORBIDDEN.                       #
// #                                                                          #
// # Evidence and the failing tests that pin the real behaviour:              #
// #   docs/RULE-VALIDITY-REPORT.md                                           #
// #   crates/basilisk-checker/tests/legacy_annotation_text_parser_pin_tests.rs
// #   crates/basilisk-checker/tests/pep_spelling_invariance_pin_tests.rs     #
// ############################################################################

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
use crate::subtyping::{module_context, SubtypingContext};

use super::oracle::ModuleOracle;

/// Everything a rule needs to answer "what type is this, and does it fit?".
pub(crate) struct ModuleTypes<'m> {
    annotations: Option<AnnotationResolver<'m>>,
    oracle: Option<ModuleOracle<'m>>,
    subtyping: SubtypingContext,
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
            subtyping: module_context(module),
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

    /// The module's nominal class hierarchy ([TYPEINF-SUBTYPING]).
    pub(crate) fn subtyping(&self) -> &SubtypingContext {
        &self.subtyping
    }
}
