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
use crate::types::InferredType;

use super::nominal::NominalHierarchy;
use super::oracle::ModuleOracle;

/// The module's type judgment, borrowed from the shared context.
pub(crate) struct TypeJudge<'m, 'a> {
    oracle: Option<&'a ModuleOracle<'m>>,
    resolver: &'a AnnotationResolver<'m>,
    nominal: &'a NominalHierarchy<'m>,
}

impl<'m, 'a> TypeJudge<'m, 'a> {
    /// Borrow the judgment from a module's shared type context.
    pub(crate) fn new(
        oracle: Option<&'a ModuleOracle<'m>>,
        resolver: &'a AnnotationResolver<'m>,
        nominal: &'a NominalHierarchy<'m>,
    ) -> Self {
        Self {
            oracle,
            resolver,
            nominal,
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

    /// The declared type of the annotation node covering `span`, resolved
    /// through the cascade. `None` when no annotation node has exactly that
    /// span — the caller then has nothing to judge and must stay silent.
    pub(crate) fn declared_at(&self, span: Span) -> Option<InferredType> {
        self.resolver.resolve_span(span)
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

    /// Does `inferred` fit `declared`?
    ///
    /// Two halves: the structural relations `InferredType` knows (the builtin
    /// tower, literals, unions, containers), then the nominal subclass walk
    /// over the module's resolved class hierarchy.
    pub(crate) fn fits(&self, inferred: &InferredType, declared: &InferredType) -> bool {
        // Two nominal leaves are the NOMINAL layer's question. `InferredType`
        // holds a rendering, not a resolved identity, so `is_assignable_to`
        // has nothing lawful to decide them with and deliberately panics
        // rather than compare characters.
        //
        // THIS GUARD IS TOP-LEVEL ONLY, AND THAT IS A LIVE BUG. It catches
        // `Named` vs `Named` and nothing else, so every NESTED pair still
        // walks into `is_assignable_to` and reaches the panic in production:
        // `list[Cairn]` against `list[Waypoint]`, `dict[str, Cairn]`, a tuple
        // element, a callable parameter or return, a `TypeIs[...]` operand.
        // Anything that recurses past the outermost constructor is unprotected.
        //
        // The guard is not the fix and must not be extended into one — a
        // deeper walk here would be the same text-derived comparison one
        // level down. The fix is a nominal leaf that carries its definition
        // site, after which `is_assignable_to` can decide the pair itself at
        // any depth.
        if matches!(
            (inferred, declared),
            (InferredType::Named(_), InferredType::Named(_))
        ) {
            return nominal_subclass_assignable(inferred, declared, self.nominal);
        }
        inferred.is_assignable_to(declared)
            || nominal_subclass_assignable(inferred, declared, self.nominal)
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

/// Is `inferred` assignable to `declared` because one names a SUBCLASS of the
/// other, or a member of the enum the other names?
///
/// REBUILT on resolved class identity. The deleted version rendered both
/// types back into strings through a spelling table (`InferredType::Int =>
/// "int"`, `Named(name) => name.split('[').next()`) and compared the text,
/// settling enum membership with
/// `sub.strip_prefix(sup).is_some_and(|rest| rest.starts_with('.'))` — so any
/// class whose name merely began with the target's and a dot was accepted,
/// and a genuine member reached under an alias was rejected.
///
/// Here both leaves resolve through the module's binding table to class
/// definitions and the answer comes from the resolved hierarchy. Only the
/// `Named` leaves are handled: every other pair is a relation
/// [`InferredType::is_assignable_to`] already owns, and `false` here means
/// "no NOMINAL evidence", never "these types conflict".
pub(crate) fn nominal_subclass_assignable(
    inferred: &InferredType,
    declared: &InferredType,
    nominal: &NominalHierarchy<'_>,
) -> bool {
    let (InferredType::Named(sub), InferredType::Named(sup)) = (inferred, declared) else {
        return false;
    };
    // No `sub == sup` shortcut. Same spelling is not same definition — a
    // module may declare two classes with one name — and one definition
    // reached under two names has two spellings. Both cases are answered
    // below by resolving each leaf to a definition site.
    nominal
        .is_subclass(sub, sup)
        .or_else(|| nominal.is_declared_member(sub, sup))
        // Neither leaf resolves to a class this module defines, or the walk
        // hit an edge it could not follow. There is no resolved identity to
        // compare, so this is not evidence of a conflict: stay silent rather
        // than report a mismatch the checker cannot substantiate
        // ([CHKARCH-CONFORMANCE-MODE]).
        .unwrap_or(true)
}
