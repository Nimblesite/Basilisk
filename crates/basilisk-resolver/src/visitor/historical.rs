//! Historical visitor functions.

use ruff_python_ast::{ExceptHandler, Expr, Stmt, StmtFunctionDef};
use ruff_text_size::Ranged;

use crate::scope::{HistoricalPositionalViolation, HistoricalPositionalViolationKind};

use super::calls_and_reveal::call_func_name;
use super::core::text_range_to_span;

pub(super) fn is_historical_posonly_name(name: &str) -> bool {
    name.starts_with("__") && !name.ends_with("__")
}

/// Build a map from function/method name to the set of parameter names that
/// are positional-only by the historical `__name` convention.
///
/// Only parameters in `args` (not `kwonlyargs`) count; functions that use
/// PEP 570 `/` syntax (`posonlyargs` is non-empty) are excluded.
pub(super) fn collect_historical_posonly_func_params(
    stmts: &[Stmt],
) -> std::collections::HashMap<String, std::collections::HashSet<String>> {
    let mut map = std::collections::HashMap::new();
    collect_posonly_params_from_stmts(stmts, &mut map);
    map
}

pub(super) fn collect_posonly_params_from_stmts(
    stmts: &[Stmt],
    map: &mut std::collections::HashMap<String, std::collections::HashSet<String>>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDef(func) => {
                let params = &func.parameters;
                // Historical convention does not apply when PEP 570 `/` is used.
                if params.posonlyargs.is_empty() {
                    let posonly: std::collections::HashSet<String> = params
                        .args
                        .iter()
                        .map(|p| p.parameter.name.as_str())
                        .filter(|name| is_historical_posonly_name(name))
                        .map(str::to_owned)
                        .collect();
                    if !posonly.is_empty() {
                        let _ = map.insert(func.name.to_string(), posonly);
                    }
                }
                collect_posonly_params_from_stmts(&func.body, map);
            }
            Stmt::ClassDef(cls) => {
                collect_posonly_params_from_stmts(&cls.body, map);
            }
            _ => {}
        }
    }
}

pub(super) fn collect_historical_positional_violations(
    stmts: &[Stmt],
) -> Vec<HistoricalPositionalViolation> {
    let posonly_map = collect_historical_posonly_func_params(stmts);
    let mut out = Vec::new();
    collect_hist_violations_from_stmts(stmts, &posonly_map, &mut out);
    out
}

pub(super) fn collect_hist_violations_from_stmts(
    stmts: &[Stmt],
    posonly_map: &std::collections::HashMap<String, std::collections::HashSet<String>>,
    out: &mut Vec<HistoricalPositionalViolation>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDef(func) => {
                check_func_for_hist_posonly_violation(func, out);
                collect_hist_violations_from_stmts(&func.body, posonly_map, out);
            }
            Stmt::ClassDef(cls) => {
                collect_hist_violations_from_stmts(&cls.body, posonly_map, out);
            }
            Stmt::Expr(e) => {
                collect_hist_violations_from_expr(&e.value, posonly_map, out);
            }
            Stmt::Assign(a) => {
                collect_hist_violations_from_expr(&a.value, posonly_map, out);
            }
            Stmt::AnnAssign(a) => {
                if let Some(val) = &a.value {
                    collect_hist_violations_from_expr(val, posonly_map, out);
                }
            }
            Stmt::Return(r) => {
                if let Some(val) = &r.value {
                    collect_hist_violations_from_expr(val, posonly_map, out);
                }
            }
            Stmt::If(node) => {
                collect_hist_violations_from_stmts(&node.body, posonly_map, out);
                for clause in &node.elif_else_clauses {
                    collect_hist_violations_from_stmts(&clause.body, posonly_map, out);
                }
            }
            Stmt::For(node) => {
                collect_hist_violations_from_stmts(&node.body, posonly_map, out);
                collect_hist_violations_from_stmts(&node.orelse, posonly_map, out);
            }
            Stmt::While(node) => {
                collect_hist_violations_from_stmts(&node.body, posonly_map, out);
                collect_hist_violations_from_stmts(&node.orelse, posonly_map, out);
            }
            Stmt::With(node) => {
                collect_hist_violations_from_stmts(&node.body, posonly_map, out);
            }
            Stmt::Try(node) => {
                collect_hist_violations_from_stmts(&node.body, posonly_map, out);
                for handler in &node.handlers {
                    let ExceptHandler::ExceptHandler(eh) = handler;
                    collect_hist_violations_from_stmts(&eh.body, posonly_map, out);
                }
                collect_hist_violations_from_stmts(&node.orelse, posonly_map, out);
                collect_hist_violations_from_stmts(&node.finalbody, posonly_map, out);
            }
            _ => {}
        }
    }
}

pub(super) fn check_func_for_hist_posonly_violation(
    func: &StmtFunctionDef,
    out: &mut Vec<HistoricalPositionalViolation>,
) {
    let params = &func.parameters;
    if !params.posonlyargs.is_empty() {
        return;
    }
    let mut seen_keyword_param = false;
    for (i, param) in params.args.iter().enumerate() {
        let name = param.parameter.name.as_str();
        if i == 0 && (name == "self" || name == "cls") {
            continue;
        }
        if is_historical_posonly_name(name) {
            if seen_keyword_param {
                out.push(HistoricalPositionalViolation {
                    kind: HistoricalPositionalViolationKind::PositionalOnlyAfterKeyword,
                    span: text_range_to_span(param.parameter.name.range()),
                    name: name.to_owned(),
                });
            }
        } else {
            seen_keyword_param = true;
        }
    }
}

/// Extract the simple function/method name from a call expression's function part.
pub(super) fn collect_hist_violations_from_expr(
    expr: &Expr,
    posonly_map: &std::collections::HashMap<String, std::collections::HashSet<String>>,
    out: &mut Vec<HistoricalPositionalViolation>,
) {
    let Expr::Call(call) = expr else { return };

    let func_name = call_func_name(&call.func);

    for kw in &call.arguments.keywords {
        if let Some(arg_name) = &kw.arg {
            let name_str = arg_name.as_str();
            if is_historical_posonly_name(name_str) {
                // Only flag if we can confirm this param is positional-only in the callee.
                let is_violation = func_name.is_some_and(|fname| {
                    posonly_map
                        .get(fname)
                        .is_some_and(|params| params.contains(name_str))
                });
                if is_violation {
                    out.push(HistoricalPositionalViolation {
                        kind: HistoricalPositionalViolationKind::KeywordPassedToPositionalOnly,
                        span: text_range_to_span(kw.range()),
                        name: name_str.to_owned(),
                    });
                }
            }
        }
    }
    for arg in &call.arguments.args {
        collect_hist_violations_from_expr(arg, posonly_map, out);
    }
    collect_hist_violations_from_expr(&call.func, posonly_map, out);
}

// ---------------------------------------------------------------------------
