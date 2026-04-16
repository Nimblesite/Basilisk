//! BSK-E0141: Unpack[`TypedDict`] kwargs violations.
//!
//! Detects invalid uses of `**kwargs: Unpack[TypedDict]` in function signatures:
//! parameter overlap with `TypedDict` keys, `Unpack[TypeVar]` (invalid), and
//! call-site validation for functions with Unpack kwargs.

use std::collections::HashMap;

use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};
use crate::rules::shared::{ann_str, expr_name};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0141",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0141",
};

/// Emits BSK-E0141 for Unpack[`TypedDict`] kwargs violations.
pub(crate) struct UnpackKwargsViolation;

impl Rule for UnpackKwargsViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let Ok(parsed) = basilisk_parser::parse_source(module.source.clone(), module.path.clone())
        else {
            return;
        };
        let ctx = KwargsContext::from_ast(&parsed.ast.body);
        check_stmts_recursive(&parsed.ast.body, &ctx, &module.path, diagnostics);
    }
}

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

/// Info about a function with Unpack kwargs.
#[derive(Clone)]
struct UnpackFuncInfo {
    /// `TypedDict` name used in `Unpack`.
    td_name: String,
    /// All keys from the `TypedDict`.
    td_keys: Vec<String>,
    /// Number of explicit positional parameters.
    positional_count: usize,
}

struct KwargsContext {
    typeddict_keys: Vec<(String, Vec<String>)>,
    typevar_names: Vec<String>,
    /// Functions with Unpack kwargs: name → info.
    unpack_funcs: HashMap<String, UnpackFuncInfo>,
    /// Variable annotations: name → annotation text.
    var_annotations: HashMap<String, String>,
}

impl KwargsContext {
    fn from_ast(stmts: &[Stmt]) -> Self {
        let mut typeddict_keys: Vec<(String, Vec<String>)> = Vec::new();
        let mut typevar_names = Vec::new();
        let mut unpack_funcs = HashMap::new();
        let mut var_annotations = HashMap::new();
        for stmt in stmts {
            match stmt {
                Stmt::ClassDef(cls) => {
                    collect_typeddict(cls, &mut typeddict_keys);
                }
                Stmt::Assign(assign) => {
                    collect_typevar(assign, &mut typevar_names);
                }
                Stmt::FunctionDef(func) => {
                    collect_unpack_func(func, &typeddict_keys, &mut unpack_funcs);
                }
                Stmt::AnnAssign(ann) => {
                    collect_var_annotation(ann, &mut var_annotations);
                }
                _ => {}
            }
        }
        Self {
            typeddict_keys,
            typevar_names,
            unpack_funcs,
            var_annotations,
        }
    }

    /// Clone context and add local annotations/variable types from a statement block.
    fn clone_with_locals(&self, stmts: &[Stmt]) -> Self {
        let mut ctx = Self {
            typeddict_keys: self.typeddict_keys.clone(),
            typevar_names: self.typevar_names.clone(),
            unpack_funcs: self.unpack_funcs.clone(),
            var_annotations: self.var_annotations.clone(),
        };
        for stmt in stmts {
            if let Stmt::AnnAssign(ann) = stmt {
                if let Some(name) = expr_name(&ann.target) {
                    let _ = ctx
                        .var_annotations
                        .insert(name.to_owned(), ann_str(&ann.annotation));
                }
            }
        }
        ctx
    }

    fn get_td_keys(&self, name: &str) -> Option<&[String]> {
        self.typeddict_keys
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, k)| k.as_slice())
    }

    fn is_typevar(&self, name: &str) -> bool {
        self.typevar_names.iter().any(|n| n == name)
    }
}

// ---------------------------------------------------------------------------
// Context collection helpers
// ---------------------------------------------------------------------------

fn collect_typeddict(cls: &ast::StmtClassDef, typeddict_keys: &mut Vec<(String, Vec<String>)>) {
    if !is_typeddict(cls, typeddict_keys) {
        return;
    }
    let keys: Vec<String> = cls
        .body
        .iter()
        .filter_map(|s| {
            if let Stmt::AnnAssign(ann) = s {
                expr_name(&ann.target).map(std::borrow::ToOwned::to_owned)
            } else {
                None
            }
        })
        .collect();
    let mut all_keys = collect_base_td_keys(cls, typeddict_keys);
    all_keys.extend(keys);
    typeddict_keys.push((cls.name.to_string(), all_keys));
}

fn collect_base_td_keys(
    cls: &ast::StmtClassDef,
    typeddict_keys: &[(String, Vec<String>)],
) -> Vec<String> {
    let mut result = Vec::new();
    if let Some(args) = &cls.arguments {
        for base in &args.args {
            if let Expr::Name(n) = base {
                if let Some((_, bkeys)) = typeddict_keys.iter().find(|(n2, _)| n2 == n.id.as_str())
                {
                    result.extend(bkeys.iter().cloned());
                }
            }
        }
    }
    result
}

fn collect_typevar(assign: &ast::StmtAssign, typevar_names: &mut Vec<String>) {
    if assign.targets.len() != 1 {
        return;
    }
    if let Some(name) = assign.targets.first().and_then(expr_name) {
        if is_typevar_call(&assign.value) {
            typevar_names.push(name.to_owned());
        }
    }
}

fn collect_unpack_func(
    func: &ast::StmtFunctionDef,
    typeddict_keys: &[(String, Vec<String>)],
    unpack_funcs: &mut HashMap<String, UnpackFuncInfo>,
) {
    let Some(kwarg) = &func.parameters.kwarg else {
        return;
    };
    let Some(annotation) = &kwarg.annotation else {
        return;
    };
    let Some(unpack_type) = extract_unpack_arg(annotation) else {
        return;
    };
    if let Some((_, keys)) = typeddict_keys.iter().find(|(n, _)| n == unpack_type) {
        let positional_count = func.parameters.posonlyargs.len() + func.parameters.args.len();
        let _ = unpack_funcs.insert(
            func.name.to_string(),
            UnpackFuncInfo {
                td_name: unpack_type.to_owned(),
                td_keys: keys.clone(),
                positional_count,
            },
        );
    }
}

fn collect_var_annotation(ann: &ast::StmtAnnAssign, var_annotations: &mut HashMap<String, String>) {
    if let Some(name) = expr_name(&ann.target) {
        let _ = var_annotations.insert(name.to_owned(), ann_str(&ann.annotation));
    }
}

// ---------------------------------------------------------------------------
// Statement traversal
// ---------------------------------------------------------------------------

fn check_stmts_recursive(
    stmts: &[Stmt],
    ctx: &KwargsContext,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    // Collect local annotations and variable types for this scope
    let mut local_ctx = ctx.clone_with_locals(stmts);
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDef(func) => {
                check_function_def(func, &local_ctx, path, diag);
                check_stmts_recursive(&func.body, &local_ctx, path, diag);
            }
            Stmt::ClassDef(cls) => check_stmts_recursive(&cls.body, &local_ctx, path, diag),
            Stmt::Expr(expr_stmt) => check_call_in_expr(&expr_stmt.value, &local_ctx, path, diag),
            Stmt::Assign(assign) => {
                collect_local_var_type(assign, &mut local_ctx);
                check_call_in_expr(&assign.value, &local_ctx, path, diag);
            }
            Stmt::AnnAssign(ann) => {
                if let Some(name) = expr_name(&ann.target) {
                    let _ = local_ctx
                        .var_annotations
                        .insert(name.to_owned(), ann_str(&ann.annotation));
                }
            }
            _ => {}
        }
    }
}

/// Track variable types from assignments like `td2 = TD2(v1=2, v3="4")`.
fn collect_local_var_type(assign: &ast::StmtAssign, ctx: &mut KwargsContext) {
    if assign.targets.len() != 1 {
        return;
    }
    let Some(var_name) = assign.targets.first().and_then(expr_name) else {
        return;
    };
    // If RHS is a call to a known TypedDict constructor, track the variable type
    if let Expr::Call(call) = assign.value.as_ref() {
        if let Some(callee) = expr_name(&call.func) {
            if ctx.typeddict_keys.iter().any(|(n, _)| n == callee) {
                let _ = ctx
                    .var_annotations
                    .insert(var_name.to_owned(), callee.to_owned());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Function definition checks
// ---------------------------------------------------------------------------

fn check_function_def(
    func: &ast::StmtFunctionDef,
    ctx: &KwargsContext,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    let Some(kwarg) = &func.parameters.kwarg else {
        return;
    };
    let Some(annotation) = &kwarg.annotation else {
        return;
    };
    let Some(unpack_type) = extract_unpack_arg(annotation) else {
        return;
    };
    let func_span = mk_span(func.range());
    if ctx.is_typevar(unpack_type) {
        diag.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Invalid `**kwargs: Unpack[{unpack_type}]`: `{unpack_type}` is a TypeVar, not a TypedDict"
            ),
            span: func_span,
            path: path.to_owned(),
            help: Some("Use a concrete TypedDict type with Unpack".to_owned()),
            note: None,
        provenance: None,
        });
        return;
    }
    check_param_overlap(func, unpack_type, ctx, path, diag, func_span);
}

fn check_param_overlap(
    func: &ast::StmtFunctionDef,
    unpack_type: &str,
    ctx: &KwargsContext,
    path: &str,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) {
    let Some(td_keys) = ctx.get_td_keys(unpack_type) else {
        return;
    };
    for param in &func.parameters.args {
        let pname = param.parameter.name.as_str();
        if td_keys.iter().any(|k| k == pname) {
            diag.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Parameter `{pname}` overlaps with TypedDict `{unpack_type}` key `{pname}`"
                ),
                span,
                path: path.to_owned(),
                help: Some(format!("Make `{pname}` positional-only (add `/`)")),
                note: None,
                provenance: None,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Call-site checks
// ---------------------------------------------------------------------------

fn check_call_in_expr(expr: &Expr, ctx: &KwargsContext, path: &str, diag: &mut Vec<Diagnostic>) {
    let Expr::Call(call) = expr else {
        return;
    };
    let Some(func_name) = expr_name(&call.func) else {
        return;
    };
    let Some(info) = ctx.unpack_funcs.get(func_name) else {
        return;
    };
    let call_span = mk_span(call.range());
    check_positional_args(call, info, func_name, path, diag, call_span);
    check_missing_required(call, info, func_name, ctx, path, diag, call_span);
    check_extra_keywords(call, info, func_name, path, diag, call_span);
    check_dict_spread(call, info, func_name, ctx, path, diag, call_span);
    check_duplicate_keywords(call, func_name, ctx, path, diag, call_span);
}

/// Positional args beyond the explicit params are not allowed with Unpack kwargs.
fn check_positional_args(
    call: &ast::ExprCall,
    info: &UnpackFuncInfo,
    func_name: &str,
    path: &str,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) {
    if call.arguments.args.len() > info.positional_count {
        diag.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Cannot pass positional arguments to `{func_name}` for Unpack[{}] kwargs",
                info.td_name
            ),
            span,
            path: path.to_owned(),
            help: Some("Pass keyword arguments instead".to_owned()),
            note: None,
            provenance: None,
        });
    }
}

/// Check that all required `TypedDict` keys are provided as keyword args.
fn check_missing_required(
    call: &ast::ExprCall,
    info: &UnpackFuncInfo,
    func_name: &str,
    ctx: &KwargsContext,
    path: &str,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) {
    // If there's a ** spread, we can't statically check missing keys easily
    if call.arguments.keywords.iter().any(|kw| kw.arg.is_none()) {
        return;
    }
    if call.arguments.keywords.is_empty() && call.arguments.args.len() <= info.positional_count {
        // No kwargs at all — check if TD has required keys
        let td_has_required = has_required_td_keys(&info.td_name, ctx);
        if td_has_required {
            diag.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Call to `{func_name}` missing required keyword arguments from `{}`",
                    info.td_name
                ),
                span,
                path: path.to_owned(),
                help: None,
                note: None,
                provenance: None,
            });
        }
    }
}

/// Check for keyword args not in the `TypedDict`.
fn check_extra_keywords(
    call: &ast::ExprCall,
    info: &UnpackFuncInfo,
    func_name: &str,
    path: &str,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) {
    for kw in &call.arguments.keywords {
        if let Some(arg_name) = &kw.arg {
            if !info.td_keys.iter().any(|k| k == arg_name.as_str()) {
                diag.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Keyword `{arg_name}` in call to `{func_name}` is not a key of TypedDict `{}`",
                        info.td_name
                    ),
                    span,
                    path: path.to_owned(),
                    help: None,
                    note: None,
            provenance: None,
                });
                return;
            }
        }
    }
}

/// Check for dict spread (untyped dict) into Unpack kwargs.
fn check_dict_spread(
    call: &ast::ExprCall,
    info: &UnpackFuncInfo,
    func_name: &str,
    ctx: &KwargsContext,
    path: &str,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) {
    for kw in &call.arguments.keywords {
        if kw.arg.is_some() {
            continue;
        }
        // ** spread: check if the value is a known TypedDict variable or a dict
        let Some(var_name) = expr_name(&kw.value) else {
            continue;
        };
        if let Some(ann) = ctx.var_annotations.get(var_name) {
            if ann.starts_with("dict[") || ann == "dict" {
                diag.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Cannot spread `{var_name}: {ann}` into `{func_name}` with Unpack[{}] kwargs",
                        info.td_name
                    ),
                    span,
                    path: path.to_owned(),
                    help: Some("Use a TypedDict instead of a plain dict".to_owned()),
                    note: None,
            provenance: None,
                });
            }
        }
    }
}

/// Check for duplicate keywords between explicit kwargs and spread.
fn check_duplicate_keywords(
    call: &ast::ExprCall,
    func_name: &str,
    ctx: &KwargsContext,
    path: &str,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) {
    let explicit_kw_names: Vec<&str> = call
        .arguments
        .keywords
        .iter()
        .filter_map(|kw| kw.arg.as_ref().map(ast::Identifier::as_str))
        .collect();
    if explicit_kw_names.is_empty() {
        return;
    }
    for kw in &call.arguments.keywords {
        if kw.arg.is_some() {
            continue;
        }
        let Some(var_name) = expr_name(&kw.value) else {
            continue;
        };
        let spread_keys = resolve_spread_keys(var_name, ctx);
        for ek in &explicit_kw_names {
            if spread_keys.iter().any(|sk| sk == ek) {
                diag.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Keyword `{ek}` in call to `{func_name}` conflicts with `**{var_name}`"
                    ),
                    span,
                    path: path.to_owned(),
                    help: None,
                    note: None,
                    provenance: None,
                });
                return;
            }
        }
    }
}

/// Resolve the keys a spread variable would provide.
fn resolve_spread_keys<'a>(var_name: &str, ctx: &'a KwargsContext) -> Vec<&'a str> {
    // Check if the variable's annotation type is a known TypedDict
    if let Some(ann) = ctx.var_annotations.get(var_name) {
        if let Some(keys) = ctx.get_td_keys(ann) {
            return keys.iter().map(String::as_str).collect();
        }
    }
    // Check directly as a TypedDict name
    if let Some(keys) = ctx.get_td_keys(var_name) {
        return keys.iter().map(String::as_str).collect();
    }
    Vec::new()
}

/// Check if a `TypedDict` has any required keys (all keys are considered required
/// unless wrapped in `NotRequired`, but we simplify to: if there are keys, there
/// are likely required ones).
fn has_required_td_keys(td_name: &str, ctx: &KwargsContext) -> bool {
    ctx.get_td_keys(td_name)
        .is_some_and(|keys| !keys.is_empty())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_typeddict(cls: &ast::StmtClassDef, known_tds: &[(String, Vec<String>)]) -> bool {
    cls.arguments.as_ref().is_some_and(|args| {
        args.args.iter().any(|a| {
            if let Expr::Name(n) = a {
                let name = n.id.as_str();
                name == "TypedDict" || known_tds.iter().any(|(td, _)| td == name)
            } else {
                false
            }
        })
    })
}

fn is_typevar_call(expr: &Expr) -> bool {
    matches!(expr, Expr::Call(call) if matches!(call.func.as_ref(), Expr::Name(n) if n.id.as_str() == "TypeVar"))
}

fn extract_unpack_arg(expr: &Expr) -> Option<&str> {
    if let Expr::Subscript(sub) = expr {
        if matches!(sub.value.as_ref(), Expr::Name(n) if n.id.as_str() == "Unpack") {
            return expr_name(&sub.slice);
        }
    }
    None
}

fn mk_span(range: ruff_text_size::TextRange) -> Span {
    Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    }
}
