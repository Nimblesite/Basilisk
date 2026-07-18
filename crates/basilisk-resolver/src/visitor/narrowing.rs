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
        // isinstance(x, T) / issubclass(x, T) / hasattr(x, "name")
        Expr::Call(call) => extract_call_guard(call, if_body_span, else_body_span),
        // x is None / x == lit / x in (lits) / "key" in td
        Expr::Compare(cmp) if cmp.comparators.len() == 1 => {
            extract_compare_guard(cmp, if_body_span, else_body_span)
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
fn extract_call_guard(
    call: &ruff_python_ast::ExprCall,
    if_body_span: Span,
    else_body_span: Option<Span>,
) -> Option<NarrowingGuardKind> {
    let func_name = expr_simple_name(&call.func)?;
    if call.arguments.args.len() != 2 {
        return None;
    }
    let variable = expr_simple_name(call.arguments.args.first()?)?;
    let second = call.arguments.args.get(1)?;
    match func_name.as_str() {
        "isinstance" => {
            let type_names = extract_type_names(second);
            (!type_names.is_empty()).then_some(NarrowingGuardKind::IsInstance {
                variable,
                type_names,
                if_body_span,
                else_body_span,
            })
        }
        // Implements [TYPEINF-NARROWING-ISSUBCLASS].
        "issubclass" => {
            let type_names = extract_type_names(second);
            (!type_names.is_empty()).then_some(NarrowingGuardKind::IsSubclass {
                variable,
                type_names,
                if_body_span,
                else_body_span,
            })
        }
        // Implements [TYPEINF-NARROWING-HASATTR] (groundwork).
        "hasattr" => match second {
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
fn extract_compare_guard(
    cmp: &ruff_python_ast::ExprCompare,
    if_body_span: Span,
    else_body_span: Option<Span>,
) -> Option<NarrowingGuardKind> {
    let op = *cmp.ops.first()?;
    let right = cmp.comparators.first()?;
    match op {
        CmpOp::Is | CmpOp::IsNot => {
            extract_none_guard(cmp, op, right, if_body_span, else_body_span)
        }
        // Implements [TYPEINF-NARROWING-EQ-LITERAL].
        CmpOp::Eq | CmpOp::NotEq => {
            let variable = expr_simple_name(&cmp.left)?;
            let literal_text = literal_source_text(right)?;
            Some(NarrowingGuardKind::EqualsLiteral {
                variable,
                literal_text,
                is_positive: matches!(op, CmpOp::Eq),
                if_body_span,
                else_body_span,
            })
        }
        CmpOp::In | CmpOp::NotIn => {
            extract_membership_guard(cmp, op, right, if_body_span, else_body_span)
        }
        _ => None,
    }
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
    // Implements [TYPEINF-NARROWING-IN-LITERAL]: `x in ("a", "b")`.
    let variable = expr_simple_name(&cmp.left)?;
    let elements: &[Expr] = match right {
        Expr::Tuple(tuple) => &tuple.elts,
        Expr::List(list) => &list.elts,
        Expr::Set(set) => &set.elts,
        _ => return None,
    };
    let literal_texts: Vec<String> = elements.iter().filter_map(literal_source_text).collect();
    if literal_texts.len() != elements.len() || literal_texts.is_empty() {
        return None;
    }
    Some(NarrowingGuardKind::InLiterals {
        variable,
        literal_texts,
        is_positive,
        if_body_span,
        else_body_span,
    })
}

/// Case-preserving literal text for equality/membership guards: quoted
/// strings, decimal ints (including negated), and booleans. Anything else —
/// including strings containing a double quote — is skipped rather than
/// approximated.
fn literal_source_text(expr: &Expr) -> Option<String> {
    match expr {
        Expr::StringLiteral(lit) => {
            let value = lit.value.to_str();
            (!value.contains('"')).then(|| format!("\"{value}\""))
        }
        Expr::NumberLiteral(num) => match &num.value {
            ruff_python_ast::Number::Int(int) => int.as_i64().map(|n| n.to_string()),
            _ => None,
        },
        Expr::BooleanLiteral(lit) => Some(if lit.value { "True" } else { "False" }.to_owned()),
        Expr::UnaryOp(unary) if matches!(unary.op, ruff_python_ast::UnaryOp::USub) => {
            match unary.operand.as_ref() {
                Expr::NumberLiteral(num) => match &num.value {
                    ruff_python_ast::Number::Int(int) => int.as_i64().map(|n| (-n).to_string()),
                    _ => None,
                },
                _ => None,
            }
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
