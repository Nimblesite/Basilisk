//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Narrowing guard types collected from function bodies.
//!
//! These represent control-flow-sensitive type narrowing facts extracted
//! during AST resolution. They are collected for the planned checker
//! narrowing engine (see NARROWPLAN Phase 1 in
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md) and currently have
//! no consumer; live narrowing for `assert_type` uses the flow environment
//! in `visitor/assert_narrow.rs` instead ([TYPEINF-NARROWING-ASSIGN]).

use super::span::Span;

/// A type narrowing guard detected in a function body.
#[derive(Debug, Clone, PartialEq)]
pub struct NarrowingGuard {
    /// What kind of narrowing this guard performs.
    pub kind: NarrowingGuardKind,
    /// The span of the entire guard expression (for diagnostics).
    pub span: Span,
    /// Nesting depth within the function body (0 = top-level).
    ///
    /// Guards inside loops (`for`/`while`) have `in_loop = true` and
    /// their narrowing does NOT persist after the loop body.
    pub in_loop: bool,
}

/// The kind of narrowing guard.
#[derive(Debug, Clone, PartialEq)]
pub enum NarrowingGuardKind {
    /// `isinstance(var, Type)` — narrows `var` to `Type` in the positive branch,
    /// complement in the negative branch (§7.1).
    IsInstance {
        /// The variable being narrowed.
        variable: String,
        /// The type(s) being checked against (e.g. `"int"`, `"(int, str)"`).
        type_names: Vec<String>,
        /// Span of the `if` block where the narrowing applies.
        if_body_span: Span,
        /// Span of the `else` block (if present) where the complement applies.
        else_body_span: Option<Span>,
    },
    /// `x is None` — narrows `x` to `None` in the positive branch,
    /// removes `None` in the negative branch (§7.2).
    IsNone {
        /// The variable being narrowed.
        variable: String,
        /// `true` for `is None`, `false` for `is not None`.
        is_positive: bool,
        /// Span of the `if` block.
        if_body_span: Span,
        /// Span of the `else` block (if present).
        else_body_span: Option<Span>,
    },
    /// Truthiness narrowing — `if x:` removes falsy types in the positive branch (§7.3).
    Truthiness {
        /// The variable being narrowed.
        variable: String,
        /// Span of the `if` block (truthy branch).
        if_body_span: Span,
        /// Span of the `else` block (falsy branch, if present).
        else_body_span: Option<Span>,
    },
    /// Assignment narrowing — `x = expr` narrows `x` to the type of `expr` (§7.4).
    Assignment {
        /// The variable being narrowed.
        variable: String,
        /// The annotation text of the assigned type, if determinable.
        assigned_type: Option<String>,
    },
    /// `assert x is not None` or `assert isinstance(x, T)` — narrows for all
    /// subsequent code in the same scope (§7.8).
    Assert {
        /// The inner guard that the assert enforces.
        inner: Box<NarrowingGuardKind>,
    },
    /// `TypeGuard[T]` return — narrows the first argument in the positive branch only (§7.6).
    TypeGuard {
        /// The variable being narrowed (first argument to the guard function).
        variable: String,
        /// The type the guard narrows to.
        guard_type: String,
        /// Span of the `if` block.
        if_body_span: Span,
        /// Span of the `else` block (type is NOT narrowed here for `TypeGuard`).
        else_body_span: Option<Span>,
    },
    /// `TypeIs[T]` return — narrows bidirectionally: positive branch gets `T`,
    /// negative branch gets complement (§7.7).
    TypeIs {
        /// The variable being narrowed.
        variable: String,
        /// The type the guard narrows to.
        guard_type: String,
        /// Span of the `if` block.
        if_body_span: Span,
        /// Span of the `else` block.
        else_body_span: Option<Span>,
    },
    /// `match` statement — each `case` branch narrows the subject (§7.5).
    Match {
        /// The variable being matched.
        variable: String,
        /// Cases with their pattern type and body span.
        cases: Vec<MatchCaseNarrowing>,
        /// Whether a wildcard `case _:` is present (exhaustive).
        has_wildcard: bool,
    },
}

/// A single `case` branch in a match statement narrowing.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchCaseNarrowing {
    /// The type that the match subject is narrowed to in this case.
    pub pattern_type: String,
    /// The span of the case body.
    pub body_span: Span,
}
