//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Collect type narrowing guards from function bodies.
//!
//! Walks the AST of a function body and extracts narrowing facts for:
//! - `isinstance(x, T)` guards (§7.1)
//! - `x is None` / `x is not None` guards (§7.2)
//! - Truthiness guards `if x:` (§7.3)
//! - `assert isinstance(x, T)` / `assert x is not None` (§7.8)
//! - `match` statement case narrowing (§7.5)

use ruff_python_ast::{CmpOp, ExceptHandler, Expr, Stmt};
use ruff_text_size::Ranged;

use crate::scope::{
    narrowing_types::{MatchCaseNarrowing, NarrowingGuard, NarrowingGuardKind},
    Span,
};

use super::core::text_range_to_span;
use super::function_info::annotation_source_text;

/// Collect all narrowing guards from a function body.
pub(super) fn collect_narrowing_guards(stmts: &[Stmt]) -> Vec<NarrowingGuard> {
    let mut guards = Vec::new();
    collect_from_stmts(stmts, &mut guards, false);
    guards
}

fn collect_from_stmts(stmts: &[Stmt], guards: &mut Vec<NarrowingGuard>, in_loop: bool) {
    for stmt in stmts {
        collect_from_stmt(stmt, guards, in_loop);
    }
}

fn collect_from_stmt(stmt: &Stmt, guards: &mut Vec<NarrowingGuard>, in_loop: bool) {
    match stmt {
        Stmt::If(node) => {
            let if_body_span = body_span(&node.body);
            let else_body_span = node
                .elif_else_clauses
                .last()
                .map(|clause| body_span(&clause.body));

            if let Some(guard_kind) =
                extract_guard_from_test(&node.test, if_body_span, else_body_span)
            {
                guards.push(NarrowingGuard {
                    kind: guard_kind,
                    span: text_range_to_span(node.test.range()),
                    in_loop,
                });
            }

            // Recurse into branches
            collect_from_stmts(&node.body, guards, in_loop);
            for clause in &node.elif_else_clauses {
                // elif clauses may also contain narrowing guards
                if let Some(test) = &clause.test {
                    let elif_body_span = body_span(&clause.body);
                    if let Some(guard_kind) = extract_guard_from_test(test, elif_body_span, None) {
                        guards.push(NarrowingGuard {
                            kind: guard_kind,
                            span: text_range_to_span(test.range()),
                            in_loop,
                        });
                    }
                }
                collect_from_stmts(&clause.body, guards, in_loop);
            }
        }
        Stmt::Assert(node) => {
            if let Some(inner_kind) = extract_assert_guard(&node.test) {
                guards.push(NarrowingGuard {
                    kind: NarrowingGuardKind::Assert {
                        inner: Box::new(inner_kind),
                    },
                    span: text_range_to_span(node.range),
                    in_loop,
                });
            }
        }
        // Implements [TYPEINF-NARROWING-MATCH] — per-case pattern narrowing and
        // wildcard tracking for exhaustiveness.
        Stmt::Match(node) => {
            if let Some(variable) = expr_simple_name(&node.subject) {
                let mut cases = Vec::new();
                let mut has_wildcard = false;

                for case in &node.cases {
                    if is_wildcard_pattern(&case.pattern) {
                        has_wildcard = true;
                    }
                    if let Some(pattern_type) = extract_match_pattern_type(&case.pattern) {
                        cases.push(MatchCaseNarrowing {
                            pattern_type,
                            body_span: body_span(&case.body),
                        });
                    }
                    collect_from_stmts(&case.body, guards, in_loop);
                }

                if !cases.is_empty() {
                    guards.push(NarrowingGuard {
                        kind: NarrowingGuardKind::Match {
                            variable,
                            cases,
                            has_wildcard,
                        },
                        span: text_range_to_span(node.range),
                        in_loop,
                    });
                }
            }
        }
        // Loops: narrowing inside does NOT persist after the loop (§7.10)
        // Implements [TYPEINF-NARROWING-SCOPE] — the `in_loop` flag marks guards
        // collected inside a loop body so they do not leak past the loop.
        Stmt::For(node) => {
            collect_from_stmts(&node.body, guards, true);
            collect_from_stmts(&node.orelse, guards, in_loop);
        }
        Stmt::While(node) => {
            collect_from_stmts(&node.body, guards, true);
            collect_from_stmts(&node.orelse, guards, in_loop);
        }
        Stmt::With(node) => {
            collect_from_stmts(&node.body, guards, in_loop);
        }
        Stmt::Try(node) => {
            collect_from_stmts(&node.body, guards, in_loop);
            for handler in &node.handlers {
                let ExceptHandler::ExceptHandler(h) = handler;
                collect_from_stmts(&h.body, guards, in_loop);
            }
            collect_from_stmts(&node.orelse, guards, in_loop);
            collect_from_stmts(&node.finalbody, guards, in_loop);
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
    test: &Expr,
    if_body_span: Span,
    else_body_span: Option<Span>,
) -> Option<NarrowingGuardKind> {
    match test {
        // isinstance(x, T) or isinstance(x, (T1, T2))
        Expr::Call(call) => {
            let func_name = expr_simple_name(&call.func)?;
            if func_name == "isinstance" && call.arguments.args.len() == 2 {
                let variable = expr_simple_name(call.arguments.args.first()?)?;
                let type_names = extract_type_names(call.arguments.args.get(1)?);
                if !type_names.is_empty() {
                    return Some(NarrowingGuardKind::IsInstance {
                        variable,
                        type_names,
                        if_body_span,
                        else_body_span,
                    });
                }
            }
            None
        }
        // x is None / x is not None
        Expr::Compare(cmp) if cmp.comparators.len() == 1 => {
            let is_none_check = matches!(cmp.comparators.first(), Some(Expr::NoneLiteral(_)));
            let left_is_name = expr_simple_name(&cmp.left);

            // Also handle `None is x` (reversed)
            let (variable, is_none) = if is_none_check {
                (left_is_name?, true)
            } else if matches!(cmp.left.as_ref(), Expr::NoneLiteral(_)) {
                let right_name = cmp.comparators.first().and_then(expr_simple_name)?;
                (right_name, true)
            } else {
                return None;
            };

            if !is_none {
                return None;
            }

            let is_positive = match cmp.ops.first() {
                Some(CmpOp::Is) => true,
                Some(CmpOp::IsNot) => false,
                _ => return None,
            };

            Some(NarrowingGuardKind::IsNone {
                variable,
                is_positive,
                if_body_span,
                else_body_span,
            })
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

/// Extract a narrowing guard from an `assert` statement's test expression.
// Implements [TYPEINF-NARROWING-ASSERT] — `assert isinstance(x, T)` /
// `assert x is not None` narrows for all subsequent code in the flow path.
fn extract_assert_guard(test: &Expr) -> Option<NarrowingGuardKind> {
    match test {
        // assert isinstance(x, T)
        Expr::Call(call) => {
            let func_name = expr_simple_name(&call.func)?;
            if func_name == "isinstance" && call.arguments.args.len() == 2 {
                let variable = expr_simple_name(call.arguments.args.first()?)?;
                let type_names = extract_type_names(call.arguments.args.get(1)?);
                if !type_names.is_empty() {
                    // Dummy spans — assert narrows for ALL subsequent code, not a specific body
                    return Some(NarrowingGuardKind::IsInstance {
                        variable,
                        type_names,
                        if_body_span: Span::new(0, 0),
                        else_body_span: None,
                    });
                }
            }
            None
        }
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

/// Extract type names from an isinstance second argument.
///
/// Handles both `isinstance(x, int)` and `isinstance(x, (int, str))`.
fn extract_type_names(expr: &Expr) -> Vec<String> {
    match expr {
        Expr::Name(_) | Expr::Attribute(_) | Expr::Subscript(_) => {
            vec![annotation_source_text(expr)]
        }
        Expr::Tuple(tup) => tup.elts.iter().map(annotation_source_text).collect(),
        _ => Vec::new(),
    }
}

/// Extract the simple name from an expression, if it's a bare `Name` node.
fn expr_simple_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.to_string()),
        _ => None,
    }
}

/// Check if a match pattern is a wildcard (`_` or no binding).
fn is_wildcard_pattern(pattern: &ruff_python_ast::Pattern) -> bool {
    matches!(pattern, ruff_python_ast::Pattern::MatchAs(p) if p.name.is_none() && p.pattern.is_none())
}

/// Extract the type name from a match case pattern.
fn extract_match_pattern_type(pattern: &ruff_python_ast::Pattern) -> Option<String> {
    match pattern {
        ruff_python_ast::Pattern::MatchClass(cls) => Some(annotation_source_text(&cls.cls)),
        ruff_python_ast::Pattern::MatchValue(val) => Some(annotation_source_text(&val.value)),
        ruff_python_ast::Pattern::MatchAs(p) if p.name.is_none() && p.pattern.is_none() => {
            // Wildcard — no narrowing
            None
        }
        ruff_python_ast::Pattern::MatchAs(p) => {
            // `case x as name:` — extract inner pattern type
            p.pattern.as_deref().and_then(extract_match_pattern_type)
        }
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
