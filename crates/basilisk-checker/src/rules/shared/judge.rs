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

//! Implements [NARROWPLAN-INTEGRATION] / [TYPEINF-TARGET-BIDIRECTIONAL]. See
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-INTEGRATION
//!
//! The one judgment every "does this value fit this declared type?" rule
//! asks — assignments, returns, yields, arguments. It reads the module's
//! shared [`ModuleTypes`](super::module_types::ModuleTypes): the oracle types
//! the expression, [`SubtypingContext`] settles nominal relationships, and the
//! annotation cascade decides whether the declared side is even judgeable.
//!
//! Every arm abstains rather than guesses. A span with no expression, an
//! expression the engine types `Unknown`, a structural target, an unresolvable
//! nominal leaf — each answers "no evidence", never "error"
//! ([CHKARCH-CONFORMANCE-MODE]).

use basilisk_resolver::Span;
use ruff_python_ast::Expr;

use crate::annotation::AnnotationResolver;
use crate::subtyping::SubtypingContext;
use crate::types::InferredType;

use super::oracle::ModuleOracle;

/// The module's type judgment, borrowed from the shared context.
pub(crate) struct TypeJudge<'m, 'a> {
    oracle: Option<&'a ModuleOracle<'m>>,
    resolver: &'a AnnotationResolver<'m>,
    subtyping: &'a SubtypingContext,
}

impl<'m, 'a> TypeJudge<'m, 'a> {
    /// Borrow the judgment from a module's shared type context.
    pub(crate) fn new(
        oracle: Option<&'a ModuleOracle<'m>>,
        resolver: &'a AnnotationResolver<'m>,
        subtyping: &'a SubtypingContext,
    ) -> Self {
        Self {
            oracle,
            resolver,
            subtyping,
        }
    }

    /// The engine's type for the expression at `span`, `Unknown` when there is
    /// no expression there or the engine declines to answer — an unresolved
    /// value never manufactures a diagnostic.
    pub(crate) fn inferred(&self, span: Option<Span>) -> InferredType {
        self.oracle
            .zip(span)
            .and_then(|(oracle, span)| oracle.synth_span(span))
            .unwrap_or(InferredType::Unknown)
    }

    /// Evaluate a type expression held only as TEXT through the same
    /// cascade — alias expansion, class case, shadowing included
    /// ([TYPEINF-ANNOTATION-RESOLUTION]).
    pub(crate) fn resolve_annotation_text(&self, text: &str) -> Option<InferredType> {
        self.resolver.resolve_text(text)
    }

    /// The AST node occupying `span`, if the oracle indexed one.
    pub(crate) fn node(&self, span: Option<Span>) -> Option<&'m Expr> {
        self.oracle.zip(span).and_then(|(o, span)| o.expr(span))
    }

    /// Does the collection display at `span` check against `declared`?
    ///
    /// Displays are contextually typed ([TYPEINF-SPECIAL-LITERAL-CONTEXT]):
    /// check mode carries the declared element types INWARD, so
    /// `d: dict[str, str] = {"k": v}` judges `v` against `str` instead of
    /// rejecting the whole display under dict invariance. Bottom-up synthesis
    /// alone would type it `dict[LiteralString, ...]` and fire.
    pub(crate) fn display_checks(&self, span: Option<Span>, declared: &InferredType) -> bool {
        let Some(display) = self.node(span) else {
            return false;
        };
        if !matches!(
            display,
            Expr::List(_) | Expr::Dict(_) | Expr::Set(_) | Expr::Tuple(_)
        ) {
            return false;
        }
        self.oracle
            .zip(span)
            .and_then(|(o, span)| o.checks_span(span, declared))
            == Some(true)
    }

    /// Does `inferred` fit `declared` — by assignability, or by a nominal
    /// subclass relationship only the module's class table knows?
    pub(crate) fn fits(&self, inferred: &InferredType, declared: &InferredType) -> bool {
        inferred.is_assignable_to(declared)
            || nominal_subclass_assignable(inferred, declared, self.subtyping)
    }

    /// Is `declared` a target this nominal judgment may rule on at all?
    ///
    /// Structural targets (`Protocol`, `TypedDict`, including inside unions and
    /// containers) need member-level judgment, and a nominal leaf the module
    /// cannot ground (an unresolvable import, a `TypeVar` spelled as a name) is
    /// a question rather than an answer. Firing on either is a false positive
    /// on spec-valid code.
    pub(crate) fn judgeable(&self, declared: &InferredType) -> bool {
        !self.resolver.is_structural_target(declared) && self.grounded(declared)
    }

    /// Is `inferred` EVIDENCE this judgment may reject on? A structural value
    /// (a `TypedDict` fits by schema, a `Protocol` by members) and an
    /// ungrounded nominal leaf (`Self`, an unexpanded `TypeVar`) are
    /// questions, not answers — rejecting on either fires on spec-valid code
    /// ([CHKARCH-CONFORMANCE-MODE]).
    pub(crate) fn evidence(&self, inferred: &InferredType) -> bool {
        !self.resolver.is_structural_target(inferred) && self.grounded_deep(inferred)
    }

    /// Every nominal leaf at ANY depth is grounded — the inferred side has no
    /// abstention downstream, so a doubtful leaf anywhere disqualifies it.
    fn grounded_deep(&self, ty: &InferredType) -> bool {
        match ty {
            InferredType::Named(name) => self.resolver.is_grounded_name(name),
            InferredType::List(inner)
            | InferredType::Set(inner)
            | InferredType::Optional(inner)
            | InferredType::TypeForm(inner)
            | InferredType::Guard { inner, .. } => self.grounded_deep(inner),
            InferredType::Dict(key, value) => self.grounded_deep(key) && self.grounded_deep(value),
            InferredType::Tuple(items) | InferredType::Union(items) => {
                items.iter().all(|item| self.grounded_deep(item))
            }
            InferredType::Generator(yielded, sent, returned) => {
                self.grounded_deep(yielded)
                    && self.grounded_deep(sent)
                    && self.grounded_deep(returned)
            }
            InferredType::Callable(info) => {
                info.param_types
                    .iter()
                    .all(|param| self.grounded_deep(param))
                    && self.grounded_deep(&info.return_type)
            }
            _ => true,
        }
    }

    /// Every top-level nominal leaf (through unions and optionals) is grounded.
    fn grounded(&self, declared: &InferredType) -> bool {
        match declared {
            InferredType::Named(name) => self.resolver.is_grounded_name(name),
            InferredType::Union(arms) => arms.iter().all(|arm| self.grounded(arm)),
            InferredType::Optional(inner) => self.grounded(inner),
            _ => true,
        }
    }
}

// ##########################################################################
// # DELETED — `nominal_subclass_assignable` and `nominal_leaf`.            #
// # DO NOT RESTORE EITHER. DO NOT LEAVE A PLACEHOLDER THAT ANSWERS         #
// # `true` OR `false` IN THEIR PLACE. DO NOT VENDOR THEM UNDER NEW NAMES.  #
// #                                                                        #
// # `nominal_leaf` was a SPELLING TABLE. It converted a resolved type back #
// # into a string — `InferredType::Int => "int"`, `List(_) => "list"` —    #
// # and `Named(name) => name.split('[').next()`, parsing a rendered        #
// # generic by splitting it at a bracket.                                  #
// #                                                                        #
// # `nominal_subclass_assignable` then compared those strings, and settled #
// # ENUM MEMBERSHIP with:                                                  #
// #                                                                        #
// #     sub.strip_prefix(sup).is_some_and(|rest| rest.starts_with('.'))    #
// #                                                                        #
// # — a text operation standing in for "is this member declared by that    #
// # enum?". Any class whose name merely began with the target's name and   #
// # a dot was accepted as a member of it; a genuine member reached under   #
// # an alias was rejected. The question is about DECLARATIONS and must be  #
// # answered from the resolver's class/enum tables by symbol identity.     #
// #                                                                        #
// # `TypeJudge::fits` is LEFT BROKEN ON PURPOSE — it is the map of what    #
// # must be rebuilt on canonical `TypeNode` + `assignable`, whose `None`   #
// # is an honest abstention.                                               #
// #                                                                        #
// # Pinned by: tests/nominal_spelling_surgery_pin_tests.rs                 #
// ##########################################################################
