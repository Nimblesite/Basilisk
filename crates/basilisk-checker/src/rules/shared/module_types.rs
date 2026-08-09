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
use crate::subtyping::SubtypingContext;

use super::oracle::ModuleOracle;

/// Everything a rule needs to answer "what type is this, and does it fit?".
pub(crate) struct ModuleTypes<'m> {
    annotations: Option<AnnotationResolver<'m>>,
    oracle: Option<ModuleOracle<'m>>,
    // The `subtyping: SubtypingContext` field is DELETED — see the banner on
    // the removed accessor below. Do not re-add it in any form.
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

    // ######################################################################
    // # DELETED — `subtyping()`, and the `subtyping: SubtypingContext`     #
    // # field it exposed. DO NOT RESTORE. DO NOT SUBSTITUTE A PLACEHOLDER  #
    // # CONTEXT THAT ANSWERS EVERY QUERY `false` TO MAKE THIS COMPILE.     #
    // #                                                                    #
    // # `crate::subtyping` keyed its entire hierarchy on STRINGS:          #
    // # `is_subtype(&str, &str)` over class names harvested from rendered  #
    // # annotation text, with `"int"`/`"str"`/`"object"` literal matching, #
    // # `|` splitting, and `starts_with("tuple[")`. Every rule that took   #
    // # this accessor inherited a verdict derived from spelling, however   #
    // # careful the rule's own logic was. That is why ~20 rules could not  #
    // # be honest individually — they were reading from a dishonest well.  #
    // #                                                                    #
    // # The replacement is the canonical binding table plus `TypeNode`     #
    // # relations; a class hierarchy is built from RESOLVED base symbols,  #
    // # never from base-class source text.                                 #
    // #                                                                    #
    // # Every caller of `types.subtyping()` is LEFT AS A PANICKING CALL    #
    // # ON PURPOSE. Those call sites are the map of what must be rebuilt.  #
    // ######################################################################

    /// DELETED — panics. The accessor's signature survives only so its eight
    /// call sites stay visible as the rebuild map; see the banner above.
    pub(crate) fn subtyping(&self) -> &SubtypingContext {
        panic!(
            "basilisk-checker: `ModuleTypes::subtyping` was DELETED because the context \
             it handed out keyed its entire class hierarchy on STRINGS harvested from \
             rendered annotation text, so every rule that took it inherited a verdict \
             derived from spelling however careful the rule's own logic was. It panics \
             because the real implementation — a hierarchy built from resolved base \
             symbols on the binding table — DOES NOT EXIST YET. Do not restore the \
             field and do not hand out a placeholder context that answers every query \
             the same way to make this compile."
        )
    }
}
