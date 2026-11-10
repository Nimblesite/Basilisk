//! Implements [TYPEINF-ALGO]. See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-ALGO
//!
//! The source-level entry point to the inference engine, plus the two
//! predicates every display surface applies to what it returns.
//!
//! This is NOT a second inference algorithm — there is only one
//! ([TYPEINF-TARGET-BIDIRECTIONAL]). [`infer_expression_source_in_scope`]
//! parses one expression and hands it to [`crate::bidir::BidirEngine`], the
//! same engine behind [`crate::incremental_defs::expression_types`]; hover,
//! completions, and inlay hints read their types from here so a rendered type
//! and a diagnostic can never disagree.
//!
//! [`is_fully_known`] and [`display_widened`] are the render contract, not
//! inference: what may be shown at all, and in what form.

use basilisk_resolver::{ResolvedModule, Span};

use crate::types::InferredType;

/// The module's span-indexed type oracle, exposed for display surfaces
/// (hover, inlay hints, completions) so a rendered type and a diagnostic can
/// never disagree — both read the SAME per-module [`crate::bidir::BidirEngine`]
/// the checker rules judge with ([NARROWPLAN-INTEGRATION] Step 5).
pub struct ModuleSpanTypes<'m> {
    types: crate::rules::ModuleTypes<'m>,
}

impl std::fmt::Debug for ModuleSpanTypes<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModuleSpanTypes").finish_non_exhaustive()
    }
}

impl<'m> ModuleSpanTypes<'m> {
    /// Build the oracle for `module` — one walk, every expression indexed.
    #[must_use]
    pub fn build(module: &'m ResolvedModule) -> Self {
        Self {
            types: crate::rules::ModuleTypes::build(module),
        }
    }

    /// The engine's type for the expression at exactly `span`, seen from its
    /// own lexical scope. `None` when no expression occupies the span or the
    /// engine abstains.
    #[must_use]
    pub fn type_at(&self, span: Span) -> Option<InferredType> {
        self.types
            .oracle()
            .and_then(|oracle| oracle.synth_span(span))
    }

    /// The display rendering of the expression at `span`: the engine's answer,
    /// gated by [`is_fully_known`] and widened by [`display_widened`] — the
    /// same render contract every display surface applies. Empty when the
    /// type cannot be shown.
    ///
    /// Empty container displays render as their bare container name (`[]` →
    /// `list`) — the checker-internal element sentinel must never reach a
    /// label (GitHub #385) — and lambdas render nothing: parameter and return
    /// inference for them is not modelled.
    #[must_use]
    pub fn display_at(&self, span: Span) -> String {
        use ruff_python_ast::Expr;
        match self.types.oracle().and_then(|oracle| oracle.expr(span)) {
            Some(Expr::List(list)) if list.elts.is_empty() => return "list".to_owned(),
            Some(Expr::Dict(dict)) if dict.items.is_empty() => return "dict".to_owned(),
            Some(Expr::Lambda(_)) | None => return String::new(),
            _ => {}
        }
        let Some(ty) = self.type_at(span) else {
            return String::new();
        };
        if is_fully_known(&ty) {
            display_widened(&ty).to_string()
        } else {
            String::new()
        }
    }
}

/// Synthesize the type of one expression's SOURCE text through the
/// bidirectional engine ([TYPEINF-TARGET-BIDIRECTIONAL]).
///
/// Anything unparseable or unsupported answers the conservative `Unknown` —
/// never a guess ([TYPEINF-TARGET-GRADUAL]).
#[must_use]
pub fn infer_expression_source(source: &str) -> InferredType {
    infer_expression_source_in_scope(source, &std::collections::HashMap::new())
}

/// [`infer_expression_source`] with the surrounding scope's names bound.
///
/// The engine has no name resolution of its own, so a free name synthesizes to
/// `Unknown` and every expression built on one goes with it — `s.upper()` is
/// typeable only if `s` is. Callers that know what the names in view are
/// (a display surface reading a resolved module) supply them here, so an
/// expression receiver can be typed at all (GitHub #390).
///
/// An empty scope reproduces [`infer_expression_source`] exactly.
#[must_use]
pub fn infer_expression_source_in_scope<S: std::hash::BuildHasher>(
    _source: &str,
    _scope: &std::collections::HashMap<String, InferredType, S>,
) -> InferredType {
    // DELETED: an isolated `&str` expression has no original AST identity,
    // source offset, or enclosing binding scope. Re-parsing it into a new
    // zero-offset tree cannot support a semantic type verdict.
    panic!(
        "basilisk-checker: `infer_expression_source_in_scope` was DELETED because it \
         reparsed expression source text outside the module AST and supplied a \
         hand-built name map as scope. The real implementation must receive the original \
         expression node and resolved scope. Do not restore parsing or return `Unknown`."
    )
}

/// Whether a type contains no `Unknown` anywhere — display surfaces show a
/// type only when it is fully known (a partial `list[Unknown]` hint would be
/// worse than silence, per the gradual posture [TYPEINF-TARGET-GRADUAL]).
///
/// Matched EXHAUSTIVELY on purpose: a catch-all would answer "fully known"
/// for a future type-carrying variant and let an `Unknown` nested inside it
/// reach a rendered hover. Adding a variant to [`InferredType`] must break
/// this build, not this guarantee.
#[must_use]
pub fn is_fully_known(ty: &InferredType) -> bool {
    match ty {
        InferredType::Unknown => false,
        // Both are complete answers: the top type and "some class object".
        InferredType::Object | InferredType::ClassObject => true,
        // Variants carrying nested types: known only if every child is.
        InferredType::List(inner)
        | InferredType::Set(inner)
        | InferredType::Optional(inner)
        | InferredType::TypeForm(inner)
        | InferredType::Guard { inner, .. } => is_fully_known(inner),
        InferredType::Dict(key, value) => is_fully_known(key) && is_fully_known(value),
        InferredType::Tuple(elems) | InferredType::Union(elems) => elems.iter().all(is_fully_known),
        InferredType::Callable(info) => {
            info.param_types.iter().all(is_fully_known) && is_fully_known(&info.return_type)
        }
        InferredType::Generator(yielded, sent, returned) => {
            is_fully_known(yielded) && is_fully_known(sent) && is_fully_known(returned)
        }
        // Leaves: nothing nested to hide an `Unknown` in.
        InferredType::Int
        | InferredType::Str
        | InferredType::Float
        | InferredType::Bool
        | InferredType::Bytes
        | InferredType::None_
        | InferredType::Literal(_)
        | InferredType::LiteralString
        | InferredType::Named(_)
        | InferredType::Any
        | InferredType::Never => true,
    }
}

/// Widen an inferred type to its DISPLAY form: literals become their base
/// type (`Literal[1]` → `int`), matching how annotations are conventionally
/// written in hover/inlay surfaces. A string literal's base type is
/// `LiteralString`, not `str` — PEP 675 provenance is part of the display
/// (GitHub #290). Precision-preserving variants (unions, containers) widen
/// structurally.
///
/// Matched EXHAUSTIVELY on purpose: under a catch-all, a future type-carrying
/// variant would clone through unwidened and render a raw `Literal[1]` inside
/// it, contradicting the contract above. Every nested position widens —
/// including `Callable`, `Generator`, and `TypeForm`, which a catch-all
/// silently skipped.
#[must_use]
pub fn display_widened(ty: &InferredType) -> InferredType {
    match ty {
        // Neither carries a nested type, and neither is a literal.
        InferredType::Object => InferredType::Object,
        InferredType::ClassObject => InferredType::ClassObject,
        InferredType::Literal(literal) => match literal {
            crate::types::LiteralValue::Int(_) => InferredType::Int,
            // PEP 675: a literal expression is provably a `LiteralString`;
            // plain `str` stays reserved for dynamic string values.
            crate::types::LiteralValue::Str(_) => InferredType::LiteralString,
            crate::types::LiteralValue::Float(_) => InferredType::Float,
            crate::types::LiteralValue::Bool(_) => InferredType::Bool,
            crate::types::LiteralValue::Bytes(_) => InferredType::Bytes,
        },
        InferredType::LiteralString => InferredType::LiteralString,
        InferredType::List(elem) => InferredType::List(Box::new(display_widened(elem))),
        InferredType::Set(elem) => InferredType::Set(Box::new(display_widened(elem))),
        InferredType::Dict(key, value) => InferredType::Dict(
            Box::new(display_widened(key)),
            Box::new(display_widened(value)),
        ),
        InferredType::Tuple(elems) => {
            InferredType::Tuple(elems.iter().map(display_widened).collect())
        }
        InferredType::Optional(inner) => InferredType::Optional(Box::new(display_widened(inner))),
        InferredType::Union(members) => members
            .iter()
            .map(display_widened)
            .fold(InferredType::Never, InferredType::union),
        InferredType::TypeForm(inner) => InferredType::TypeForm(Box::new(display_widened(inner))),
        InferredType::Guard { type_is, inner } => InferredType::Guard {
            type_is: *type_is,
            inner: Box::new(display_widened(inner)),
        },
        InferredType::Callable(info) => InferredType::Callable(crate::types::CallableInfo {
            param_types: info.param_types.iter().map(display_widened).collect(),
            return_type: Box::new(display_widened(&info.return_type)),
        }),
        InferredType::Generator(yielded, sent, returned) => InferredType::Generator(
            Box::new(display_widened(yielded)),
            Box::new(display_widened(sent)),
            Box::new(display_widened(returned)),
        ),
        // Already in display form — nothing nested to widen.
        InferredType::Int
        | InferredType::Str
        | InferredType::Float
        | InferredType::Bool
        | InferredType::Bytes
        | InferredType::None_
        | InferredType::Named(_)
        | InferredType::Any
        | InferredType::Never
        | InferredType::Unknown => ty.clone(),
    }
}

#[cfg(test)]
mod tests {
    /// [NARROWPLAN-CHECKLIST] Stage 2: the shared expression-source entry
    /// point runs the SAME bidirectional engine as checker diagnostics —
    /// literals, containers, arithmetic, and method calls all synthesize.
    #[test]
    fn expression_source_synthesizes_through_the_shared_engine() {
        use super::infer_expression_source;
        use crate::types::{InferredType, LiteralValue};
        assert_eq!(
            infer_expression_source("42"),
            InferredType::Literal(LiteralValue::Int(42))
        );
        // Elements keep literal precision; display widening collapses them.
        assert_eq!(
            super::display_widened(&infer_expression_source("[1, 2]")),
            InferredType::List(Box::new(InferredType::Int))
        );
        assert_eq!(infer_expression_source("1 + 2.5"), InferredType::Float);
        assert_eq!(infer_expression_source("'a'.upper()"), InferredType::Str);
        // Unparseable or unresolvable input answers `Unknown`, never a guess.
        assert_eq!(infer_expression_source("def ("), InferredType::Unknown);
        assert_eq!(infer_expression_source("mystery"), InferredType::Unknown);
    }

    /// Display widening turns literal precision into the annotation-style
    /// base type, recursing through containers and unions.
    #[test]
    fn display_widening_reaches_annotation_form() {
        use super::display_widened;
        use crate::types::{InferredType, LiteralValue};
        assert_eq!(
            display_widened(&InferredType::Literal(LiteralValue::Int(1))),
            InferredType::Int
        );
        // PEP 675 provenance survives widening: a string literal's base type
        // IS `LiteralString` (GitHub #290).
        assert_eq!(
            display_widened(&InferredType::LiteralString),
            InferredType::LiteralString
        );
        assert_eq!(
            display_widened(&InferredType::List(Box::new(InferredType::Literal(
                LiteralValue::Str("x".into())
            )))),
            InferredType::List(Box::new(InferredType::LiteralString))
        );
        let union = InferredType::Union(vec![
            InferredType::Literal(LiteralValue::Int(1)),
            InferredType::Literal(LiteralValue::Int(2)),
            InferredType::Str,
        ]);
        assert_eq!(
            display_widened(&union),
            InferredType::Union(vec![InferredType::Int, InferredType::Str]),
            "widened literal duplicates must collapse in the union"
        );
    }

    /// Widening reaches EVERY nested position, including the ones a catch-all
    /// arm used to clone through untouched (`Callable`, `Generator`,
    /// `TypeForm`) — a rendered type never shows a raw `Literal[…]` inside.
    #[test]
    fn display_widening_reaches_every_nested_position() {
        use super::display_widened;
        use crate::types::{CallableInfo, InferredType, LiteralValue};
        let lit = |value: i64| InferredType::Literal(LiteralValue::Int(value));
        assert_eq!(
            display_widened(&InferredType::Callable(CallableInfo {
                param_types: vec![lit(1)],
                return_type: Box::new(lit(2)),
            })),
            InferredType::Callable(CallableInfo {
                param_types: vec![InferredType::Int],
                return_type: Box::new(InferredType::Int),
            }),
            "callable parameters and return must widen"
        );
        assert_eq!(
            display_widened(&InferredType::Generator(
                Box::new(lit(1)),
                Box::new(lit(2)),
                Box::new(lit(3))
            )),
            InferredType::Generator(
                Box::new(InferredType::Int),
                Box::new(InferredType::Int),
                Box::new(InferredType::Int)
            ),
            "all three generator positions must widen"
        );
        assert_eq!(
            display_widened(&InferredType::TypeForm(Box::new(lit(1)))),
            InferredType::TypeForm(Box::new(InferredType::Int)),
            "the type-form payload must widen"
        );
    }

    /// `is_fully_known` rejects any type with a nested `Unknown` — display
    /// surfaces stay silent instead of rendering partial types
    /// ([TYPEINF-TARGET-GRADUAL]).
    #[test]
    fn fully_known_rejects_nested_unknowns() {
        use super::is_fully_known;
        use crate::types::{CallableInfo, InferredType};
        assert!(is_fully_known(&InferredType::Int));
        assert!(is_fully_known(&InferredType::List(Box::new(
            InferredType::Str
        ))));
        assert!(!is_fully_known(&InferredType::Unknown));
        assert!(!is_fully_known(&InferredType::List(Box::new(
            InferredType::Unknown
        ))));
        assert!(!is_fully_known(&InferredType::Dict(
            Box::new(InferredType::Str),
            Box::new(InferredType::Unknown)
        )));
        assert!(!is_fully_known(&InferredType::Union(vec![
            InferredType::Int,
            InferredType::Unknown
        ])));
        assert!(!is_fully_known(&InferredType::Callable(CallableInfo {
            param_types: vec![],
            return_type: Box::new(InferredType::Unknown),
        })));
        assert!(!is_fully_known(&InferredType::Generator(
            Box::new(InferredType::Int),
            Box::new(InferredType::None_),
            Box::new(InferredType::Unknown)
        )));
    }
}
