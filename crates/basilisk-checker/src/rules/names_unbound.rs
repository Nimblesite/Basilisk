//! Implements [`names_unbound`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! `names_unbound`: possibly-unbound variable at a `return`.
//!
//! [NARROWPLAN-INTEGRATION] Step 8
//! ([#285](https://github.com/Nimblesite/Basilisk/issues/285)): definite
//! assignment is tracked over ALL paths, and divergence is the walker's
//! inference-driven analysis ([NARROWPLAN-FLOW],
//! [`crate::narrow::stmt_diverges`]) — a branch that provably never falls
//! through (`return`, `raise`, a `NoReturn`-typed call, `while True:`
//! without `break`) cannot leave the name unbound, so it drops out of the
//! merge instead of poisoning it.
//!
//! ```python
//! def maybe_assign(flag: bool) -> int:
//!     if flag:
//!         result = 42
//!     return result   # result may be unbound if flag is False → names_unbound
//!
//! def guarded(flag: bool) -> int:
//!     if flag:
//!         result = 42
//!     else:
//!         return 0    # this path never reaches the return below
//!     return result   # bound on every live path — silent
//! ```
//!
//! Gradual posture ([TYPEINF-TARGET-GRADUAL]): a read the walk cannot prove
//! bound on every live path fires only where the walk is exact (straight
//! lines, `if`/`elif`/`else`, `try` success paths, `match` cases, `with`
//! bodies); inside loop bodies, `except` handlers, and `finally` blocks —
//! where an earlier iteration or a mid-statement exception makes "bound"
//! path-dependent — the walk abstains.

use std::collections::HashSet;

use basilisk_resolver::{collect_walrus_targets, Reach, ResolvedModule, Span};
use ruff_python_ast::{ExceptHandler, Expr, Pattern, Stmt};
use ruff_text_size::Ranged;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::narrow::{bound_names, stmt_diverges, target_names, SynthFn};
use crate::types::InferredType;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "names_unbound",
    docs_url: "https://www.basilisk-python.dev/errors/names_unbound",
};

/// Emits `names_unbound` for `return` statements that reference names not
/// bound on every live path.
pub(crate) struct UnboundVariable;

impl Rule for UnboundVariable {
    fn check(
        &self,
        module: &ResolvedModule,
        ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        super::check_with_own_types(self, module, ctx, diagnostics);
    }

    fn check_with_types(
        &self,
        module: &ResolvedModule,
        types: &super::shared::module_types::ModuleTypes<'_>,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };
        let oracle = types.oracle();
        // Divergence consults the engine: a call statement typed `Never`
        // (`NoReturn`) diverges; anything unprovable stays reachable.
        let mut synth = |expr: &Expr| -> InferredType {
            oracle
                .and_then(|o| o.synth_span(expr_span(expr)))
                .unwrap_or(InferredType::Unknown)
        };
        check_functions_in(&parsed.ast.body, &module.path, &mut synth, diagnostics);
    }
}

/// Byte span of an expression.
fn expr_span(expr: &Expr) -> Span {
    let range = expr.range();
    Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    }
}

/// Analyse every function definition, at any nesting depth.
fn check_functions_in(
    stmts: &[Stmt],
    path: &str,
    synth: &mut SynthFn<'_>,
    out: &mut Vec<Diagnostic>,
) {
    for stmt in stmts {
        if let Stmt::FunctionDef(func) = stmt {
            analyse_function(func, path, synth, out);
        }
        for body in nested_bodies(stmt) {
            check_functions_in(body, path, synth, out);
        }
    }
}

/// The statement lists a compound statement nests (for function discovery).
fn nested_bodies(stmt: &Stmt) -> Vec<&[Stmt]> {
    match stmt {
        Stmt::FunctionDef(node) => vec![&node.body],
        Stmt::ClassDef(node) => vec![&node.body],
        Stmt::If(node) => std::iter::once(node.body.as_slice())
            .chain(node.elif_else_clauses.iter().map(|c| c.body.as_slice()))
            .collect(),
        Stmt::While(node) => vec![&node.body, &node.orelse],
        Stmt::For(node) => vec![&node.body, &node.orelse],
        Stmt::With(node) => vec![&node.body],
        Stmt::Try(node) => {
            let mut bodies = vec![node.body.as_slice(), node.orelse.as_slice()];
            bodies.extend(
                node.handlers
                    .iter()
                    .map(|ExceptHandler::ExceptHandler(h)| h.body.as_slice()),
            );
            bodies.push(&node.finalbody);
            bodies
        }
        Stmt::Match(node) => node.cases.iter().map(|c| c.body.as_slice()).collect(),
        _ => Vec::new(),
    }
}

/// Run the definite-assignment walk over one function body.
fn analyse_function(
    func: &ruff_python_ast::StmtFunctionDef,
    path: &str,
    synth: &mut SynthFn<'_>,
    out: &mut Vec<Diagnostic>,
) {
    let mut scan = UnboundScan::for_function(func, path);
    let mut bound = HashSet::new();
    let _ = scan.walk_block(&func.body, &mut bound, synth, out);
}

/// Per-function state of the definite-assignment walk.
struct UnboundScan<'a> {
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
    fn for_function(func: &'a ruff_python_ast::StmtFunctionDef, path: &'a str) -> Self {
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
    fn walk_block(
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

/// Merge live branch states into `bound`; `true` when no branch is live
/// (the whole construct diverges).
fn merge_alive(bound: &mut HashSet<String>, alive: Vec<HashSet<String>>) -> bool {
    let merged = alive
        .into_iter()
        .reduce(|acc, set| acc.intersection(&set).cloned().collect());
    match merged {
        Some(names) => {
            *bound = names;
            false
        }
        None => true,
    }
}

/// Names a simple (non-branching) statement definitely binds.
fn bind_statement_targets(stmt: &Stmt, bound: &mut HashSet<String>) {
    let mut names = Vec::new();
    match stmt {
        Stmt::Assign(node) => {
            for target in &node.targets {
                target_names(target, &mut names);
            }
        }
        Stmt::AnnAssign(node) => target_names(&node.target, &mut names),
        Stmt::AugAssign(node) => target_names(&node.target, &mut names),
        Stmt::FunctionDef(node) => names.push(node.name.to_string()),
        Stmt::ClassDef(node) => names.push(node.name.to_string()),
        Stmt::TypeAlias(node) => target_names(&node.name, &mut names),
        Stmt::Import(node) => names.extend(import_bound_names(node)),
        Stmt::ImportFrom(node) => names.extend(from_import_bound_names(node)),
        Stmt::Delete(node) => {
            let mut deleted = Vec::new();
            for target in &node.targets {
                target_names(target, &mut deleted);
            }
            for name in deleted {
                let _ = bound.remove(&name);
            }
        }
        _ => {}
    }
    bound.extend(names);
}

/// Names a plain `import` statement binds (`import a.b` binds `a`).
fn import_bound_names(node: &ruff_python_ast::StmtImport) -> Vec<String> {
    node.names
        .iter()
        .map(|alias| {
            alias.asname.as_ref().map_or_else(
                || {
                    alias
                        .name
                        .split('.')
                        .next()
                        .unwrap_or(alias.name.as_str())
                        .to_string()
                },
                std::string::ToString::to_string,
            )
        })
        .collect()
}

/// Names a `from ... import ...` statement binds.
fn from_import_bound_names(node: &ruff_python_ast::StmtImportFrom) -> Vec<String> {
    node.names
        .iter()
        .filter(|alias| alias.name.as_str() != "*")
        .map(|alias| {
            alias
                .asname
                .as_ref()
                .map_or_else(|| alias.name.to_string(), std::string::ToString::to_string)
        })
        .collect()
}

/// Capture names a `match` pattern binds when it matches.
fn pattern_names(pattern: &Pattern, bound: &mut HashSet<String>) {
    match pattern {
        Pattern::MatchAs(node) => {
            if let Some(name) = &node.name {
                let _ = bound.insert(name.to_string());
            }
            if let Some(inner) = &node.pattern {
                pattern_names(inner, bound);
            }
        }
        Pattern::MatchStar(node) => {
            if let Some(name) = &node.name {
                let _ = bound.insert(name.to_string());
            }
        }
        Pattern::MatchMapping(node) => {
            if let Some(rest) = &node.rest {
                let _ = bound.insert(rest.to_string());
            }
            for inner in &node.patterns {
                pattern_names(inner, bound);
            }
        }
        Pattern::MatchOr(node) => {
            for inner in &node.patterns {
                pattern_names(inner, bound);
            }
        }
        Pattern::MatchSequence(node) => {
            for inner in &node.patterns {
                pattern_names(inner, bound);
            }
        }
        Pattern::MatchClass(node) => {
            for inner in &node.arguments.patterns {
                pattern_names(inner, bound);
            }
            for kw in &node.arguments.keywords {
                pattern_names(&kw.pattern, bound);
            }
        }
        Pattern::MatchValue(_) | Pattern::MatchSingleton(_) => {}
    }
}

/// `case _:` and bare `case name:` match anything.
fn irrefutable(pattern: &Pattern) -> bool {
    matches!(pattern, Pattern::MatchAs(node) if node.pattern.is_none())
}

/// Collect `global`/`nonlocal` declarations (not entering nested scopes).
fn collect_escaped(stmts: &[Stmt], out: &mut HashSet<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::Global(node) => out.extend(node.names.iter().map(ToString::to_string)),
            Stmt::Nonlocal(node) => out.extend(node.names.iter().map(ToString::to_string)),
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
            _ => {
                for body in nested_bodies(stmt) {
                    collect_escaped(body, out);
                }
            }
        }
    }
}

fn make_diagnostic(func_name: &str, name: &str, span: Span, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Function `{func_name}` returns `{name}` but `{name}` may be unbound on some paths"
        ),
        span,
        path,
        Some(format!(
            "Assign `{name}` unconditionally before the `return`, or add a default value"
        )),
        Some(
            "Basilisk detects variables that are assigned only inside conditional branches \
             (if/while/try) and may not be defined on every execution path"
                .to_owned(),
        ),
    )
}
