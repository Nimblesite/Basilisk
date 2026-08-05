//! The per-function definite-assignment walk for [`super::UnboundVariable`]
//! ([CHKARCH-DIAG-TYPESAFETY], [NARROWPLAN-INTEGRATION] Step 8). See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//!
//! Divergence comes from the inference-driven walker
//! ([`crate::narrow::stmt_diverges`]), so a branch that provably never falls
//! through drops out of the merge instead of poisoning it.

use std::collections::HashSet;

use basilisk_resolver::{collect_walrus_targets, Reach, Span};
use ruff_python_ast::{ExceptHandler, Expr, Stmt};
use ruff_text_size::Ranged;

use crate::diagnostic::Diagnostic;
use crate::narrow::{bound_names, stmt_diverges, target_names, SynthFn};
use crate::types::InferredType;

use super::bindings::{
    bind_statement_targets, collect_escaped, irrefutable, merge_alive, pattern_names,
};
use super::make_diagnostic;

/// Per-function state of the definite-assignment walk.
pub(super) struct UnboundScan<'a> {
    /// Every name the body binds anywhere — the gate: only local variables
    /// (not globals or builtins) can be "unbound on some paths".
    all_assigns: HashSet<String>,
    /// Parameter names — always bound.
    params: HashSet<String>,
    /// `global`/`nonlocal`-declared names — bound in an enclosing scope.
    escaped: HashSet<String>,
    func_name: &'a str,
    path: &'a str,
    /// Non-zero inside constructs where "bound" is path-dependent beyond
    /// this walk's precision (loop bodies, handlers, `finally`): abstain.
    suppress: u32,
}

impl<'a> UnboundScan<'a> {
    pub(super) fn for_function(func: &'a ruff_python_ast::StmtFunctionDef, path: &'a str) -> Self {
        let mut all_assigns = HashSet::new();
        bound_names(&func.body, &mut all_assigns);
        let params = func
            .parameters
            .iter()
            .map(|p| p.name().to_string())
            .collect();
        let mut escaped = HashSet::new();
        collect_escaped(&func.body, &mut escaped);
        Self {
            all_assigns,
            params,
            escaped,
            func_name: func.name.as_str(),
            path,
            suppress: 0,
        }
    }

    /// Walk a statement list; `true` when the list definitely diverges.
    pub(super) fn walk_block(
        &mut self,
        stmts: &[Stmt],
        bound: &mut HashSet<String>,
        synth: &mut SynthFn<'_>,
        out: &mut Vec<Diagnostic>,
    ) -> bool {
        for stmt in stmts {
            // PEP 572: a walrus in the statement's OWN expressions binds
            // whenever control reaches it, exactly like a prior assignment.
            bound.extend(collect_walrus_targets(
                std::slice::from_ref(stmt),
                Reach::Definite,
            ));
            if self.walk_stmt(stmt, bound, synth, out) {
                return true;
            }
        }
        false
    }

    /// Walk one statement; `true` when it definitely diverges.
    fn walk_stmt(
        &mut self,
        stmt: &Stmt,
        bound: &mut HashSet<String>,
        synth: &mut SynthFn<'_>,
        out: &mut Vec<Diagnostic>,
    ) -> bool {
        match stmt {
            Stmt::Return(node) => {
                self.check_return(node, bound, out);
                true
            }
            Stmt::Raise(_) => true,
            Stmt::Expr(node) => synth(&node.value) == InferredType::Never,
            Stmt::If(node) => self.walk_if(node, bound, synth, out),
            Stmt::Try(node) => self.walk_try(node, bound, synth, out),
            Stmt::While(_) | Stmt::For(_) => self.walk_loop(stmt, bound, synth, out),
            Stmt::With(node) => self.walk_with(node, bound, synth, out),
            Stmt::Match(node) => self.walk_match(node, bound, synth, out),
            _ => {
                bind_statement_targets(stmt, bound);
                false
            }
        }
    }

    /// `return <name>`: fire when the name is a local variable the walk
    /// could not prove bound on every live path reaching this statement.
    fn check_return(
        &self,
        node: &ruff_python_ast::StmtReturn,
        bound: &HashSet<String>,
        out: &mut Vec<Diagnostic>,
    ) {
        if self.suppress > 0 {
            return;
        }
        let Some(Expr::Name(name)) = node.value.as_deref() else {
            return;
        };
        let id = name.id.as_str();
        if self.params.contains(id)
            || self.escaped.contains(id)
            || bound.contains(id)
            || !self.all_assigns.contains(id)
        {
            return;
        }
        let range = name.range();
        let span = Span {
            start: range.start().to_u32(),
            end: range.end().to_u32(),
        };
        out.push(make_diagnostic(self.func_name, id, span, self.path));
    }

    /// Branch merge: names bound after the `if` are those bound in EVERY
    /// live (non-diverging) branch; a branch that diverges drops out.
    fn walk_if(
        &mut self,
        node: &ruff_python_ast::StmtIf,
        bound: &mut HashSet<String>,
        synth: &mut SynthFn<'_>,
        out: &mut Vec<Diagnostic>,
    ) -> bool {
        let mut alive = Vec::new();
        let mut branch = bound.clone();
        if !self.walk_block(&node.body, &mut branch, synth, out) {
            alive.push(branch);
        }
        let mut has_else = false;
        for clause in &node.elif_else_clauses {
            has_else |= clause.test.is_none();
            let mut branch = bound.clone();
            if !self.walk_block(&clause.body, &mut branch, synth, out) {
                alive.push(branch);
            }
        }
        if !has_else {
            alive.push(bound.clone());
        }
        merge_alive(bound, alive)
    }

    /// `try` success path is sequential (`body` then `orelse`); each handler
    /// runs from the pre-`try` state with its own binds; `finally` always
    /// runs. Diverging paths drop out of the merge exactly as in `if`.
    fn walk_try(
        &mut self,
        node: &ruff_python_ast::StmtTry,
        bound: &mut HashSet<String>,
        synth: &mut SynthFn<'_>,
        out: &mut Vec<Diagnostic>,
    ) -> bool {
        let mut alive = Vec::new();
        let mut success = bound.clone();
        if !self.walk_block(&node.body, &mut success, synth, out)
            && !self.walk_block(&node.orelse, &mut success, synth, out)
        {
            alive.push(success);
        }
        self.walk_handlers(node, bound, &mut alive, synth, out);
        let mut finals = bound.clone();
        let finally_diverges =
            self.abstaining(|scan| scan.walk_block(&node.finalbody, &mut finals, synth, out));
        if merge_alive(bound, alive) || finally_diverges {
            return true;
        }
        bound.extend(finals);
        false
    }

    /// Each handler starts from the pre-`try` state (the exception may
    /// pre-empt any body assign); reads inside abstain, binds count.
    fn walk_handlers(
        &mut self,
        node: &ruff_python_ast::StmtTry,
        bound: &HashSet<String>,
        alive: &mut Vec<HashSet<String>>,
        synth: &mut SynthFn<'_>,
        out: &mut Vec<Diagnostic>,
    ) {
        for ExceptHandler::ExceptHandler(handler) in &node.handlers {
            let mut inner = bound.clone();
            if let Some(name) = &handler.name {
                let _ = inner.insert(name.to_string());
            }
            let diverges =
                self.abstaining(|scan| scan.walk_block(&handler.body, &mut inner, synth, out));
            if !diverges {
                alive.push(inner);
            }
        }
    }

    /// Loop bodies may run zero times: nothing they bind is definite, and
    /// reads inside abstain (a prior iteration may have bound the name).
    /// Divergence (`while True:` without `break`) is the walker's
    /// inference-driven verdict.
    fn walk_loop(
        &mut self,
        stmt: &Stmt,
        bound: &mut HashSet<String>,
        synth: &mut SynthFn<'_>,
        out: &mut Vec<Diagnostic>,
    ) -> bool {
        if let Stmt::For(node) = stmt {
            // The loop target is treated as bound past the loop — the
            // long-standing acceptance this rule has always granted.
            let mut names = Vec::new();
            target_names(&node.target, &mut names);
            bound.extend(names);
        }
        let (body, orelse) = match stmt {
            Stmt::While(node) => (&node.body, &node.orelse),
            Stmt::For(node) => (&node.body, &node.orelse),
            _ => return false,
        };
        self.abstaining(|scan| {
            let mut inner = bound.clone();
            let _ = scan.walk_block(body, &mut inner, synth, out);
            let mut else_inner = bound.clone();
            let _ = scan.walk_block(orelse, &mut else_inner, synth, out);
        });
        stmt_diverges(stmt, synth)
    }

    /// A `with` body executes whenever the statement is reached: walk it
    /// inline, binds and divergence included.
    fn walk_with(
        &mut self,
        node: &ruff_python_ast::StmtWith,
        bound: &mut HashSet<String>,
        synth: &mut SynthFn<'_>,
        out: &mut Vec<Diagnostic>,
    ) -> bool {
        for item in &node.items {
            if let Some(vars) = item.optional_vars.as_deref() {
                let mut names = Vec::new();
                target_names(vars, &mut names);
                bound.extend(names);
            }
        }
        self.walk_block(&node.body, bound, synth, out)
    }

    /// `match` cases merge like `if` branches; a refutable case set keeps
    /// the implicit no-match fallthrough alive.
    fn walk_match(
        &mut self,
        node: &ruff_python_ast::StmtMatch,
        bound: &mut HashSet<String>,
        synth: &mut SynthFn<'_>,
        out: &mut Vec<Diagnostic>,
    ) -> bool {
        let mut alive = Vec::new();
        let mut exhaustive = false;
        for case in &node.cases {
            let mut branch = bound.clone();
            pattern_names(&case.pattern, &mut branch);
            if !self.walk_block(&case.body, &mut branch, synth, out) {
                alive.push(branch);
            }
            exhaustive |= case.guard.is_none() && irrefutable(&case.pattern);
        }
        if !exhaustive {
            alive.push(bound.clone());
        }
        merge_alive(bound, alive)
    }

    /// Run `body` with firing suppressed (binds and divergence still count).
    fn abstaining<R>(&mut self, body: impl FnOnce(&mut Self) -> R) -> R {
        self.suppress += 1;
        let result = body(self);
        self.suppress -= 1;
        result
    }
}
