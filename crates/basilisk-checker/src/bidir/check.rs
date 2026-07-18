//! Implements [TYPEINF-TARGET-BIDIRECTIONAL]. See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-BIDIRECTIONAL
//! The bidirectional engine — checking mode (`check(e, τ)`), the primary
//! driver.
//!
//! Expected-type context flows top-down into container literals,
//! comprehensions, lambda parameters, and call arguments; only when no
//! checking rule applies does the engine fall back to synthesis plus one
//! recorded `synth(e) <: τ` constraint.

use ruff_python_ast::Expr;
use ruff_text_size::Ranged;

use crate::types::InferredType;

use super::constraints::ConstraintReason;
use super::engine::BidirEngine;
use super::ty::Ty;

impl BidirEngine {
    /// Checking mode: verify `expr` against the `expected` type, threading
    /// the expectation into sub-expressions wherever a rule applies.
    pub fn check(&mut self, expr: &Expr, expected: &Ty) {
        self.check_with_reason(expr, expected, ConstraintReason::ExpectedType);
    }

    /// [`Self::check`] with an explicit constraint reason for the fallback
    /// (and nested) obligations.
    pub(super) fn check_with_reason(
        &mut self,
        expr: &Expr,
        expected: &Ty,
        reason: ConstraintReason,
    ) {
        match (expr, expected) {
            (Expr::List(list), Ty::List(elem)) => self.check_elements(&list.elts, elem),
            (Expr::Set(set), Ty::Set(elem)) => self.check_elements(&set.elts, elem),
            (Expr::Dict(dict), Ty::Dict(key, value)) => self.check_dict(dict, key, value),
            (Expr::Tuple(tuple), Ty::Tuple(elems)) => {
                self.check_tuple(expr, tuple, elems, expected, reason);
            }
            (Expr::Lambda(lambda), Ty::Callable(params, ret)) => {
                self.check_lambda(lambda, params, ret);
            }
            (Expr::If(ternary), _) => self.check_ternary(ternary, expected, reason),
            (Expr::Named(walrus), _) => self.check_walrus(walrus, expected),
            (Expr::Call(call), _) => self.check_call(call, expected),
            (Expr::ListComp(comp), Ty::List(elem)) => {
                self.check_comprehension(&comp.generators, &comp.elt, elem);
            }
            (Expr::SetComp(comp), Ty::Set(elem)) => {
                self.check_comprehension(&comp.generators, &comp.elt, elem);
            }
            (Expr::DictComp(comp), Ty::Dict(key, value)) => {
                self.check_dict_comprehension(comp, key, value);
            }
            (_, Ty::Union(alts)) => self.check_against_union(expr, alts, reason),
            _ => self.check_fallback(expr, expected, reason),
        }
    }

    /// Elements of a checked list/set literal are each checked against the
    /// expected element type — context threads inward, no fresh variable.
    fn check_elements(&mut self, elts: &[Expr], elem: &Ty) {
        for elt in elts {
            if let Expr::Starred(starred) = elt {
                let _ = self.synth(&starred.value);
            } else {
                self.check_with_reason(elt, elem, ConstraintReason::CollectionElement);
            }
        }
    }

    /// Keys/values of a checked dict literal thread into the expected types.
    fn check_dict(&mut self, dict: &ruff_python_ast::ExprDict, key: &Ty, value: &Ty) {
        for item in &dict.items {
            match &item.key {
                Some(key_expr) => {
                    self.check_with_reason(key_expr, key, ConstraintReason::DictKey);
                    self.check_with_reason(&item.value, value, ConstraintReason::DictValue);
                }
                None => {
                    let _ = self.synth(&item.value);
                }
            }
        }
    }

    /// A tuple literal against `tuple[T1, .., Tn]` (pairwise) or the
    /// homogeneous `tuple[X, ...]` lift (every element against `X`).
    fn check_tuple(
        &mut self,
        whole: &Expr,
        tuple: &ruff_python_ast::ExprTuple,
        elems: &[Ty],
        expected: &Ty,
        reason: ConstraintReason,
    ) {
        if let Some(homogeneous) = homogeneous_element(elems) {
            let homogeneous = homogeneous.clone();
            self.check_elements(&tuple.elts, &homogeneous);
            return;
        }
        let no_spread = tuple.elts.iter().all(|e| !matches!(e, Expr::Starred(_)));
        if tuple.elts.len() == elems.len() && no_spread {
            let elems = elems.to_vec();
            for (elt, elem) in tuple.elts.iter().zip(&elems) {
                self.check_with_reason(elt, elem, ConstraintReason::CollectionElement);
            }
            return;
        }
        self.check_fallback(whole, expected, reason);
    }

    /// A lambda against `Callable[[P..], R]`: parameters take the expected
    /// **input** types (fresh input variables when the expectation is the
    /// gradual `Callable[..., R]`), and the body checks against `R`.
    fn check_lambda(&mut self, lambda: &ruff_python_ast::ExprLambda, params: &[Ty], ret: &Ty) {
        let names = super::engine::lambda_param_names(lambda);
        let bound: Vec<(String, Ty)> = names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let ty = params
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| Ty::Var(self.fresh_input_var()));
                (name.clone(), ty)
            })
            .collect();
        let body = &lambda.body;
        self.scoped(|engine| {
            for (name, ty) in &bound {
                engine.bind(name, ty.clone());
            }
            engine.check_with_reason(body, ret, ConstraintReason::LambdaBody);
        });
    }

    /// Both branches of a ternary check against the same expectation.
    fn check_ternary(
        &mut self,
        ternary: &ruff_python_ast::ExprIf,
        expected: &Ty,
        reason: ConstraintReason,
    ) {
        let _ = self.synth(&ternary.test);
        self.check_with_reason(&ternary.body, expected, reason);
        self.check_with_reason(&ternary.orelse, expected, reason);
    }

    /// `(name := value)` checks the value and binds the name to it.
    fn check_walrus(&mut self, walrus: &ruff_python_ast::ExprNamed, expected: &Ty) {
        self.check_with_reason(&walrus.value, expected, ConstraintReason::WalrusValue);
        if let Expr::Name(target) = walrus.target.as_ref() {
            self.bind(target.id.as_str(), expected.clone());
        }
    }

    /// A call in checked position: arguments thread through
    /// [`BidirEngine::synth_call`], and the declared return must satisfy the
    /// expectation.
    fn check_call(&mut self, call: &ruff_python_ast::ExprCall, expected: &Ty) {
        let ret = self.synth_call(call);
        self.constraints_push(
            ret,
            expected.clone(),
            call.range(),
            ConstraintReason::CallReturn,
        );
    }

    /// A list/set comprehension in checked position: generators bind, then
    /// the element expression checks against the expected element type.
    fn check_comprehension(
        &mut self,
        generators: &[ruff_python_ast::Comprehension],
        elt: &Expr,
        elem: &Ty,
    ) {
        let elem = elem.clone();
        self.scoped(|engine| {
            engine.bind_generators(generators);
            engine.check_with_reason(elt, &elem, ConstraintReason::ComprehensionElement);
        });
    }

    /// A dict comprehension in checked position.
    fn check_dict_comprehension(
        &mut self,
        comp: &ruff_python_ast::ExprDictComp,
        key: &Ty,
        value: &Ty,
    ) {
        let (key, value) = (key.clone(), value.clone());
        self.scoped(|engine| {
            engine.bind_generators(&comp.generators);
            if let Some(key_expr) = comp.key.as_deref() {
                engine.check_with_reason(key_expr, &key, ConstraintReason::DictKey);
            }
            engine.check_with_reason(&comp.value, &value, ConstraintReason::DictValue);
        });
    }

    /// Against a union: when exactly one alternative structurally matches the
    /// literal's shape, thread context into it; otherwise fall back.
    fn check_against_union(&mut self, expr: &Expr, alts: &[Ty], reason: ConstraintReason) {
        let matching: Vec<&Ty> = alts.iter().filter(|alt| shape_matches(expr, alt)).collect();
        if let [only] = matching.as_slice() {
            let only = (*only).clone();
            self.check_with_reason(expr, &only, reason);
            return;
        }
        self.check_fallback(expr, &Ty::Union(alts.to_vec()), reason);
    }

    /// No checking rule applies: synthesize and record one obligation.
    fn check_fallback(&mut self, expr: &Expr, expected: &Ty, reason: ConstraintReason) {
        let ty = self.synth(expr);
        self.constraints_push(ty, expected.clone(), expr.range(), reason);
    }

    /// Constraint recording, kept private to the engine internals.
    fn constraints_push(
        &mut self,
        sub: Ty,
        sup: Ty,
        range: ruff_text_size::TextRange,
        reason: ConstraintReason,
    ) {
        self.constraints.push(sub, sup, range, reason);
    }

    /// Fresh input-polarity variable (helper keeps borrow scopes tight).
    fn fresh_input_var(&mut self) -> super::tyvar::TyVarId {
        self.vars.fresh(super::tyvar::Polarity::Input)
    }
}

/// The homogeneous-tuple lift: `tuple[X, ...]` arrives as a two-element list
/// whose second entry is the `Named("...")` marker (matching
/// [`InferredType::Tuple`]'s encoding).
fn homogeneous_element(elems: &[Ty]) -> Option<&Ty> {
    match elems {
        [elem, Ty::Ground(InferredType::Named(marker))] if marker == "..." => Some(elem),
        _ => None,
    }
}

/// Whether an expression's literal shape can only mean one union alternative.
fn shape_matches(expr: &Expr, alt: &Ty) -> bool {
    matches!(
        (expr, alt),
        (Expr::List(_) | Expr::ListComp(_), Ty::List(_))
            | (Expr::Set(_) | Expr::SetComp(_), Ty::Set(_))
            | (Expr::Dict(_) | Expr::DictComp(_), Ty::Dict(_, _))
            | (Expr::Tuple(_), Ty::Tuple(_))
            | (Expr::Lambda(_), Ty::Callable(_, _))
    )
}
