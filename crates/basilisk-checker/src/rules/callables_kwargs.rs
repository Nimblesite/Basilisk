//! Implements [`callables_kwargs`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `callables_kwargs`: Unpack[`TypedDict`] kwargs violations.
//!
//! Detects invalid uses of `**kwargs: Unpack[TypedDict]` in function signatures:
//! parameter overlap with `TypedDict` keys, `Unpack[TypeVar]` (invalid), and
//! call-site validation for functions with Unpack kwargs.

use std::collections::HashMap;

use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::{ann_str, expr_name};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "callables_kwargs",
    docs_url: "https://www.basilisk-python.dev/errors/callables_kwargs",
};

/// Emits `callables_kwargs` for Unpack[`TypedDict`] kwargs violations.
pub(crate) struct UnpackKwargsViolation;

impl Rule for UnpackKwargsViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };
        let ctx = KwargsContext::from_ast(&parsed.ast.body);
        check_stmts_recursive(&parsed.ast.body, &ctx, None, &module.path, diagnostics);
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
    /// `true` when the `TypedDict` declares `extra_items=` (PEP 728).
    td_extra_items: bool,
}

/// A `TypedDict` definition: all keys (including inherited) + PEP 728 flag.
struct TypedDictInfo {
    keys: Vec<String>,
    extra_items: bool,
}

/// Immutable module-level facts, collected once per module. Never cloned during
/// traversal — per-scope state lives in the [`VarScope`] chain instead.
struct KwargsContext {
    /// `TypedDict` name → definition. First definition wins on name collisions,
    /// matching the previous first-match linear lookup.
    typeddicts: HashMap<String, TypedDictInfo>,
    typevar_names: Vec<String>,
    /// Functions with Unpack kwargs: name → info.
    unpack_funcs: HashMap<String, UnpackFuncInfo>,
}

/// Per-scope variable annotations, chained to the enclosing scope. Lookups walk
/// outward so inner scopes shadow outer ones without copying any map — this
/// keeps traversal O(statements), not O(statements × module size).
struct VarScope<'a> {
    parent: Option<&'a VarScope<'a>>,
    /// Variable annotations in this scope: name → annotation text.
    vars: HashMap<String, String>,
}

impl VarScope<'_> {
    fn lookup(&self, name: &str) -> Option<&str> {
        self.vars
            .get(name)
            .map(String::as_str)
            .or_else(|| self.parent.and_then(|p| p.lookup(name)))
    }
}

impl KwargsContext {
    fn from_ast(stmts: &[Stmt]) -> Self {
        let mut typeddicts: HashMap<String, TypedDictInfo> = HashMap::new();
        let mut typevar_names = Vec::new();
        let mut unpack_funcs = HashMap::new();
        for stmt in stmts {
            match stmt {
                Stmt::ClassDef(cls) => {
                    collect_typeddict(cls, &mut typeddicts);
                }
                Stmt::Assign(assign) => {
                    collect_typevar(assign, &mut typevar_names);
                }
                Stmt::FunctionDef(func) => {
                    collect_unpack_func(func, &typeddicts, &mut unpack_funcs);
                }
                _ => {}
            }
        }
        Self {
            typeddicts,
            typevar_names,
            unpack_funcs,
        }
    }

    fn get_td_keys(&self, name: &str) -> Option<&[String]> {
        self.typeddicts.get(name).map(|td| td.keys.as_slice())
    }

    fn is_typevar(&self, name: &str) -> bool {
        self.typevar_names.iter().any(|n| n == name)
    }
}

// ---------------------------------------------------------------------------
// Context collection helpers
// ---------------------------------------------------------------------------

fn collect_typeddict(cls: &ast::StmtClassDef, typeddicts: &mut HashMap<String, TypedDictInfo>) {
    if !is_typeddict(cls, typeddicts) {
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
    let mut all_keys = collect_base_td_keys(cls, typeddicts);
    all_keys.extend(keys);
    let has_extra_items = typeddict_has_extra_items(cls, typeddicts);
    // First definition wins, matching the previous first-match linear lookup.
    let _ = typeddicts
        .entry(cls.name.to_string())
        .or_insert(TypedDictInfo {
            keys: all_keys,
            extra_items: has_extra_items,
        });
}

fn collect_base_td_keys(
    cls: &ast::StmtClassDef,
    typeddicts: &HashMap<String, TypedDictInfo>,
) -> Vec<String> {
    let mut result = Vec::new();
    if let Some(args) = &cls.arguments {
        for base in &args.args {
            if let Expr::Name(n) = base {
                if let Some(td) = typeddicts.get(n.id.as_str()) {
                    result.extend(td.keys.iter().cloned());
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
    typeddicts: &HashMap<String, TypedDictInfo>,
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
    if let Some(td) = typeddicts.get(unpack_type) {
        let positional_count = func.parameters.posonlyargs.len() + func.parameters.args.len();
        let _ = unpack_funcs.insert(
            func.name.to_string(),
            UnpackFuncInfo {
                td_name: unpack_type.to_owned(),
                td_keys: td.keys.clone(),
                positional_count,
                td_extra_items: td.extra_items,
            },
        );
    }
}

// ---------------------------------------------------------------------------
// Statement traversal
// ---------------------------------------------------------------------------

fn check_stmts_recursive(
    stmts: &[Stmt],
    ctx: &KwargsContext,
    parent: Option<&VarScope<'_>>,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    // Collect this block's annotated variables up front (hoisted, as before);
    // assignments add entries as the walk reaches them.
    let mut scope = VarScope {
        parent,
        vars: HashMap::new(),
    };
    for stmt in stmts {
        if let Stmt::AnnAssign(ann) = stmt {
            if let Some(name) = expr_name(&ann.target) {
                let _ = scope.vars.insert(name.to_owned(), ann_str(&ann.annotation));
            }
        }
    }
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDef(func) => {
                check_function_def(func, ctx, path, diag);
                check_stmts_recursive(&func.body, ctx, Some(&scope), path, diag);
            }
            Stmt::ClassDef(cls) => {
                check_stmts_recursive(&cls.body, ctx, Some(&scope), path, diag);
            }
            Stmt::Expr(expr_stmt) => {
                check_call_in_expr(&expr_stmt.value, ctx, &scope, path, diag);
            }
            Stmt::Assign(assign) => {
                collect_local_var_type(assign, ctx, &mut scope);
                check_call_in_expr(&assign.value, ctx, &scope, path, diag);
            }
            _ => {}
        }
    }
}

/// Track variable types from assignments like `td2 = TD2(v1=2, v3="4")`.
fn collect_local_var_type(assign: &ast::StmtAssign, ctx: &KwargsContext, scope: &mut VarScope<'_>) {
    if assign.targets.len() != 1 {
        return;
    }
    let Some(var_name) = assign.targets.first().and_then(expr_name) else {
        return;
    };
    // If RHS is a call to a known TypedDict constructor, track the variable type
    if let Expr::Call(call) = assign.value.as_ref() {
        if let Some(callee) = expr_name(&call.func) {
            if ctx.typeddicts.contains_key(callee) {
                let _ = scope.vars.insert(var_name.to_owned(), callee.to_owned());
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
        diag.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Invalid `**kwargs: Unpack[{unpack_type}]`: `{unpack_type}` is a TypeVar, not a TypedDict"
            ),
            func_span,
            path,
            Some("Use a concrete TypedDict type with Unpack".to_owned()),
            None,
        ));
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
            diag.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Parameter `{pname}` overlaps with TypedDict `{unpack_type}` key `{pname}`"
                ),
                span,
                path,
                Some(format!("Make `{pname}` positional-only (add `/`)")),
                None,
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Call-site checks
// ---------------------------------------------------------------------------

fn check_call_in_expr(
    expr: &Expr,
    ctx: &KwargsContext,
    scope: &VarScope<'_>,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
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
    check_dict_spread(call, info, func_name, scope, path, diag, call_span);
    check_duplicate_keywords(call, func_name, ctx, scope, path, diag, call_span);
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
        diag.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Cannot pass positional arguments to `{func_name}` for Unpack[{}] kwargs",
                info.td_name
            ),
            span,
            path,
            Some("Pass keyword arguments instead".to_owned()),
            None,
        ));
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
            diag.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Call to `{func_name}` missing required keyword arguments from `{}`",
                    info.td_name
                ),
                span,
                path,
                None,
                None,
            ));
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
    // PEP 728: `extra_items=` TypedDicts accept keys beyond their schema.
    if info.td_extra_items {
        return;
    }
    for kw in &call.arguments.keywords {
        if let Some(arg_name) = &kw.arg {
            if !info.td_keys.iter().any(|k| k == arg_name.as_str()) {
                diag.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Keyword `{arg_name}` in call to `{func_name}` is not a key of TypedDict `{}`",
                        info.td_name
                    ),
                    span,
                    path,
                    None,
                    None,
                ));
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
    scope: &VarScope<'_>,
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
        if let Some(ann) = scope.lookup(var_name) {
            if ann.starts_with("dict[") || ann == "dict" {
                diag.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Cannot spread `{var_name}: {ann}` into `{func_name}` with Unpack[{}] kwargs",
                        info.td_name
                    ),
                    span,
                    path,
                    Some("Use a TypedDict instead of a plain dict".to_owned()),
                    None,
                ));
            }
        }
    }
}

/// Check for duplicate keywords between explicit kwargs and spread.
fn check_duplicate_keywords(
    call: &ast::ExprCall,
    func_name: &str,
    ctx: &KwargsContext,
    scope: &VarScope<'_>,
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
        let spread_keys = resolve_spread_keys(var_name, ctx, scope);
        for ek in &explicit_kw_names {
            if spread_keys.iter().any(|sk| sk == ek) {
                diag.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Keyword `{ek}` in call to `{func_name}` conflicts with `**{var_name}`"
                    ),
                    span,
                    path,
                    None,
                    None,
                ));
                return;
            }
        }
    }
}

/// Resolve the keys a spread variable would provide.
fn resolve_spread_keys<'a>(
    var_name: &str,
    ctx: &'a KwargsContext,
    scope: &VarScope<'_>,
) -> Vec<&'a str> {
    // Check if the variable's annotation type is a known TypedDict
    if let Some(ann) = scope.lookup(var_name) {
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

fn is_typeddict(cls: &ast::StmtClassDef, known_tds: &HashMap<String, TypedDictInfo>) -> bool {
    cls.arguments.as_ref().is_some_and(|args| {
        args.args.iter().any(|a| {
            if let Expr::Name(n) = a {
                let name = n.id.as_str();
                name == "TypedDict" || known_tds.contains_key(name)
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

/// `true` when a `TypedDict` class declares `extra_items=` directly or
/// inherits it from a base `TypedDict` (PEP 728).
fn typeddict_has_extra_items(
    cls: &ast::StmtClassDef,
    typeddicts: &HashMap<String, TypedDictInfo>,
) -> bool {
    let Some(args) = &cls.arguments else {
        return false;
    };
    let direct = args
        .keywords
        .iter()
        .any(|kw| kw.arg.as_ref().is_some_and(|a| a.as_str() == "extra_items"));
    direct
        || args.args.iter().any(|base| {
            matches!(
                base,
                Expr::Name(n) if typeddicts.get(n.id.as_str()).is_some_and(|td| td.extra_items)
            )
        })
}
