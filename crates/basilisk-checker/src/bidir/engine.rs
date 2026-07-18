//! Implements [TYPEINF-TARGET-BIDIRECTIONAL]. See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-BIDIRECTIONAL
//! The bidirectional engine — synthesis mode (`synth(e) → τ`).
//!
//! Every AST expression node supports both modes: [`BidirEngine::synth`]
//! infers bottom-up, and [`BidirEngine::check`] (in [`super::check`]) drives
//! top-down with an expected type. Neither mode judges subtyping directly —
//! they only allocate variables and record constraints
//! ([TYPEINF-TARGET-CONSTRAINTS]); [`super::solve`] discharges them.

use std::collections::HashMap;

use ruff_python_ast::{Expr, Number, Operator, UnaryOp};
use ruff_text_size::Ranged;

use crate::types::{InferredType, LiteralValue};

use super::constraints::{ConstraintReason, ConstraintSet};
use super::solve::{solve, Solution};
use super::ty::Ty;
use super::tyvar::{Polarity, TyVarStore};

/// One inference run's mutable state: variables, recorded constraints, and a
/// stack of lexical scopes (module/function bindings, lambda parameters,
/// comprehension targets).
#[derive(Debug, Default)]
pub struct BidirEngine {
    pub(super) vars: TyVarStore,
    pub(super) constraints: ConstraintSet,
    scopes: Vec<HashMap<String, Ty>>,
}

impl BidirEngine {
    /// Start an engine whose outermost scope holds the given bindings
    /// (typically parameter and local annotations).
    #[must_use]
    pub fn new(globals: HashMap<String, Ty>) -> Self {
        Self {
            vars: TyVarStore::default(),
            constraints: ConstraintSet::default(),
            scopes: vec![globals],
        }
    }

    /// Solve everything recorded so far and return the outcome.
    #[must_use]
    pub fn finish(self) -> Solution {
        solve(self.vars, self.constraints.into_vec())
    }

    /// Look a name up through the scope stack, innermost first.
    pub(super) fn lookup(&self, name: &str) -> Option<Ty> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    /// Bind a name in the innermost scope.
    pub(super) fn bind(&mut self, name: &str, ty: Ty) {
        if let Some(scope) = self.scopes.last_mut() {
            let _ = scope.insert(name.to_owned(), ty);
        }
    }

    /// Run `body` inside a fresh child scope.
    pub(super) fn scoped<R>(&mut self, body: impl FnOnce(&mut Self) -> R) -> R {
        self.scopes.push(HashMap::new());
        let result = body(self);
        let _ = self.scopes.pop();
        result
    }

    /// Synthesis mode: infer a type for `expr` bottom-up, recording
    /// constraints for anything that flows into a fresh variable.
    pub fn synth(&mut self, expr: &Expr) -> Ty {
        match expr {
            Expr::NumberLiteral(num) => synth_number(&num.value),
            Expr::BooleanLiteral(lit) => {
                Ty::Ground(InferredType::Literal(LiteralValue::Bool(lit.value)))
            }
            Expr::StringLiteral(lit) => Ty::Ground(InferredType::Literal(LiteralValue::Str(
                lit.value.to_str().to_owned(),
            ))),
            Expr::BytesLiteral(_) => Ty::Ground(InferredType::Bytes),
            Expr::FString(_) => Ty::Ground(InferredType::Str),
            Expr::NoneLiteral(_) => Ty::Ground(InferredType::None_),
            Expr::EllipsisLiteral(_) => Ty::Ground(InferredType::Named("ellipsis".to_owned())),
            Expr::Name(name) => self.lookup(name.id.as_str()).unwrap_or_else(Ty::unknown),
            Expr::List(list) => self.synth_elements(&list.elts, expr, Ty::List),
            Expr::Set(set) => self.synth_elements(&set.elts, expr, Ty::Set),
            Expr::Dict(dict) => self.synth_dict(dict, expr),
            Expr::Tuple(tuple) => self.synth_tuple(tuple),
            Expr::Lambda(lambda) => self.synth_lambda(lambda),
            Expr::If(ternary) => self.synth_ternary(ternary),
            Expr::Named(walrus) => self.synth_walrus(walrus),
            Expr::Call(call) => self.synth_call(call),
            Expr::Compare(_) => Ty::Ground(InferredType::Bool),
            Expr::BoolOp(bool_op) => self.synth_bool_op(&bool_op.values),
            Expr::UnaryOp(unary) => self.synth_unary(unary),
            Expr::BinOp(bin_op) => self.synth_bin_op(bin_op),
            Expr::ListComp(comp) => self.synth_comprehension(&comp.generators, &comp.elt, Ty::List),
            Expr::SetComp(comp) => self.synth_comprehension(&comp.generators, &comp.elt, Ty::Set),
            Expr::DictComp(comp) => self.synth_dict_comprehension(comp),
            Expr::Subscript(subscript) => self.synth_subscript(subscript),
            _ => Ty::unknown(),
        }
    }

    /// `[e1, ..]` / `{e1, ..}`: elements flow into a fresh output variable —
    /// `list[Var{lower=…}]`, settling only at first constraining use
    /// (deferred generalization, [TYPEINF-TARGET-CONSTRAINTS]).
    fn synth_elements(
        &mut self,
        elts: &[Expr],
        whole: &Expr,
        wrap: impl FnOnce(Box<Ty>) -> Ty,
    ) -> Ty {
        let elem = self.vars.fresh(Polarity::Output);
        for elt in elts {
            let ty = self.synth_spread_aware(elt);
            self.constraints.push(
                ty,
                Ty::Var(elem),
                elt.range(),
                ConstraintReason::CollectionElement,
            );
        }
        let _ = whole;
        wrap(Box::new(Ty::Var(elem)))
    }

    /// A starred element spreads an iterable whose element type Stage 0 does
    /// not extract — it contributes the conservative `Unknown`.
    fn synth_spread_aware(&mut self, elt: &Expr) -> Ty {
        if let Expr::Starred(starred) = elt {
            let _ = self.synth(&starred.value);
            return Ty::unknown();
        }
        self.synth(elt)
    }

    /// `{k: v, ..}`: keys and values flow into fresh output variables.
    fn synth_dict(&mut self, dict: &ruff_python_ast::ExprDict, whole: &Expr) -> Ty {
        let key_var = self.vars.fresh(Polarity::Output);
        let value_var = self.vars.fresh(Polarity::Output);
        for item in &dict.items {
            let Some(key) = &item.key else {
                // `**spread` merges a mapping Stage 0 does not decompose.
                let _ = self.synth(&item.value);
                self.constraints.push(
                    Ty::unknown(),
                    Ty::Var(key_var),
                    whole.range(),
                    ConstraintReason::DictKey,
                );
                continue;
            };
            let key_ty = self.synth(key);
            self.constraints.push(
                key_ty,
                Ty::Var(key_var),
                key.range(),
                ConstraintReason::DictKey,
            );
            let value_ty = self.synth(&item.value);
            self.constraints.push(
                value_ty,
                Ty::Var(value_var),
                item.value.range(),
                ConstraintReason::DictValue,
            );
        }
        Ty::Dict(Box::new(Ty::Var(key_var)), Box::new(Ty::Var(value_var)))
    }

    /// `(e1, ..)`: heterogeneous — each element keeps its own type.
    fn synth_tuple(&mut self, tuple: &ruff_python_ast::ExprTuple) -> Ty {
        Ty::Tuple(
            tuple
                .elts
                .iter()
                .map(|elt| self.synth_spread_aware(elt))
                .collect(),
        )
    }

    /// `lambda p, ..: body`: parameters become fresh **input** variables in a
    /// child scope; the body synthesizes under them.
    fn synth_lambda(&mut self, lambda: &ruff_python_ast::ExprLambda) -> Ty {
        let names = lambda_param_names(lambda);
        let params: Vec<Ty> = names
            .iter()
            .map(|_| Ty::Var(self.vars.fresh(Polarity::Input)))
            .collect();
        let body = self.scoped(|engine| {
            for (name, ty) in names.iter().zip(&params) {
                engine.bind(name, ty.clone());
            }
            engine.synth(&lambda.body)
        });
        Ty::Callable(params, Box::new(body))
    }

    /// `a if cond else b`: the union of both branches.
    fn synth_ternary(&mut self, ternary: &ruff_python_ast::ExprIf) -> Ty {
        let _ = self.synth(&ternary.test);
        let body = self.synth(&ternary.body);
        let orelse = self.synth(&ternary.orelse);
        Ty::Union(vec![body, orelse])
    }

    /// `(name := value)`: the value's type, also bound to `name`.
    fn synth_walrus(&mut self, walrus: &ruff_python_ast::ExprNamed) -> Ty {
        let value = self.synth(&walrus.value);
        if let Expr::Name(target) = walrus.target.as_ref() {
            self.bind(target.id.as_str(), value.clone());
        }
        value
    }

    /// `f(a, ..)`: when the callee synthesizes to a callable, arguments are
    /// **checked** against its declared parameters (expected types thread
    /// into call arguments — check as the primary driver) and the declared
    /// return is the result; otherwise the conservative `Unknown`.
    pub(super) fn synth_call(&mut self, call: &ruff_python_ast::ExprCall) -> Ty {
        let callee = self.synth(&call.func);
        let Ty::Callable(params, ret) = callee else {
            for arg in &call.arguments.args {
                let _ = self.synth(arg);
            }
            return self.non_callable_call_result(call, &callee);
        };
        for (arg, param) in call.arguments.args.iter().zip(&params) {
            self.check_with_reason(arg, param, ConstraintReason::CallArgument);
        }
        for arg in call.arguments.args.iter().skip(params.len()) {
            let _ = self.synth(arg);
        }
        *ret
    }

    /// A call whose callee did not synthesize to a `Callable`: builtin
    /// functions/constructors (via the centralized table in
    /// [`super::builtins`]), builtin METHOD calls on known receivers, and
    /// user-class constructors (`Named` callee → `Named` instance).
    fn non_callable_call_result(&mut self, call: &ruff_python_ast::ExprCall, callee: &Ty) -> Ty {
        // Constructor: a class object used as a callee yields an instance.
        // (`Named` deliberately conflates class/instance at Stage 2 — the
        // display value is right and no rule enforces it yet.)
        if let Ty::Ground(InferredType::Named(name)) = callee {
            return Ty::Ground(InferredType::Named(name.clone()));
        }
        match call.func.as_ref() {
            Expr::Name(name) => super::builtins::builtin_call_return(name.id.as_str())
                .map_or_else(Ty::unknown, Ty::Ground),
            Expr::Attribute(attribute) => {
                let receiver = self.synth(&attribute.value).to_inferred(&self.vars);
                super::builtins::builtin_method_return(&receiver, attribute.attr.as_str())
                    .map_or_else(Ty::unknown, Ty::Ground)
            }
            _ => Ty::unknown(),
        }
    }

    /// `x[i]`: element extraction for known container shapes.
    fn synth_subscript(&mut self, subscript: &ruff_python_ast::ExprSubscript) -> Ty {
        let receiver = self.synth(&subscript.value);
        let index = self.synth(&subscript.slice);
        match (&receiver, &index) {
            (Ty::List(elem), _) if !matches!(subscript.slice.as_ref(), Expr::Slice(_)) => {
                (**elem).clone()
            }
            (Ty::List(_), _) => receiver.clone(),
            (Ty::Dict(_, value), _) => (**value).clone(),
            (Ty::Tuple(elems), Ty::Ground(InferredType::Literal(LiteralValue::Int(position)))) => {
                usize::try_from(*position)
                    .ok()
                    .and_then(|index| elems.get(index).cloned())
                    .unwrap_or_else(Ty::unknown)
            }
            (Ty::Ground(InferredType::Str | InferredType::LiteralString), _) => {
                Ty::Ground(InferredType::Str)
            }
            _ => Ty::unknown(),
        }
    }

    /// `a or b`, `a and b`: the union of operand types (truthiness-based
    /// refinement is Stage 2 narrowing, not Stage 0).
    fn synth_bool_op(&mut self, values: &[Expr]) -> Ty {
        Ty::Union(values.iter().map(|value| self.synth(value)).collect())
    }

    /// Unary operators preserve literal precision for integer negation.
    fn synth_unary(&mut self, unary: &ruff_python_ast::ExprUnaryOp) -> Ty {
        let operand = self.synth(&unary.operand);
        match (unary.op, operand) {
            (UnaryOp::Not, _) => Ty::Ground(InferredType::Bool),
            (UnaryOp::USub, Ty::Ground(InferredType::Literal(LiteralValue::Int(n)))) => {
                Ty::Ground(InferredType::Literal(LiteralValue::Int(n.wrapping_neg())))
            }
            (UnaryOp::USub | UnaryOp::UAdd | UnaryOp::Invert, ty) => numeric_result(&ty),
        }
    }

    /// Binary operators: the small conservative table Stage 0 supports.
    fn synth_bin_op(&mut self, bin_op: &ruff_python_ast::ExprBinOp) -> Ty {
        let left = self.synth(&bin_op.left);
        let right = self.synth(&bin_op.right);
        bin_op_result(bin_op.op, &left, &right)
    }

    /// `[elt for tgt in iter ..]` / `{elt for ..}`.
    fn synth_comprehension(
        &mut self,
        generators: &[ruff_python_ast::Comprehension],
        elt: &Expr,
        wrap: impl FnOnce(Box<Ty>) -> Ty,
    ) -> Ty {
        let elem = self.vars.fresh(Polarity::Output);
        self.scoped(|engine| {
            engine.bind_generators(generators);
            let ty = engine.synth(elt);
            engine.constraints.push(
                ty,
                Ty::Var(elem),
                elt.range(),
                ConstraintReason::ComprehensionElement,
            );
        });
        wrap(Box::new(Ty::Var(elem)))
    }

    /// `{k: v for ..}`.
    fn synth_dict_comprehension(&mut self, comp: &ruff_python_ast::ExprDictComp) -> Ty {
        let key_var = self.vars.fresh(Polarity::Output);
        let value_var = self.vars.fresh(Polarity::Output);
        self.scoped(|engine| {
            engine.bind_generators(&comp.generators);
            if let Some(key_expr) = comp.key.as_deref() {
                let key = engine.synth(key_expr);
                engine.constraints.push(
                    key,
                    Ty::Var(key_var),
                    key_expr.range(),
                    ConstraintReason::DictKey,
                );
            }
            let value = engine.synth(&comp.value);
            engine.constraints.push(
                value,
                Ty::Var(value_var),
                comp.value.range(),
                ConstraintReason::DictValue,
            );
        });
        Ty::Dict(Box::new(Ty::Var(key_var)), Box::new(Ty::Var(value_var)))
    }

    /// Bind each generator's target from its iterable's element type.
    pub(super) fn bind_generators(&mut self, generators: &[ruff_python_ast::Comprehension]) {
        for generator in generators {
            let iterable = self.synth(&generator.iter);
            let element = iteration_element(&iterable);
            self.bind_target(&generator.target, &element);
            for guard in &generator.ifs {
                let _ = self.synth(guard);
            }
        }
    }

    /// Bind a `for` target (name or tuple of names) to an element type.
    fn bind_target(&mut self, target: &Expr, element: &Ty) {
        match (target, element) {
            (Expr::Name(name), _) => self.bind(name.id.as_str(), element.clone()),
            (Expr::Tuple(names), Ty::Tuple(elems)) if names.elts.len() == elems.len() => {
                for (name, elem) in names.elts.iter().zip(elems) {
                    self.bind_target(name, elem);
                }
            }
            (Expr::Tuple(names), _) => {
                for name in &names.elts {
                    self.bind_target(name, &Ty::unknown());
                }
            }
            _ => {}
        }
    }
}

/// Number literals keep integer literal precision; floats stay ground.
fn synth_number(value: &Number) -> Ty {
    match value {
        Number::Int(int) => int.as_i64().map_or(Ty::Ground(InferredType::Int), |n| {
            Ty::Ground(InferredType::Literal(LiteralValue::Int(n)))
        }),
        Number::Float(_) => Ty::Ground(InferredType::Float),
        Number::Complex { .. } => Ty::Ground(InferredType::Named("complex".to_owned())),
    }
}

/// What iterating over a value yields, when Stage 0 can tell.
fn iteration_element(iterable: &Ty) -> Ty {
    match iterable {
        Ty::List(elem) | Ty::Set(elem) => (**elem).clone(),
        Ty::Dict(key, _) => (**key).clone(),
        Ty::Ground(InferredType::List(elem) | InferredType::Set(elem)) => {
            Ty::Ground((**elem).clone())
        }
        Ty::Ground(InferredType::Dict(key, _)) => Ty::Ground((**key).clone()),
        Ty::Ground(InferredType::Str | InferredType::LiteralString) => {
            Ty::Ground(InferredType::Str)
        }
        _ => Ty::unknown(),
    }
}

/// Positional lambda parameter names, in declaration order.
pub(super) fn lambda_param_names(lambda: &ruff_python_ast::ExprLambda) -> Vec<String> {
    lambda.parameters.as_ref().map_or_else(Vec::new, |params| {
        params
            .posonlyargs
            .iter()
            .chain(&params.args)
            .map(|param| param.parameter.name.to_string())
            .collect()
    })
}

/// Widen a numeric operand to its ground arithmetic result type.
fn numeric_result(ty: &Ty) -> Ty {
    match ty {
        Ty::Ground(
            InferredType::Literal(LiteralValue::Int(_)) | InferredType::Int | InferredType::Bool,
        ) => Ty::Ground(InferredType::Int),
        Ty::Ground(InferredType::Float) => Ty::Ground(InferredType::Float),
        _ => Ty::unknown(),
    }
}

/// The conservative Stage 0 binary-operator result table.
fn bin_op_result(op: Operator, left: &Ty, right: &Ty) -> Ty {
    let (left, right) = (numeric_or_seq(left), numeric_or_seq(right));
    match (op, left, right) {
        (Operator::Div, OpClass::Int, OpClass::Int) => Ty::Ground(InferredType::Float),
        (_, OpClass::Int, OpClass::Int) => Ty::Ground(InferredType::Int),
        (_, OpClass::Int | OpClass::Float, OpClass::Int | OpClass::Float) => {
            Ty::Ground(InferredType::Float)
        }
        (Operator::Add, OpClass::Str, OpClass::Str) => Ty::Ground(InferredType::Str),
        _ => Ty::unknown(),
    }
}

/// Operand classification for [`bin_op_result`].
enum OpClass {
    Int,
    Float,
    Str,
    Other,
}

/// Classify an operand for the binary-operator table.
fn numeric_or_seq(ty: &Ty) -> OpClass {
    match ty {
        Ty::Ground(
            InferredType::Int | InferredType::Bool | InferredType::Literal(LiteralValue::Int(_)),
        ) => OpClass::Int,
        Ty::Ground(InferredType::Float) => OpClass::Float,
        Ty::Ground(
            InferredType::Str
            | InferredType::LiteralString
            | InferredType::Literal(LiteralValue::Str(_)),
        ) => OpClass::Str,
        _ => OpClass::Other,
    }
}
