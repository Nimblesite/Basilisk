//! Implements call-site collection for [`directives_cast`]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
//!
//! `typing.cast(typ, val)` is invalid wherever it appears — its arity and its
//! first argument do not become legal because the call sits in a `return`
//! instead of an assignment. The module-wide [`ResolvedModule::calls`] vector
//! deliberately records only the outermost call of a few statement kinds, which
//! left `return cast(1, x)` and `print(cast(1, x))` unchecked (issue #335).
//!
//! This collector walks **every** expression position in the module and records
//! each `cast(...)` it finds. It is scoped to `cast` on purpose: widening the
//! shared `calls` vector would change what every other call-site rule sees.
//!
//! [`ResolvedModule::calls`]: crate::scope::ResolvedModule::calls

use ruff_python_ast::visitor::{walk_expr, Visitor};
use ruff_python_ast::{Expr, Stmt};

use crate::scope::CallSite;

use super::calls_and_reveal::call_site_from_expr;

/// The name of the callee this collector records. Both the bare `cast(...)`
/// import spelling and the qualified `typing.cast(...)` spelling resolve to
/// this simple name in [`CallSite::callee`].
const CAST: &str = "cast";

/// Collect every `cast(...)` call site in `stmts`, in any expression position.
///
/// `source` is used only as a fast-path guard: a call to `cast` cannot exist in
/// a module whose text never contains that identifier, so the great majority of
/// modules skip the walk entirely and this collector costs one substring scan.
pub(super) fn collect_cast_calls(stmts: &[Stmt], source: &str) -> Vec<CallSite> {
    if !source.contains(CAST) {
        return Vec::new();
    }
    let mut collector = CastCallCollector { out: Vec::new() };
    for stmt in stmts {
        collector.visit_stmt(stmt);
    }
    collector.out
}

struct CastCallCollector {
    out: Vec<CallSite>,
}

impl<'a> Visitor<'a> for CastCallCollector {
    fn visit_expr(&mut self, expr: &'a Expr) {
        if matches!(expr, Expr::Call(call) if is_cast_callee(&call.func)) {
            self.out.extend(call_site_from_expr(expr));
        }
        walk_expr(self, expr);
    }
}

/// Returns `true` for the callee of `cast(...)` or `<module>.cast(...)`.
fn is_cast_callee(func: &Expr) -> bool {
    match func {
        Expr::Name(name) => name.id.as_str() == CAST,
        Expr::Attribute(attr) => attr.attr.as_str() == CAST,
        _ => false,
    }
}
