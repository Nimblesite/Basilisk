//! Implements [TYPEINF-TARGET-NARROWING]. See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-NARROWING
//! **Inference-driven reachability** ([NARROWPLAN-FLOW]): divergence is
//! decided by what the type lattice proves — a call statement whose
//! synthesized type is `Never` (a `NoReturn` function) diverges, a
//! `while` loop whose test the engine proves always-truthy and whose body
//! never `break`s diverges — composed with the control-flow statements whose
//! divergence is definitional (`return`/`raise`/`continue`/`break`) and
//! recursion through compound statements. This replaces the previous
//! pattern-matched last-statement idiom (ty's model, per
//! [NARROWPLAN-CHECKLIST] Stage 2).
//!
//! Gradual posture ([TYPEINF-TARGET-GRADUAL]): everything the engine cannot
//! PROVE is treated as reachable — an `Unknown` call type never fabricates
//! divergence.

use ruff_python_ast::{ExceptHandler, Expr, Pattern, Stmt};

use crate::types::InferredType;

use super::guards::literal_is_truthy;

/// The expression-typing oracle divergence consults — the flow walker passes
/// its environment-seeded bidirectional synthesis.
pub(crate) type SynthFn<'a> = dyn FnMut(&Expr) -> InferredType + 'a;

/// Whether a statement list definitely diverges (control never reaches the
/// statement after it).
///
/// Private to the recursion: the flow walker asks about a body through its own
/// memoized [`crate::narrow::flow`] entry point instead, so a probe and the
/// walk that follows it cannot re-synthesize the same expressions
/// ([NARROWPLAN-INTEGRATION]).
fn stmts_diverge(stmts: &[Stmt], synth: &mut SynthFn<'_>) -> bool {
    stmts.last().is_some_and(|last| stmt_diverges(last, synth))
}

/// Whether one statement definitely diverges.
pub(crate) fn stmt_diverges(stmt: &Stmt, synth: &mut SynthFn<'_>) -> bool {
    match stmt {
        Stmt::Return(_) | Stmt::Raise(_) | Stmt::Continue(_) | Stmt::Break(_) => true,
        // The inference-driven case: a call statement typed `Never`
        // (`NoReturn`) never returns control.
        Stmt::Expr(node) => synth(&node.value) == InferredType::Never,
        Stmt::If(node) => if_diverges(node, synth),
        Stmt::Match(node) => match_diverges(node, synth),
        Stmt::With(node) => stmts_diverge(&node.body, synth),
        Stmt::Try(node) => try_diverges(node, synth),
        Stmt::While(node) => while_diverges(node, synth),
        _ => false,
    }
}

/// An `if` diverges when a plain `else` exists and every branch diverges.
fn if_diverges(node: &ruff_python_ast::StmtIf, synth: &mut SynthFn<'_>) -> bool {
    let has_else = node.elif_else_clauses.iter().any(|c| c.test.is_none());
    has_else
        && stmts_diverge(&node.body, synth)
        && node
            .elif_else_clauses
            .iter()
            .all(|clause| stmts_diverge(&clause.body, synth))
}

/// A `match` diverges when an unguarded wildcard case exists and every case
/// body diverges (without a wildcard, no case may match and control falls
/// through).
fn match_diverges(node: &ruff_python_ast::StmtMatch, synth: &mut SynthFn<'_>) -> bool {
    let has_wildcard = node
        .cases
        .iter()
        .any(|case| is_wildcard(&case.pattern) && case.guard.is_none());
    has_wildcard
        && node
            .cases
            .iter()
            .all(|case| stmts_diverge(&case.body, synth))
}

/// The catch-all `case _:` / `case name:` pattern.
fn is_wildcard(pattern: &Pattern) -> bool {
    matches!(pattern, Pattern::MatchAs(p) if p.pattern.is_none())
}

/// A `try` diverges when its `finally` diverges (it always runs), or when
/// the body and every handler diverge (normal completion exits through the
/// body, a handled exception through its diverging handler, an unhandled one
/// propagates).
fn try_diverges(node: &ruff_python_ast::StmtTry, synth: &mut SynthFn<'_>) -> bool {
    stmts_diverge(&node.finalbody, synth)
        || (stmts_diverge(&node.body, synth)
            && node.handlers.iter().all(|handler| {
                let ExceptHandler::ExceptHandler(h) = handler;
                stmts_diverge(&h.body, synth)
            }))
}

/// A `while` diverges when its test is PROVEN always-truthy (a truthy
/// literal, e.g. `while True:`) and its body contains no `break` at this
/// loop's level.
fn while_diverges(node: &ruff_python_ast::StmtWhile, synth: &mut SynthFn<'_>) -> bool {
    always_truthy(&synth(&node.test)) && !contains_break(&node.body)
}

/// Only literal types with statically-known truthiness qualify — everything
/// else stays "possibly falsy" (gradual, never a guess).
fn always_truthy(ty: &InferredType) -> bool {
    matches!(ty, InferredType::Literal(literal) if literal_is_truthy(literal))
}

/// Whether a `break` occurs at this loop's level (nested loops start their
/// own break scope; nested functions are boundaries).
fn contains_break(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|stmt| match stmt {
        Stmt::Break(_) => true,
        Stmt::If(node) => {
            contains_break(&node.body)
                || node
                    .elif_else_clauses
                    .iter()
                    .any(|clause| contains_break(&clause.body))
        }
        Stmt::With(node) => contains_break(&node.body),
        Stmt::Match(node) => node.cases.iter().any(|case| contains_break(&case.body)),
        Stmt::Try(node) => {
            contains_break(&node.body)
                || node.handlers.iter().any(|handler| {
                    let ExceptHandler::ExceptHandler(h) = handler;
                    contains_break(&h.body)
                })
                || contains_break(&node.orelse)
                || contains_break(&node.finalbody)
        }
        // `For`/`While` open their own break scope; functions/classes are
        // boundaries.
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::expect_used,
        reason = "test-only parsing of fixed, known-valid fixtures"
    )]

    use super::*;

    fn body_of(source: &str) -> Vec<Stmt> {
        let parsed = ruff_python_parser::parse_module(source).expect("fixture parses");
        parsed.syntax().body.to_vec()
    }

    /// A synthesis oracle that types every expression `Unknown`.
    fn unknown_synth(_expr: &Expr) -> InferredType {
        InferredType::Unknown
    }

    /// The definitional divergers and the fall-through cases.
    #[test]
    fn definitional_statements_diverge() {
        for (source, expected) in [
            ("return\n", true),
            ("raise ValueError()\n", true),
            ("x = 1\n", false),
            ("print(1)\n", false),
        ] {
            let body = body_of(source);
            assert_eq!(
                stmts_diverge(&body, &mut unknown_synth),
                expected,
                "{source:?}"
            );
        }
    }

    /// Inference-driven: a call whose type is `Never` diverges; an `Unknown`
    /// call never fabricates divergence (gradual posture).
    #[test]
    fn never_typed_call_diverges_and_unknown_does_not() {
        let body = body_of("fail()\n");
        let mut never_synth = |_: &Expr| InferredType::Never;
        assert!(stmts_diverge(&body, &mut never_synth));
        assert!(!stmts_diverge(&body, &mut unknown_synth));
    }

    /// `if`/`else` where every branch diverges is itself divergent; without
    /// the `else` it is not.
    #[test]
    fn if_needs_an_else_and_all_branches() {
        let with_else = body_of("if c:\n    return\nelse:\n    raise E()\n");
        assert!(stmts_diverge(&with_else, &mut unknown_synth));
        let without_else = body_of("if c:\n    return\n");
        assert!(!stmts_diverge(&without_else, &mut unknown_synth));
        let live_else = body_of("if c:\n    return\nelse:\n    x = 1\n");
        assert!(!stmts_diverge(&live_else, &mut unknown_synth));
    }

    /// `match` needs an unguarded wildcard plus all-diverging cases.
    #[test]
    fn match_needs_a_wildcard_and_all_cases() {
        let exhaustive =
            body_of("match x:\n    case 1:\n        return\n    case _:\n        raise E()\n");
        assert!(stmts_diverge(&exhaustive, &mut unknown_synth));
        let no_wildcard = body_of("match x:\n    case 1:\n        return\n");
        assert!(!stmts_diverge(&no_wildcard, &mut unknown_synth));
        let guarded_wildcard =
            body_of("match x:\n    case 1:\n        return\n    case _ if c:\n        return\n");
        assert!(!stmts_diverge(&guarded_wildcard, &mut unknown_synth));
    }

    /// `while True:` without a `break` diverges; a `break` (even nested in an
    /// `if`) or a non-literal test keeps it reachable. A `break` inside a
    /// NESTED loop does not count.
    #[test]
    fn while_true_divergence_respects_breaks() {
        let mut literal_synth = |expr: &Expr| match expr {
            Expr::BooleanLiteral(lit) if lit.value => {
                InferredType::Literal(crate::types::LiteralValue::Bool(true))
            }
            _ => InferredType::Unknown,
        };
        let spins = body_of("while True:\n    x = 1\n");
        assert!(stmts_diverge(&spins, &mut literal_synth));
        let breaks = body_of("while True:\n    if c:\n        break\n");
        assert!(!stmts_diverge(&breaks, &mut literal_synth));
        let nested_break = body_of("while True:\n    for i in xs:\n        break\n");
        assert!(
            stmts_diverge(&nested_break, &mut literal_synth),
            "a break belonging to a nested loop must not count"
        );
        let unknown_test = body_of("while cond:\n    x = 1\n");
        assert!(!stmts_diverge(&unknown_test, &mut literal_synth));
    }

    /// `try` divergence: a diverging `finally` always diverges; body plus
    /// all-diverging handlers diverges; a live handler keeps it reachable.
    #[test]
    fn try_divergence_paths() {
        let final_diverges = body_of("try:\n    x = 1\nfinally:\n    return\n");
        assert!(stmts_diverge(&final_diverges, &mut unknown_synth));
        let all_paths = body_of("try:\n    return\nexcept E:\n    raise\n");
        assert!(stmts_diverge(&all_paths, &mut unknown_synth));
        let live_handler = body_of("try:\n    return\nexcept E:\n    x = 1\n");
        assert!(!stmts_diverge(&live_handler, &mut unknown_synth));
    }
}
