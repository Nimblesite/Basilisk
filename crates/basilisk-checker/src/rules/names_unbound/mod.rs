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

use basilisk_resolver::{ResolvedModule, Span};
use ruff_python_ast::{ExceptHandler, Expr, Stmt};
use ruff_text_size::Ranged;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::narrow::SynthFn;
use crate::types::InferredType;

use super::Rule;

mod bindings;
mod scan;

use scan::UnboundScan;

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
pub(super) fn expr_span(expr: &Expr) -> Span {
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
pub(super) fn nested_bodies(stmt: &Stmt) -> Vec<&[Stmt]> {
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

pub(super) fn make_diagnostic(func_name: &str, name: &str, span: Span, path: &str) -> Diagnostic {
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
