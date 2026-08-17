//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Collect type narrowing guards from function bodies.
//!
//! Walks the AST of a function body and extracts narrowing facts for:
//! - `isinstance(x, T)` guards (§7.1)
//! - `x is None` / `x is not None` guards (§7.2)
//! - Truthiness guards `if x:` (§7.3)
//! - `assert isinstance(x, T)` / `assert x is not None` (§7.8)
//! - `match` statement case narrowing (§7.5)

use basilisk_canonical::{BindingTable, TypingForm};
use ruff_python_ast::{CmpOp, ExceptHandler, Expr, Stmt};
use ruff_text_size::Ranged;

use crate::scope::{
    narrowing_types::{NarrowingGuard, NarrowingGuardKind},
    Span,
};

use super::core::text_range_to_span;

/// Collect all narrowing guards from a function body.
///
/// Guard callees (`isinstance`, `hasattr`, `type`, …) are recognised through
/// the module's [`BindingTable`] — aliased imports of a builtin narrow, a
/// module-level rebinding of its name does not. The table models module
/// scope, so a *function-local* rebinding is not yet visible to it.
pub(super) fn collect_narrowing_guards(
    bindings: &BindingTable,
    stmts: &[Stmt],
) -> Vec<NarrowingGuard> {
    let mut guards = Vec::new();
    collect_from_stmts(bindings, stmts, &mut guards, false);
    guards
}

fn collect_from_stmts(
    bindings: &BindingTable,
    stmts: &[Stmt],
    guards: &mut Vec<NarrowingGuard>,
    in_loop: bool,
) {
    for stmt in stmts {
        collect_from_stmt(bindings, stmt, guards, in_loop);
    }
}

fn collect_from_stmt(
    bindings: &BindingTable,
    stmt: &Stmt,
    guards: &mut Vec<NarrowingGuard>,
    in_loop: bool,
) {
    match stmt {
        Stmt::If(node) => {
            let if_body_span = body_span(&node.body);
            let else_body_span = node
                .elif_else_clauses
                .last()
                .map(|clause| body_span(&clause.body));

            if let Some(guard_kind) =
                extract_guard_from_test(bindings, &node.test, if_body_span, else_body_span)
            {
                guards.push(NarrowingGuard {
                    kind: guard_kind,
                    span: text_range_to_span(node.test.range()),
                    in_loop,
                });
            }

            // Recurse into branches
            collect_from_stmts(bindings, &node.body, guards, in_loop);
            for clause in &node.elif_else_clauses {
                // elif clauses may also contain narrowing guards
                if let Some(test) = &clause.test {
                    let elif_body_span = body_span(&clause.body);
                    if let Some(guard_kind) =
                        extract_guard_from_test(bindings, test, elif_body_span, None)
                    {
                        guards.push(NarrowingGuard {
                            kind: guard_kind,
                            span: text_range_to_span(test.range()),
                            in_loop,
                        });
                    }
                }
                collect_from_stmts(bindings, &clause.body, guards, in_loop);
            }
        }
        Stmt::Assert(node) => {
            if let Some(inner_kind) = extract_assert_guard(bindings, &node.test) {
                guards.push(NarrowingGuard {
                    kind: NarrowingGuardKind::Assert {
                        inner: Box::new(inner_kind),
                    },
                    span: text_range_to_span(node.range),
                    in_loop,
                });
            }
        }
        // [TYPEINF-NARROWING-MATCH]: per-case pattern narrowing classified
        // patterns by rendered class-name text and was deleted with that
        // recogniser. Only body recursion survives; no `Match` guard is
        // produced until patterns resolve through bindings
        // ([ASTREBUILD-PHASE-RESOLVER]).
        Stmt::Match(node) => {
            for case in &node.cases {
                collect_from_stmts(bindings, &case.body, guards, in_loop);
            }
        }
        // Loops: narrowing inside does NOT persist after the loop (§7.10)
        // Implements [TYPEINF-NARROWING-SCOPE] — the `in_loop` flag marks guards
        // collected inside a loop body so they do not leak past the loop.
        Stmt::For(node) => {
            collect_from_stmts(bindings, &node.body, guards, true);
            collect_from_stmts(bindings, &node.orelse, guards, in_loop);
        }
        Stmt::While(node) => {
            collect_from_stmts(bindings, &node.body, guards, true);
            collect_from_stmts(bindings, &node.orelse, guards, in_loop);
        }
        Stmt::With(node) => {
            collect_from_stmts(bindings, &node.body, guards, in_loop);
        }
        Stmt::Try(node) => {
            collect_from_stmts(bindings, &node.body, guards, in_loop);
            for handler in &node.handlers {
                let ExceptHandler::ExceptHandler(h) = handler;
                collect_from_stmts(bindings, &h.body, guards, in_loop);
            }
            collect_from_stmts(bindings, &node.orelse, guards, in_loop);
            collect_from_stmts(bindings, &node.finalbody, guards, in_loop);
        }
        // Do NOT recurse into nested function definitions — they have their own scope
        _ => {}
    }
}

/// Extract a narrowing guard from an `if` test expression.
// Implements [TYPEINF-NARROWING-ISINSTANCE] (isinstance branch),
// [TYPEINF-NARROWING-NONE] (`is None`/`is not None` branch), and
// [TYPEINF-NARROWING-TRUTHY] (`if x:` / `if not x:` branch).
fn extract_guard_from_test(
    bindings: &BindingTable,
    test: &Expr,
    if_body_span: Span,
    else_body_span: Option<Span>,
) -> Option<NarrowingGuardKind> {
    match test {
        // isinstance(x, T) / issubclass(x, T) / hasattr(x, "name")
        Expr::Call(call) => extract_call_guard(bindings, call, if_body_span, else_body_span),
        // x is None / x == lit / x in (lits) / "key" in td
        Expr::Compare(cmp) if cmp.comparators.len() == 1 => {
            extract_compare_guard(bindings, cmp, if_body_span, else_body_span)
        }
        // `not x` — inverted truthiness
        Expr::UnaryOp(unary) if matches!(unary.op, ruff_python_ast::UnaryOp::Not) => {
            // `if not x:` is like truthiness with inverted branches
            let variable = expr_simple_name(&unary.operand)?;
            // The if_body is the falsy branch, else_body is the truthy branch
            Some(NarrowingGuardKind::Truthiness {
                variable,
                if_body_span: else_body_span.unwrap_or(if_body_span),
                else_body_span: Some(if_body_span),
            })
        }
        // Simple name — truthiness narrowing: `if x:`
        Expr::Name(_) => {
            let variable = expr_simple_name(test)?;
            Some(NarrowingGuardKind::Truthiness {
                variable,
                if_body_span,
                else_body_span,
            })
        }
        _ => None,
    }
}

/// Extract a guard from a call test: `isinstance`, `issubclass`, `hasattr`.
///
/// The callee is recognised by what it RESOLVES to ([ASTREBUILD-LAW]), via
/// [`BindingTable::form_of_with_builtins`]: an aliased import of the builtin
/// still guards, a module-level rebinding of its name never does.
#[expect(
    clippy::match_same_arms,
    reason = "the deleted-collector arms return `None` like the wildcard, but they are TRACKED ABSTENTIONS naming the form whose reader was deleted ([ASTREBUILD-PHASE-RESOLVER]); collapsing them into the wildcard would erase the map of what must be rebuilt"
)]
fn extract_call_guard(
    bindings: &BindingTable,
    call: &ruff_python_ast::ExprCall,
    if_body_span: Span,
    else_body_span: Option<Span>,
) -> Option<NarrowingGuardKind> {
    if call.arguments.args.len() != 2 {
        return None;
    }
    let variable = expr_simple_name(call.arguments.args.first()?)?;
    let second = call.arguments.args.get(1)?;
    match bindings.form_of_with_builtins(&call.func)? {
        // [TYPEINF-NARROWING-ISINSTANCE] / [TYPEINF-NARROWING-ISSUBCLASS]
        // (typing spec narrowing §`isinstance`): the second-argument reader
        // rendered class references to name text and was deleted with it. No
        // guard is produced until the argument resolves through bindings
        // ([ASTREBUILD-PHASE-RESOLVER]).
        TypingForm::IsinstanceFunction | TypingForm::IssubclassFunction => None,
        // Implements [TYPEINF-NARROWING-HASATTR]
        // (<https://typing.python.org/en/latest/spec/narrowing.html>).
        TypingForm::HasattrFunction => match second {
            Expr::StringLiteral(lit) => Some(NarrowingGuardKind::HasAttr {
                variable,
                attribute: lit.value.to_str().to_owned(),
                if_body_span,
                else_body_span,
            }),
            _ => None,
        },
        _ => None,
    }
}

/// Extract a guard from a single comparison: `is None`, `== <literal>`,
/// `in (<literals>)`, or `"key" in td`.
#[expect(
    clippy::match_same_arms,
    reason = "the deleted-collector arms return `None` like the wildcard, but they are TRACKED ABSTENTIONS naming the form whose reader was deleted ([ASTREBUILD-PHASE-RESOLVER]); collapsing them into the wildcard would erase the map of what must be rebuilt"
)]
fn extract_compare_guard(
    bindings: &BindingTable,
    cmp: &ruff_python_ast::ExprCompare,
    if_body_span: Span,
    else_body_span: Option<Span>,
) -> Option<NarrowingGuardKind> {
    let op = *cmp.ops.first()?;
    let right = cmp.comparators.first()?;
    match op {
        CmpOp::Is | CmpOp::IsNot => {
            // Implements [TYPEINF-NARROWING-TYPEOF]: `type(x) is C`.
            if let Some(guard) =
                extract_type_of_guard(bindings, cmp, op, right, if_body_span, else_body_span)
            {
                return Some(guard);
            }
            extract_none_guard(cmp, op, right, if_body_span, else_body_span)
        }
        // [TYPEINF-NARROWING-EQ-LITERAL]: the comparator was captured as
        // rendered literal text and that renderer was deleted. No guard is
        // produced until literals carry a semantic value representation
        // ([ASTREBUILD-PHASE-RESOLVER]).
        CmpOp::Eq | CmpOp::NotEq => None,
        CmpOp::In | CmpOp::NotIn => {
            extract_membership_guard(cmp, op, right, if_body_span, else_body_span)
        }
        _ => None,
    }
}

/// `type(x) is C` / `type(x) is not C` — exact-class comparison
/// (typing spec narrowing, `type()` guards). The callee must RESOLVE to the
/// builtin `type` class ([ASTREBUILD-LAW]); `typing.Type` is a special form,
/// not callable, so only [`TypingForm::TypeClass`] qualifies.
fn extract_type_of_guard(
    bindings: &BindingTable,
    cmp: &ruff_python_ast::ExprCompare,
    op: CmpOp,
    right: &Expr,
    if_body_span: Span,
    else_body_span: Option<Span>,
) -> Option<NarrowingGuardKind> {
    let Expr::Call(call) = cmp.left.as_ref() else {
        return None;
    };
    if bindings.form_of_with_builtins(&call.func)? != TypingForm::TypeClass
        || call.arguments.args.len() != 1
    {
        return None;
    }
    let variable = expr_simple_name(call.arguments.args.first()?)?;
    let type_name = expr_simple_name(right)?;
    Some(NarrowingGuardKind::TypeOfIs {
        variable,
        type_name,
        type_span: text_range_to_span(right.range()),
        // Positional: which class the right operand names is decided by the
        // binding in force at the comparison ([RESOLV-CANONICAL-BINDING]).
        type_class_site: bindings
            .local_class_definition(right)
            .map(text_range_to_span),
        is_positive: matches!(op, CmpOp::Is),
        if_body_span,
        else_body_span,
    })
}

/// The original `is None` / `is not None` extraction (both operand orders).
fn extract_none_guard(
    cmp: &ruff_python_ast::ExprCompare,
    op: CmpOp,
    right: &Expr,
    if_body_span: Span,
    else_body_span: Option<Span>,
) -> Option<NarrowingGuardKind> {
    let variable = if matches!(right, Expr::NoneLiteral(_)) {
        expr_simple_name(&cmp.left)?
    } else if matches!(cmp.left.as_ref(), Expr::NoneLiteral(_)) {
        expr_simple_name(right)?
    } else {
        return None;
    };
    Some(NarrowingGuardKind::IsNone {
        variable,
        is_positive: matches!(op, CmpOp::Is),
        if_body_span,
        else_body_span,
    })
}

/// `x in (lits)` → [`NarrowingGuardKind::InLiterals`];
/// `"key" in td` → [`NarrowingGuardKind::KeyInDict`].
fn extract_membership_guard(
    cmp: &ruff_python_ast::ExprCompare,
    op: CmpOp,
    right: &Expr,
    if_body_span: Span,
    else_body_span: Option<Span>,
) -> Option<NarrowingGuardKind> {
    let is_positive = matches!(op, CmpOp::In);
    // Implements [TYPEINF-NARROWING-TYPEDDICT-KEY]: `"key" in td`.
    if let (Expr::StringLiteral(key), Some(variable)) = (cmp.left.as_ref(), expr_simple_name(right))
    {
        return Some(NarrowingGuardKind::KeyInDict {
            variable,
            key: key.value.to_str().to_owned(),
            is_positive,
            if_body_span,
            else_body_span,
        });
    }
    // [TYPEINF-NARROWING-IN-LITERAL]: membership elements were captured as
    // rendered literal text and that renderer was deleted. No guard is
    // produced until literals carry a semantic value representation
    // ([ASTREBUILD-PHASE-RESOLVER]).
    None
}

/// Extract a narrowing guard from an `assert` statement's test expression.
// Implements [TYPEINF-NARROWING-ASSERT] — `assert isinstance(x, T)` /
// `assert x is not None` narrows for all subsequent code in the flow path.
fn extract_assert_guard(bindings: &BindingTable, test: &Expr) -> Option<NarrowingGuardKind> {
    match test {
        // `assert hasattr(x, "a")` narrows subsequent flow (§7.8); the
        // callee resolves through bindings like every call guard.
        // `assert isinstance(x, T)` still yields nothing there — its
        // second-argument reader was deleted ([ASTREBUILD-PHASE-RESOLVER]).
        Expr::Call(call) => extract_call_guard(bindings, call, Span::new(0, 0), None),
        // assert x is not None
        Expr::Compare(cmp) if cmp.comparators.len() == 1 => {
            let is_none_target = matches!(cmp.comparators.first(), Some(Expr::NoneLiteral(_)));
            if !is_none_target {
                return None;
            }
            let variable = expr_simple_name(&cmp.left)?;
            let is_positive = match cmp.ops.first() {
                Some(CmpOp::Is) => true,
                Some(CmpOp::IsNot) => false,
                _ => return None,
            };
            Some(NarrowingGuardKind::IsNone {
                variable,
                is_positive,
                if_body_span: Span::new(0, 0),
                else_body_span: None,
            })
        }
        // assert x (truthiness)
        Expr::Name(_) => {
            let variable = expr_simple_name(test)?;
            Some(NarrowingGuardKind::Truthiness {
                variable,
                if_body_span: Span::new(0, 0),
                else_body_span: None,
            })
        }
        _ => None,
    }
}

/// Extract the simple name from an expression, if it's a bare `Name` node.
fn expr_simple_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.to_string()),
        _ => None,
    }
}

/// Compute the span covering all statements in a body.
fn body_span(stmts: &[Stmt]) -> Span {
    if stmts.is_empty() {
        return Span::new(0, 0);
    }
    let start = stmts.first().map_or(0, |s| s.range().start().to_u32());
    let end = stmts.last().map_or(0, |s| s.range().end().to_u32());
    Span::new(start, end)
}
