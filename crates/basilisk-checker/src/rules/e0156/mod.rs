//! Implements [BSK-E0156] from [CHKARCH-DIAG-TYPEDDICT-EXTRA-ITEMS].
//! See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPEDDICT-EXTRA-ITEMS
//!
//! BSK-E0156: `TypedDict` `extra_items` / `closed` (PEP 728) violations.
//!
//! Validates class-definition legality, dict-literal construction, assignability
//! between `TypedDict`s, and constructor calls against the PEP 728 rules.
//! Operates on the module AST and is independent of resolver state.

mod checks;
mod model;

use std::collections::HashMap;

use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::{ann_str, expr_name, infer_expr_literal_type};

use self::checks::{
    class_def_errors, construction_extra_error, dict_extra_value_error, td_assign_error,
};
use self::model::{collect_models, mk_span, model_map, transitive_fields, TdModel};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0156",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0156",
};

/// Emits BSK-E0156 for PEP 728 `extra_items` / `closed` violations.
pub(crate) struct TypedDictExtraItemsViolation;

impl Rule for TypedDictExtraItemsViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };
        let models = collect_models(&parsed.ast.body);
        if models.is_empty() {
            return;
        }
        let map = model_map(&models);
        let var_td = collect_var_typeddicts(&parsed.ast.body, &map);
        let ctx = Ctx {
            map: &map,
            var_td: &var_td,
            path: &module.path,
        };

        for model in &models {
            for message in class_def_errors(model, &map) {
                push(diagnostics, message, model.span, &module.path);
            }
        }
        check_stmts(&parsed.ast.body, &ctx, diagnostics);
    }
}

/// Shared, borrow-only context threaded through statement traversal.
struct Ctx<'a> {
    map: &'a HashMap<&'a str, &'a TdModel>,
    var_td: &'a HashMap<String, String>,
    path: &'a str,
}

fn push(diagnostics: &mut Vec<Diagnostic>, message: String, span: Span, path: &str) {
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        message,
        span,
        path,
        None,
        None,
    ));
}

// ---------------------------------------------------------------------------
// Variable -> TypedDict annotation map
// ---------------------------------------------------------------------------

/// Map every annotated variable whose declared type names a known `TypedDict`
/// to that `TypedDict` name. Drives source/target resolution for assignments.
fn collect_var_typeddicts(
    stmts: &[Stmt],
    map: &HashMap<&str, &TdModel>,
) -> HashMap<String, String> {
    let mut out = HashMap::new();
    gather_var_typeddicts(stmts, map, &mut out);
    out
}

fn gather_var_typeddicts(
    stmts: &[Stmt],
    map: &HashMap<&str, &TdModel>,
    out: &mut HashMap<String, String>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::AnnAssign(ann) => {
                if let Some(name) = expr_name(&ann.target) {
                    let ty = ann_str(&ann.annotation);
                    if map.contains_key(ty.as_str()) {
                        let _ = out.insert(name.to_owned(), ty);
                    }
                }
            }
            Stmt::FunctionDef(func) => gather_var_typeddicts(&func.body, map, out),
            Stmt::ClassDef(cls) => gather_var_typeddicts(&cls.body, map, out),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Statement traversal
// ---------------------------------------------------------------------------

fn check_stmts(stmts: &[Stmt], ctx: &Ctx<'_>, diag: &mut Vec<Diagnostic>) {
    for stmt in stmts {
        match stmt {
            Stmt::AnnAssign(ann) => check_ann_assign(ann, ctx, diag),
            Stmt::Assign(assign) => check_assign(assign, ctx, diag),
            Stmt::Expr(expr_stmt) => check_call(&expr_stmt.value, ctx, diag),
            Stmt::FunctionDef(func) => check_stmts(&func.body, ctx, diag),
            Stmt::ClassDef(cls) => check_stmts(&cls.body, ctx, diag),
            _ => {}
        }
    }
}

/// `target: TD = {...}` (dict literal) or `target: TD = other_td_var`.
fn check_ann_assign(ann: &ast::StmtAnnAssign, ctx: &Ctx<'_>, diag: &mut Vec<Diagnostic>) {
    let Some(value) = &ann.value else {
        return;
    };
    let target_td = ann_str(&ann.annotation);
    if !ctx.map.contains_key(target_td.as_str()) {
        return;
    }
    match value.as_ref() {
        Expr::Dict(dict) => check_dict_literal(&target_td, dict, ctx, diag),
        Expr::Name(_) => check_value_source(&target_td, value, ctx, diag),
        _ => {}
    }
}

/// `target = source` where both sides are previously annotated `TypedDict`s.
fn check_assign(assign: &ast::StmtAssign, ctx: &Ctx<'_>, diag: &mut Vec<Diagnostic>) {
    if assign.targets.len() != 1 {
        return;
    }
    let Some(target_name) = assign.targets.first().and_then(expr_name) else {
        return;
    };
    let Some(target_td) = ctx.var_td.get(target_name) else {
        return;
    };
    check_value_source(target_td, &assign.value, ctx, diag);
}

/// Emit a TD-to-TD assignability error when `value` is a TypedDict-typed var.
fn check_value_source(target_td: &str, value: &Expr, ctx: &Ctx<'_>, diag: &mut Vec<Diagnostic>) {
    let Some(src_name) = expr_name(value) else {
        return;
    };
    let Some(src_td) = ctx.var_td.get(src_name) else {
        return;
    };
    if let Some(message) = td_assign_error(src_td, target_td, ctx.map) {
        push(diag, message, mk_span(value.range()), ctx.path);
    }
}

/// Each dict-literal key outside the target schema must match `extra_items`.
fn check_dict_literal(
    target_td: &str,
    dict: &ast::ExprDict,
    ctx: &Ctx<'_>,
    diag: &mut Vec<Diagnostic>,
) {
    let fields = transitive_fields(target_td, ctx.map);
    for item in &dict.items {
        let Some(key) = dict_string_key(item.key.as_ref()) else {
            continue;
        };
        if fields.iter().any(|f| f.name == key) {
            continue;
        }
        let Some(value_ty) = infer_expr_literal_type(&item.value) else {
            continue;
        };
        if let Some(message) = dict_extra_value_error(target_td, value_ty, ctx.map) {
            push(diag, message, mk_span(item.value.range()), ctx.path);
            return;
        }
    }
}

fn dict_string_key(key: Option<&Expr>) -> Option<String> {
    match key? {
        Expr::StringLiteral(s) => Some(s.value.to_str().to_owned()),
        _ => None,
    }
}

/// `TDName(key=value, ...)` constructor calls: keys outside the schema must be
/// permitted by `extra_items`.
fn check_call(expr: &Expr, ctx: &Ctx<'_>, diag: &mut Vec<Diagnostic>) {
    let Expr::Call(call) = expr else {
        return;
    };
    let Some(callee) = expr_name(&call.func) else {
        return;
    };
    if !ctx.map.contains_key(callee) {
        return;
    }
    let fields = transitive_fields(callee, ctx.map);
    for kw in &call.arguments.keywords {
        let Some(key) = kw.arg.as_ref().map(ast::Identifier::as_str) else {
            continue;
        };
        if fields.iter().any(|f| f.name == key) {
            continue;
        }
        let value_ty = infer_expr_literal_type(&kw.value);
        if let Some(message) = construction_extra_error(callee, key, value_ty, ctx.map) {
            push(diag, message, mk_span(call.range()), ctx.path);
            return;
        }
    }
}
