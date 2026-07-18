//! Implements [TYPEINF-TARGET-NARROWING]. See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-NARROWING
//! The statement-level flow walker: drives a [`NarrowEnv`] through a function
//! body, applying resolver-collected guards at branches (positive frame in
//! the `if` body, complement in the `else`, `phi`-join at the merge),
//! persisting the complement after a diverging branch (early exit), applying
//! `assert` narrowing whole-scope, and modelling assignment narrowing without
//! ever touching the declared type ([NARROWPLAN-CHECKLIST] Stage 2).

use std::collections::HashMap;

use basilisk_resolver::NarrowingGuard;
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use crate::bidir::BidirEngine;
use crate::types::InferredType;

use super::env::NarrowEnv;
use super::guards::{guard_outcomes_in, GuardOutcome};

/// One narrowed name-use site: the location and the type visible there.
#[derive(Debug, Clone, PartialEq)]
pub struct NarrowedUse {
    /// The variable read at this site.
    pub name: String,
    /// Byte offset of the use within the analysed body's source.
    pub start: u32,
    /// End byte offset of the use.
    pub end: u32,
    /// The flow-narrowed type at this point.
    pub narrowed: InferredType,
}

/// The outcome of walking one function body.
#[derive(Debug, Default)]
pub struct FlowResult {
    /// Every `Name` read whose flow-narrowed type differs from its declared
    /// type — the sites hover/diagnostics consume.
    pub narrowed_uses: Vec<NarrowedUse>,
    /// Body ranges of branches whose guard narrows a variable to `Never` —
    /// **inference-driven reachability** ([TYPEINF-TARGET-NARROWING]): the
    /// branch is unreachable because the type lattice proves the guard can
    /// never hold, not because a syntactic idiom matched.
    pub unreachable_ranges: Vec<(u32, u32)>,
}

/// Walk `body` under `env`'s declared types, consuming the function's
/// resolver-collected `guards` (matched to `if`/`assert` statements by
/// test-expression span).
#[must_use]
pub fn analyse_function(body: &[Stmt], env: NarrowEnv, guards: &[NarrowingGuard]) -> FlowResult {
    analyse_function_in(body, env, guards, &super::guards::NarrowContext::default())
}

/// [`analyse_function`] with module facts (`TypedDict` schemas) available to
/// the guard interpreter.
#[must_use]
pub fn analyse_function_in(
    body: &[Stmt],
    env: NarrowEnv,
    guards: &[NarrowingGuard],
    ctx: &super::guards::NarrowContext,
) -> FlowResult {
    let mut walker = FlowWalker {
        env,
        guards_by_span: guards
            .iter()
            .map(|guard| ((guard.span.start, guard.span.end), guard))
            .collect(),
        ctx,
        result: FlowResult::default(),
    };
    walker.walk_stmts(body);
    walker.result
}

/// Internal walker state.
struct FlowWalker<'g> {
    env: NarrowEnv,
    guards_by_span: HashMap<(u32, u32), &'g NarrowingGuard>,
    ctx: &'g super::guards::NarrowContext,
    result: FlowResult,
}

impl FlowWalker<'_> {
    fn walk_stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.walk_stmt(stmt);
        }
    }

    fn walk_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::If(node) => self.walk_if(node),
            Stmt::Assert(node) => self.walk_assert(node),
            Stmt::Assign(node) => self.walk_assign(node),
            Stmt::AnnAssign(node) => {
                if let Some(value) = node.value.as_deref() {
                    self.record_uses(value);
                }
            }
            Stmt::Return(node) => {
                if let Some(value) = node.value.as_deref() {
                    self.record_uses(value);
                }
            }
            Stmt::Expr(node) => self.record_uses(&node.value),
            // Loop bodies: narrowing inside must not escape — walk within a
            // discarded branch frame ([TYPEINF-NARROWING-SCOPE]).
            Stmt::For(node) => {
                self.record_uses(&node.iter);
                self.walk_discarded(&node.body);
                self.walk_discarded(&node.orelse);
            }
            Stmt::While(node) => {
                self.record_uses(&node.test);
                self.walk_discarded(&node.body);
                self.walk_discarded(&node.orelse);
            }
            Stmt::With(node) => self.walk_stmts(&node.body),
            Stmt::Try(node) => self.walk_try(node),
            Stmt::Match(node) => self.walk_match(node),
            // Everything else — including nested functions/classes, which
            // are a narrowing BOUNDARY (enclosing narrows must not flow into
            // a closure that may run later) — is not walked.
            _ => {}
        }
    }

    /// `if <test>: ...` — positive branch, complement branch, then join; a
    /// diverging branch persists the OTHER branch's narrowing (early exit).
    fn walk_if(&mut self, node: &ruff_python_ast::StmtIf) {
        self.record_uses(&node.test);
        let outcome = self.lookup_outcome(node.test.range());
        let then_diverges = diverges(&node.body);
        let else_body: Vec<&Stmt> = node
            .elif_else_clauses
            .iter()
            .filter(|clause| clause.test.is_none())
            .flat_map(|clause| clause.body.iter())
            .collect();
        let else_diverges = !else_body.is_empty() && diverges_refs(&else_body);

        let then_frame = self.walk_branch(&node.body, outcome.as_ref(), true);
        let else_frame = self.walk_else(&node.elif_else_clauses, outcome.as_ref());

        match (&outcome, then_diverges, else_diverges) {
            // `if x is None: return` — the complement holds afterwards.
            (Some(out), true, false) => self.env.narrow(&out.variable, out.negative.clone()),
            (Some(out), false, true) => self.env.narrow(&out.variable, out.positive.clone()),
            _ => self.env.join(then_frame, else_frame),
        }
    }

    /// Walk one branch under a guard polarity, returning its frame.
    fn walk_branch(
        &mut self,
        body: &[Stmt],
        outcome: Option<&GuardOutcome>,
        positive: bool,
    ) -> HashMap<String, InferredType> {
        self.env.push_branch();
        if let Some(out) = outcome {
            let ty = if positive {
                out.positive.clone()
            } else {
                out.negative.clone()
            };
            if ty == InferredType::Never {
                if let Some(range) = body_range(body) {
                    self.result.unreachable_ranges.push(range);
                }
            }
            self.env.narrow(&out.variable, ty);
        }
        self.walk_stmts(body);
        self.env.pop_branch()
    }

    /// Walk `elif`/`else` clauses; only the plain `else` receives the
    /// complement (an `elif` has its own guard, looked up on its own test).
    fn walk_else(
        &mut self,
        clauses: &[ruff_python_ast::ElifElseClause],
        outcome: Option<&GuardOutcome>,
    ) -> HashMap<String, InferredType> {
        let mut merged = HashMap::new();
        for clause in clauses {
            match &clause.test {
                Some(test) => {
                    self.record_uses(test);
                    let elif_outcome = self.lookup_outcome(test.range());
                    let _ = self.walk_branch(&clause.body, elif_outcome.as_ref(), true);
                }
                None => {
                    merged = self.walk_branch(&clause.body, outcome, false);
                }
            }
        }
        merged
    }

    /// `assert <test>` — whole-scope narrowing from this point on.
    fn walk_assert(&mut self, node: &ruff_python_ast::StmtAssert) {
        self.record_uses(&node.test);
        if let Some(outcome) = self.lookup_outcome(node.range()) {
            self.env.narrow(&outcome.variable, outcome.positive);
        }
    }

    /// `x = expr` — assignment narrowing: the flow type of `x` becomes the
    /// synthesized type of `expr`; the DECLARED type (what assignment
    /// validation checks against) is untouched by design.
    fn walk_assign(&mut self, node: &ruff_python_ast::StmtAssign) {
        self.record_uses(&node.value);
        if let [Expr::Name(target)] = node.targets.as_slice() {
            let ty = synth_type(&node.value);
            self.env.narrow(target.id.as_str(), ty);
        }
    }

    /// `match <subject>: case ...` — per-case narrowing of the subject
    /// ([TYPEINF-NARROWING-MATCH]): each case body walks in its own branch
    /// frame with the subject intersected with the case's pattern type.
    fn walk_match(&mut self, node: &ruff_python_ast::StmtMatch) {
        self.record_uses(&node.subject);
        let range = node.range();
        let key = (u32::from(range.start()), u32::from(range.end()));
        let match_guard = self.guards_by_span.get(&key).copied();
        for case in &node.cases {
            self.env.push_branch();
            self.narrow_match_case(match_guard, case);
            self.walk_stmts(&case.body);
            let _ = self.env.pop_branch();
        }
        self.narrow_after_match(match_guard, node);
    }

    /// Implied-else exhaustiveness: when every case body diverges, reaching
    /// past the `match` means no case matched — the subject loses every
    /// covered pattern type ([TYPEINF-NARROWING-MATCH]).
    fn narrow_after_match(
        &mut self,
        match_guard: Option<&NarrowingGuard>,
        node: &ruff_python_ast::StmtMatch,
    ) {
        let Some(guard) = match_guard.filter(|guard| !guard.in_loop) else {
            return;
        };
        let basilisk_resolver::NarrowingGuardKind::Match {
            variable,
            cases,
            has_wildcard,
        } = &guard.kind
        else {
            return;
        };
        let all_diverge = node.cases.iter().all(|case| diverges(&case.body));
        if *has_wildcard || !all_diverge {
            return;
        }
        let Some(current) = self.env.lookup(variable) else {
            return;
        };
        let covered = cases
            .iter()
            .map(|case| InferredType::from_annotation(&case.pattern_type))
            .fold(InferredType::Never, InferredType::union);
        self.env
            .narrow(variable, super::set_ops::subtract(&current, &covered));
    }

    /// Apply one case's pattern narrowing, when the resolver captured it.
    fn narrow_match_case(
        &mut self,
        match_guard: Option<&NarrowingGuard>,
        case: &ruff_python_ast::MatchCase,
    ) {
        let Some(guard) = match_guard.filter(|guard| !guard.in_loop) else {
            return;
        };
        let basilisk_resolver::NarrowingGuardKind::Match {
            variable, cases, ..
        } = &guard.kind
        else {
            return;
        };
        let Some(pattern) = matching_case(cases, case) else {
            return;
        };
        let Some(current) = self.env.lookup(variable) else {
            return;
        };
        let target = InferredType::from_annotation(&pattern.pattern_type);
        let narrowed = super::set_ops::intersect(&current, &target);
        // A `Never` here usually means a VALUE pattern (`case 1:`) whose text
        // is not a type — never fabricate unreachability from that.
        if narrowed != InferredType::Never {
            self.env.narrow(variable, narrowed);
        }
    }

    /// Try/except: handler and else/final bodies walk in discarded frames —
    /// exceptions make mid-body narrowing unreliable.
    fn walk_try(&mut self, node: &ruff_python_ast::StmtTry) {
        self.walk_discarded(&node.body);
        for handler in &node.handlers {
            let ruff_python_ast::ExceptHandler::ExceptHandler(h) = handler;
            self.walk_discarded(&h.body);
        }
        self.walk_discarded(&node.orelse);
        self.walk_stmts(&node.finalbody);
    }

    /// Walk statements inside a frame whose narrowing is thrown away.
    fn walk_discarded(&mut self, stmts: &[Stmt]) {
        self.env.push_branch();
        self.walk_stmts(stmts);
        let _ = self.env.pop_branch();
    }

    /// Find the guard whose collected span matches this range.
    fn lookup_outcome(&self, range: ruff_text_size::TextRange) -> Option<GuardOutcome> {
        let key = (u32::from(range.start()), u32::from(range.end()));
        let guard = self.guards_by_span.get(&key)?;
        let current = self.env.lookup(variable_of(guard))?;
        guard_outcomes_in(guard, &current, self.ctx)
    }

    /// Record every narrowed `Name` read inside `expr`.
    fn record_uses(&mut self, expr: &Expr) {
        let mut names = Vec::new();
        collect_name_reads(expr, &mut names);
        for (name, start, end) in names {
            let Some(narrowed) = self.env.lookup(&name) else {
                continue;
            };
            if self.env.declared(&name) == Some(&narrowed) {
                continue;
            }
            self.result.narrowed_uses.push(NarrowedUse {
                name,
                start,
                end,
                narrowed,
            });
        }
    }
}

/// The variable a guard narrows (guards produced by the resolver always
/// target exactly one name).
fn variable_of(guard: &NarrowingGuard) -> &str {
    variable_of_kind(&guard.kind)
}

/// Recurse into an `assert`'s inner guard kind for its variable.
fn variable_of_kind(kind: &basilisk_resolver::NarrowingGuardKind) -> &str {
    use basilisk_resolver::NarrowingGuardKind as K;
    match kind {
        K::IsInstance { variable, .. }
        | K::IsNone { variable, .. }
        | K::Truthiness { variable, .. }
        | K::Assignment { variable, .. }
        | K::TypeGuard { variable, .. }
        | K::TypeIs { variable, .. }
        | K::Match { variable, .. }
        | K::IsSubclass { variable, .. }
        | K::EqualsLiteral { variable, .. }
        | K::InLiterals { variable, .. }
        | K::HasAttr { variable, .. }
        | K::TypeOfIs { variable, .. }
        | K::KeyInDict { variable, .. } => variable,
        K::Assert { inner } => variable_of_kind(inner),
    }
}

/// Find the resolver case entry matching an AST case, by body span.
fn matching_case<'c>(
    cases: &'c [basilisk_resolver::MatchCaseNarrowing],
    case: &ruff_python_ast::MatchCase,
) -> Option<&'c basilisk_resolver::MatchCaseNarrowing> {
    let start = u32::from(case.body.first()?.range().start());
    let end = u32::from(case.body.last()?.range().end());
    cases
        .iter()
        .find(|entry| entry.body_span.start == start && entry.body_span.end == end)
}

/// The byte range spanned by a statement list, when non-empty.
fn body_range(body: &[Stmt]) -> Option<(u32, u32)> {
    let start = u32::from(body.first()?.range().start());
    let end = u32::from(body.last()?.range().end());
    Some((start, end))
}

/// Whether a statement list definitely diverges (ends in `return`/`raise`/
/// `continue`/`break`) — the early-exit signal for complement persistence.
fn diverges(stmts: &[Stmt]) -> bool {
    matches!(
        stmts.last(),
        Some(Stmt::Return(_) | Stmt::Raise(_) | Stmt::Continue(_) | Stmt::Break(_))
    )
}

/// [`diverges`] over a collected reference list (the `else` clause bodies).
fn diverges_refs(stmts: &[&Stmt]) -> bool {
    matches!(
        stmts.last(),
        Some(Stmt::Return(_) | Stmt::Raise(_) | Stmt::Continue(_) | Stmt::Break(_))
    )
}

/// Synthesize an expression's type through the bidirectional engine.
fn synth_type(expr: &Expr) -> InferredType {
    let mut engine = BidirEngine::new(HashMap::new());
    let ty = engine.synth(expr);
    let solution = engine.finish();
    ty.to_inferred(&solution.vars)
}

/// Visitor collecting every `Name` read with its range.
struct NameReads<'a>(&'a mut Vec<(String, u32, u32)>);

impl ruff_python_ast::visitor::Visitor<'_> for NameReads<'_> {
    fn visit_expr(&mut self, expr: &Expr) {
        if let Expr::Name(name) = expr {
            let range = name.range();
            self.0.push((
                name.id.to_string(),
                u32::from(range.start()),
                u32::from(range.end()),
            ));
        }
        ruff_python_ast::visitor::walk_expr(self, expr);
    }
}

/// Collect `(name, start, end)` for every `Name` read in an expression.
fn collect_name_reads(expr: &Expr, out: &mut Vec<(String, u32, u32)>) {
    use ruff_python_ast::visitor::Visitor as _;
    NameReads(out).visit_expr(expr);
}
