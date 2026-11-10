//! Implements the [TYPEINF-COLLECTIONS-TUPLES] index-range rule for annotated
//! variables. See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-COLLECTIONS-TUPLES
//!
//! A fixed-length `tuple[T1, ..., Tn]` supports exactly the literal indices
//! `[-n, n)`; anything else is a static error at every scope (the typing
//! spec's tuples chapter). This collector walks the AST for `name[LITERAL]`
//! subscripts whose name's **declared annotation** — an annotated local of the
//! innermost binding scope, else an annotated module variable — is a fixed
//! tuple, and records out-of-range reads as [`TupleIndexViolation`]s for the
//! `tuples_index` rule. Parameters stay with `tuples_index_2`; `key=` lambda
//! parameters stay with the `key_lambda` collector (their names are shadowed
//! here, never resolved against enclosing bindings).

use ruff_python_ast::visitor::{walk_expr, Visitor};
use ruff_python_ast::{Comprehension, Expr, ExprContext, ExprSubscript, Stmt};
use ruff_text_size::Ranged;

use crate::scope::{FunctionInfo, TupleIndexViolation, VariableInfo};

use super::core::text_range_to_span;
use super::key_lambda::{fixed_tuple_len, literal_int};

/// Collect out-of-range literal-index reads on tuple-annotated variables.
pub(super) fn collect_annotated_tuple_index_violations(
    stmts: &[Stmt],
    functions: &[FunctionInfo],
    module_vars: &[VariableInfo],
    source: &str,
) -> Vec<TupleIndexViolation> {
    let mut collector = AnnotatedTupleIndexCollector {
        functions,
        module_vars,
        source,
        shadowed: Vec::new(),
        out: Vec::new(),
    };
    for stmt in stmts {
        collector.visit_stmt(stmt);
    }
    collector.out
}

struct AnnotatedTupleIndexCollector<'a> {
    functions: &'a [FunctionInfo],
    module_vars: &'a [VariableInfo],
    source: &'a str,
    /// Names bound by enclosing lambda parameters or comprehension targets —
    /// scopes the resolver does not model as functions. A shadowed name never
    /// resolves to an outer annotation.
    shadowed: Vec<String>,
    out: Vec<TupleIndexViolation>,
}

impl<'a> Visitor<'a> for AnnotatedTupleIndexCollector<'a> {
    fn visit_expr(&mut self, expr: &'a Expr) {
        match expr {
            Expr::Lambda(lambda) => {
                let shadow_count = self.push_lambda_params(lambda);
                self.visit_expr(&lambda.body);
                self.shadowed.truncate(self.shadowed.len() - shadow_count);
            }
            Expr::ListComp(comp) => self.visit_comprehension_scope(&comp.generators, expr),
            Expr::SetComp(comp) => self.visit_comprehension_scope(&comp.generators, expr),
            Expr::DictComp(comp) => self.visit_comprehension_scope(&comp.generators, expr),
            Expr::Generator(comp) => self.visit_comprehension_scope(&comp.generators, expr),
            Expr::Subscript(sub) => {
                self.check_subscript(sub);
                walk_expr(self, expr);
            }
            _ => walk_expr(self, expr),
        }
    }
}

impl<'a> AnnotatedTupleIndexCollector<'a> {
    /// Shadow every parameter name of a lambda; returns how many were pushed.
    fn push_lambda_params(&mut self, lambda: &ruff_python_ast::ExprLambda) -> usize {
        let Some(params) = lambda.parameters.as_deref() else {
            return 0;
        };
        let before = self.shadowed.len();
        let positional = params.posonlyargs.iter().chain(&params.args);
        let keyword_only = params.kwonlyargs.iter();
        for param in positional.chain(keyword_only) {
            self.shadowed.push(param.parameter.name.to_string());
        }
        for param in params.vararg.iter().chain(params.kwarg.iter()) {
            self.shadowed.push(param.name.to_string());
        }
        self.shadowed.len() - before
    }

    /// Walk a comprehension with its target names shadowed (conservatively for
    /// the whole expression: no annotation-based diagnostics inside).
    fn visit_comprehension_scope(&mut self, generators: &'a [Comprehension], expr: &'a Expr) {
        let before = self.shadowed.len();
        for generator in generators {
            collect_target_names(&generator.target, &mut self.shadowed);
        }
        walk_expr(self, expr);
        self.shadowed.truncate(before);
    }

    /// Record `name[LITERAL]` reads whose declared fixed-tuple length excludes
    /// the index.
    fn check_subscript(&mut self, sub: &ExprSubscript) {
        if !matches!(sub.ctx, ExprContext::Load) {
            return;
        }
        let Expr::Name(base) = sub.value.as_ref() else {
            return;
        };
        let name = base.id.as_str();
        if self.shadowed.iter().any(|shadow| shadow == name) {
            return;
        }
        let Some(index) = literal_int(&sub.slice) else {
            return;
        };
        let offset = text_range_to_span(sub.range()).start_usize();
        let Some(tuple_length) = self.declared_tuple_len(name, offset) else {
            return;
        };
        let len = i64::try_from(tuple_length).unwrap_or(i64::MAX);
        if index >= len || index < -len {
            self.out.push(TupleIndexViolation {
                span: text_range_to_span(sub.range()),
                tuple_var_name: name.to_owned(),
                index_value: index,
                tuple_length,
            });
        }
    }

    /// The declared fixed-tuple length of `name` at `offset`: the innermost
    /// enclosing function whose scope binds the name decides — an annotated
    /// local's annotation applies; a parameter or unannotated binding opts out
    /// (different owner or no declared type) — falling back to an annotated
    /// module variable.
    fn declared_tuple_len(&self, name: &str, offset: usize) -> Option<usize> {
        let mut enclosing: Vec<&FunctionInfo> = self
            .functions
            .iter()
            .filter(|f| f.def_span.start_usize() <= offset && offset < f.def_span.end_usize())
            .collect();
        enclosing.sort_by_key(|f| std::cmp::Reverse(f.def_span.start));

        for func in enclosing {
            if let Some(var) = func.local_vars.iter().find(|v| v.name == name) {
                return self.annotated_fixed_tuple_len(var);
            }
            let binds_otherwise = func.parameters.iter().any(|p| p.name == name)
                || func.vararg.as_ref().is_some_and(|v| v.name == name)
                || func.kwarg.as_ref().is_some_and(|k| k.name == name)
                || func.all_local_assigns.iter().any(|a| a == name);
            if binds_otherwise {
                return None;
            }
        }
        let var = self.module_vars.iter().find(|v| v.name == name)?;
        self.annotated_fixed_tuple_len(var)
    }

    /// The variable's declared tuple length, `None` without a fixed-tuple
    /// annotation.
    fn annotated_fixed_tuple_len(&self, var: &VariableInfo) -> Option<usize> {
        let annotation = var.annotation_span?.slice_source(self.source)?;
        fixed_tuple_len(annotation.trim())
    }
}

/// Push every `Name` bound by a comprehension target (`x`, `(a, b)`, `[a, b]`,
/// starred elements) onto `shadowed`.
fn collect_target_names(target: &Expr, shadowed: &mut Vec<String>) {
    match target {
        Expr::Name(name) => shadowed.push(name.id.to_string()),
        Expr::Tuple(tuple) => {
            for element in &tuple.elts {
                collect_target_names(element, shadowed);
            }
        }
        Expr::List(list) => {
            for element in &list.elts {
                collect_target_names(element, shadowed);
            }
        }
        Expr::Starred(starred) => collect_target_names(&starred.value, shadowed),
        _ => {}
    }
}
