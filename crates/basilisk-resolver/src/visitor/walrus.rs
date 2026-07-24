//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Bindings introduced by [PEP 572](https://peps.python.org/pep-0572/)
//! assignment expressions (`name := value`).
//!
//! Every other binding form is a statement, so the collectors in
//! [`super::assigns`] match it syntactically. A walrus hides inside an
//! *expression* — `if item := prices.get(asset):` binds `item` from the `if`
//! test — which made its target invisible to those collectors and so undefined
//! to `names_undefined` (GitHub #339).

use ruff_python_ast::visitor::{walk_elif_else_clause, walk_expr, walk_stmt, Visitor};
use ruff_python_ast::{Comprehension, ElifElseClause, Expr, Stmt};

use super::class_info_ext::expr_simple_name;

/// How much of the visited code the caller may treat as evaluated.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Reach {
    /// Everything, however deeply nested or conditional: the target is bound
    /// *somewhere* in the body.
    Any,
    /// Only what Python must evaluate once control reaches each statement in
    /// `stmts`, so the target is bound on every path past it. Branch bodies and
    /// short-circuiting operands are pruned — `x` in `if a and (x := f()):` may
    /// never be bound.
    Definite,
}

/// Collect the targets of the assignment expressions `stmts` binds into the
/// enclosing scope, at the given [`Reach`].
///
/// Nested `def`/`class`/`lambda` scopes are excluded: a walrus there binds in
/// *that* scope. Comprehensions are not — PEP 572 deliberately exempts the
/// walrus from the comprehension's own scope so it binds in the enclosing one.
pub(super) fn collect_walrus_targets(stmts: &[Stmt], reach: Reach) -> Vec<String> {
    let mut collector = WalrusTargets {
        reach,
        out: Vec::new(),
    };
    for stmt in stmts {
        collector.visit_stmt(stmt);
    }
    collector.out
}

struct WalrusTargets {
    reach: Reach,
    out: Vec<String>,
}

impl WalrusTargets {
    fn descends_into_conditional_code(&self) -> bool {
        self.reach == Reach::Any
    }

    /// Whether reaching this expression's operands is conditional.
    fn is_short_circuiting(expr: &Expr) -> bool {
        matches!(
            expr,
            Expr::BoolOp(_)
                | Expr::If(_)
                | Expr::ListComp(_)
                | Expr::SetComp(_)
                | Expr::DictComp(_)
                | Expr::Generator(_)
        )
    }

    /// Visit only the operands of a short-circuiting form that always run: the
    /// first operand of `and`/`or`, the test of a ternary, and a comprehension's
    /// outermost iterable (evaluated eagerly, even for a generator expression).
    fn visit_definite_operands(&mut self, expr: &Expr) {
        match expr {
            Expr::BoolOp(op) => self.visit_first(op.values.first()),
            Expr::If(ternary) => self.visit_expr(&ternary.test),
            Expr::ListComp(comp) => self.visit_outermost_iter(&comp.generators),
            Expr::SetComp(comp) => self.visit_outermost_iter(&comp.generators),
            Expr::DictComp(comp) => self.visit_outermost_iter(&comp.generators),
            Expr::Generator(comp) => self.visit_outermost_iter(&comp.generators),
            _ => {}
        }
    }

    fn visit_outermost_iter(&mut self, generators: &[Comprehension]) {
        self.visit_first(generators.first().map(|generator| &generator.iter));
    }

    fn visit_first(&mut self, expr: Option<&Expr>) {
        if let Some(expr) = expr {
            self.visit_expr(expr);
        }
    }
}

impl Visitor<'_> for WalrusTargets {
    fn visit_stmt(&mut self, stmt: &Stmt) {
        // A nested `def`/`class` is its own scope; only the name it binds
        // belongs here, and [`super::assigns`] already records that.
        if matches!(stmt, Stmt::FunctionDef(_) | Stmt::ClassDef(_)) {
            return;
        }
        walk_stmt(self, stmt);
    }

    fn visit_body(&mut self, body: &[Stmt]) {
        // Suppressing nested bodies leaves `walk_stmt` visiting exactly the
        // expressions a statement evaluates itself — an `if`/`while` test, a
        // `for` iterable, an assigned value — which is what `Definite` means.
        if self.descends_into_conditional_code() {
            ruff_python_ast::visitor::walk_body(self, body);
        }
    }

    fn visit_elif_else_clause(&mut self, clause: &ElifElseClause) {
        // Only the leading `if` test is guaranteed to be evaluated; an `elif`
        // test runs only when every test before it was falsy.
        if self.descends_into_conditional_code() {
            walk_elif_else_clause(self, clause);
        }
    }

    fn visit_expr(&mut self, expr: &Expr) {
        // A `lambda` body is its own scope, exactly like a nested `def`.
        if matches!(expr, Expr::Lambda(_)) {
            return;
        }
        if let Expr::Named(named) = expr {
            self.out.extend(expr_simple_name(&named.target));
        }
        if !self.descends_into_conditional_code() && Self::is_short_circuiting(expr) {
            self.visit_definite_operands(expr);
            return;
        }
        walk_expr(self, expr);
    }
}
