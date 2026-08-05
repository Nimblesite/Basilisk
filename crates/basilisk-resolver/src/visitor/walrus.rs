//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Bindings introduced by [PEP 572](https://peps.python.org/pep-0572/)
//! assignment expressions (`name := value`).
//!
//! Every other binding form is a statement, so the collectors in
//! [`super::assigns`] match it syntactically. A walrus hides inside an
//! *expression* — `if item := prices.get(asset):` binds `item` from the `if`
//! test — which made its target invisible to those collectors and so undefined
//! to `names_undefined` (GitHub #339).
//!
//! Only the anywhere-in-the-body reach survives here: the definite
//! ("bound on every path past this statement") variant moved into the
//! checker's `names_unbound` walk with the rest of the definite-assignment
//! analysis ([NARROWPLAN-INTEGRATION] Step 8).

use ruff_python_ast::visitor::{walk_expr, walk_stmt, Visitor};
use ruff_python_ast::{Expr, Stmt};

use super::class_info_ext::expr_simple_name;

/// Collect the targets of every assignment expression `stmts` binds into the
/// enclosing scope, however deeply nested or conditional.
///
/// Nested `def`/`class`/`lambda` scopes are excluded: a walrus there binds in
/// *that* scope. Comprehensions are not — PEP 572 deliberately exempts the
/// walrus from the comprehension's own scope so it binds in the enclosing one.
pub(super) fn collect_walrus_targets(stmts: &[Stmt]) -> Vec<String> {
    let mut collector = WalrusTargets { out: Vec::new() };
    for stmt in stmts {
        collector.visit_stmt(stmt);
    }
    collector.out
}

struct WalrusTargets {
    out: Vec<String>,
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

    fn visit_expr(&mut self, expr: &Expr) {
        // A `lambda` body is its own scope, exactly like a nested `def`.
        if matches!(expr, Expr::Lambda(_)) {
            return;
        }
        if let Expr::Named(named) = expr {
            self.out.extend(expr_simple_name(&named.target));
        }
        walk_expr(self, expr);
    }
}
