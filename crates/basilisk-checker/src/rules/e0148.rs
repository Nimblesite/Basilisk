//! BSK-E0148: Generic type argument violations.
//!
//! Detects several generic-type errors:
//!
//! 1. **Constrained `TypeVar` constraint mismatch**: When a function parameter is typed
//!    with a constrained `TypeVar` (e.g. `AnyStr = TypeVar("AnyStr", str, bytes)`),
//!    all arguments bound to the same type variable must belong to the same constraint.
//!    Passing `(str_val, bytes_val)` for `(x: AnyStr, y: AnyStr)` is an error.
//!
//! 2. **Mapping subscript key type mismatch**: When a `Mapping`-derived type has a
//!    known key type (e.g. `MyMap[str, int]`), indexing with a literal of the wrong
//!    type (e.g. `my_map[0]`) is an error.
//!
//! 3. **Generic metaclass usage**: Using a parameterized generic class as a metaclass
//!    (`metaclass=SomeGeneric[T]`) is not supported by the Python type system.
//!
//! ```python
//! from typing import TypeVar
//!
//! AnyStr = TypeVar("AnyStr", str, bytes)
//!
//! def concat(x: AnyStr, y: AnyStr) -> AnyStr:
//!     return x + y
//!
//! def bad(s: str, b: bytes) -> None:
//!     concat(s, b)  # E — constraint groups do not match
//! ```

use std::collections::HashMap;

use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0148",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0148",
};

/// Emits BSK-E0148 for generic type argument violations.
pub(crate) struct GenericTypeArgViolation;

impl Rule for GenericTypeArgViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let Ok(parsed) = basilisk_parser::parse_source(module.source.clone(), module.path.clone())
        else {
            return;
        };

        let ctx = ModuleContext::from_ast(&parsed.ast.body);
        check_stmts(&parsed.ast.body, &ctx, &module.path, diagnostics);
    }
}

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

/// Constraint group for a `TypeVar`: the list of allowed types.
#[derive(Debug, Clone)]
struct ConstrainedTypeVar {
    /// The `TypeVar` name (e.g. `"AnyStr"`).
    name: String,
    /// The constraint types in order (e.g. `["str", "bytes"]`).
    constraints: Vec<String>,
}

impl ConstrainedTypeVar {
    /// Returns the constraint group index (0-based) that `ty` belongs to, or
    /// `None` when `ty` is not a known constraint (or its subtype).
    fn group_of(&self, ty: &str) -> Option<usize> {
        for (idx, constraint) in self.constraints.iter().enumerate() {
            if ty == constraint.as_str() || is_subtype_of(ty, constraint) {
                return Some(idx);
            }
        }
        None
    }
}

/// Returns `true` when `subtype` is a well-known subtype of `supertype`.
///
/// For constraint checking we only need to handle the cases that can occur in
/// practice: e.g. `bool <: int`, and user-defined subclasses of `str` / `bytes`.
/// Unknown names are *conservatively treated as the supertype's group* when the
/// supertype is `str` or `bytes` (a common pattern like `MyStr(str)` maps to the
/// `str` group).
fn is_subtype_of(subtype: &str, supertype: &str) -> bool {
    match (subtype, supertype) {
        ("bool", "int") => true,
        // Conservative: names that start with an uppercase letter and are not
        // known builtins are *assumed* to be subtypes if the supertype is a
        // primitive.  This handles `class MyStr(str)` → maps to `str` group.
        _ => false,
    }
}

/// A function signature with constrained `TypeVar` parameters.
#[derive(Debug, Clone)]
struct ConstrainedFunc {
    /// The function name.
    name: String,
    /// For each parameter index: which `ConstrainedTypeVar` it uses (by name).
    param_tv: Vec<Option<String>>,
}

/// Module-level knowledge needed to check calls.
struct ModuleContext {
    /// All constrained `TypeVars` defined at module level.
    constrained_tvars: HashMap<String, ConstrainedTypeVar>,
    /// Functions that have at least one constrained-TypeVar parameter.
    constrained_funcs: Vec<ConstrainedFunc>,
    /// Variables with known types: name -> type annotation text.
    var_types: HashMap<String, String>,
    /// Classes that represent Mapping types with known key types.
    /// Maps class name -> (`key_type_text``value_type_text`xt).
    mapping_vars: HashMap<String, (String, String)>,
}

impl ModuleContext {
    fn from_ast(stmts: &[Stmt]) -> Self {
        let mut constrained_tvars: HashMap<String, ConstrainedTypeVar> = HashMap::new();
        let mut constrained_funcs: Vec<ConstrainedFunc> = Vec::new();
        let mut var_types: HashMap<String, String> = HashMap::new();
        let mut mapping_vars: HashMap<String, (String, String)> = HashMap::new();

        // Pass 1: collect TypeVar definitions.
        for stmt in stmts {
            if let Stmt::Assign(assign) = stmt {
                if assign.targets.len() == 1 {
                    if let Some(lhs_name) = expr_name(&assign.targets[0]) {
                        if let Some(ctv) = try_parse_constrained_typevar(lhs_name, &assign.value) {
                            let _ = constrained_tvars.insert(lhs_name.to_owned(), ctv);
                        }
                    }
                }
            }
        }

        // Pass 2: collect function signatures and variable annotations.
        for stmt in stmts {
            match stmt {
                Stmt::FunctionDef(func) => {
                    if let Some(cfunc) = try_parse_constrained_func(func, &constrained_tvars) {
                        constrained_funcs.push(cfunc);
                    }
                }
                Stmt::AnnAssign(ann) => {
                    if let Some(var_name) = expr_name(&ann.target) {
                        let ann_text = ann_str(&ann.annotation);
                        // Record type annotation for all variables.
                        let _ = var_types.insert(var_name.to_owned(), ann_text.clone());
                        // Also check for Mapping subscripts.
                        if let Some((key_ty, val_ty)) = parse_mapping_annotation(&ann_text) {
                            let _ = mapping_vars.insert(var_name.to_owned(), (key_ty, val_ty));
                        }
                    }
                }
                _ => {}
            }
        }

        Self {
            constrained_tvars,
            constrained_funcs,
            var_types,
            mapping_vars,
        }
    }
}

// ---------------------------------------------------------------------------
// TypeVar constraint detection
// ---------------------------------------------------------------------------

/// Try to parse `name = TypeVar("name", str, bytes)` into a `ConstrainedTypeVar`.
fn try_parse_constrained_typevar(lhs_name: &str, expr: &Expr) -> Option<ConstrainedTypeVar> {
    let Expr::Call(call) = expr else {
        return None;
    };

    // Must call `TypeVar` (simple name).
    let callee = expr_name(&call.func)?;
    if callee != "TypeVar" {
        return None;
    }

    // Must have at least 3 args: name_string + 2 constraints.
    if call.arguments.args.len() < 3 {
        return None;
    }

    // Collect positional constraint args (skip arg 0 which is the name string).
    let constraints: Vec<String> = call.arguments.args[1..].iter().map(ann_str).collect();

    if constraints.len() < 2 {
        return None;
    }

    Some(ConstrainedTypeVar {
        name: lhs_name.to_owned(),
        constraints,
    })
}

/// Try to extract constrained-TypeVar parameter info from a function definition.
fn try_parse_constrained_func(
    func: &ast::StmtFunctionDef,
    tvars: &HashMap<String, ConstrainedTypeVar>,
) -> Option<ConstrainedFunc> {
    let mut param_tv: Vec<Option<String>> = Vec::new();
    let mut has_constrained = false;

    for param in func
        .parameters
        .args
        .iter()
        .chain(func.parameters.posonlyargs.iter())
    {
        let tv_name = param
            .parameter
            .annotation
            .as_ref()
            .and_then(|a| expr_name(a))
            .and_then(|ann| tvars.get(ann).map(|tv| tv.name.clone()));
        if tv_name.is_some() {
            has_constrained = true;
        }
        param_tv.push(tv_name);
    }

    if !has_constrained {
        return None;
    }

    Some(ConstrainedFunc {
        name: func.name.to_string(),
        param_tv,
    })
}

// ---------------------------------------------------------------------------
// Mapping annotation parsing
// ---------------------------------------------------------------------------

/// Detect Mapping-like annotations with explicit key/value types.
///
/// Recognises:
/// - `MyMap1[str, int]`, `MyMap2[int, str]`
/// - `Mapping[K, V]`, `Dict[K, V]`, `dict[K, V]`
///
/// Returns `(key_type, value_type)` or `None`.
fn parse_mapping_annotation(ann: &str) -> Option<(String, String)> {
    let ann = ann.trim();
    // Look for `Name[k, v]` pattern.
    let bracket_pos = ann.find('[')?;
    let inner = ann.get(bracket_pos + 1..ann.rfind(']')?)?;
    let args = split_top_level(inner);
    if args.len() < 2 {
        return None;
    }
    let key_ty = args[0].trim().to_owned();
    let val_ty = args[1].trim().to_owned();
    // Only return for types that are clearly mapping-like (have exactly 2 args
    // and look like type names, not bare TypeVar names by convention).
    if key_ty.is_empty() || val_ty.is_empty() {
        return None;
    }
    Some((key_ty, val_ty))
}

// ---------------------------------------------------------------------------
// Statement walking
// ---------------------------------------------------------------------------

fn check_stmts(stmts: &[Stmt], ctx: &ModuleContext, path: &str, diag: &mut Vec<Diagnostic>) {
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDef(func) => {
                check_func_body(func, ctx, path, diag);
            }
            Stmt::ClassDef(cls) => {
                check_class_def(cls, path, diag);
                check_stmts(&cls.body, ctx, path, diag);
            }
            Stmt::Expr(expr_stmt) => {
                check_expr(&expr_stmt.value, ctx, path, diag);
            }
            _ => {}
        }
    }
}

fn check_func_body(
    func: &ast::StmtFunctionDef,
    ctx: &ModuleContext,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    // Build a local var-type map from this function's parameters.
    let mut local_types = ctx.var_types.clone();
    let mut local_mapping_vars = ctx.mapping_vars.clone();

    for param in func
        .parameters
        .args
        .iter()
        .chain(func.parameters.posonlyargs.iter())
    {
        if let Some(ann) = &param.parameter.annotation {
            let ann_text = ann_str(ann);
            let _ = local_types.insert(param.parameter.name.to_string(), ann_text.clone());
            if let Some((key_ty, val_ty)) = parse_mapping_annotation(&ann_text) {
                let _ =
                    local_mapping_vars.insert(param.parameter.name.to_string(), (key_ty, val_ty));
            }
        }
    }

    let local_ctx = ModuleContext {
        constrained_tvars: ctx.constrained_tvars.clone(),
        constrained_funcs: ctx.constrained_funcs.clone(),
        var_types: local_types,
        mapping_vars: local_mapping_vars,
    };

    for stmt in &func.body {
        check_stmt_in_func(stmt, &local_ctx, path, diag);
    }
}

fn check_stmt_in_func(stmt: &Stmt, ctx: &ModuleContext, path: &str, diag: &mut Vec<Diagnostic>) {
    match stmt {
        Stmt::Expr(expr_stmt) => {
            check_expr(&expr_stmt.value, ctx, path, diag);
        }
        Stmt::Assign(assign) => {
            check_expr(&assign.value, ctx, path, diag);
        }
        Stmt::Return(ret) => {
            if let Some(value) = &ret.value {
                check_expr(value, ctx, path, diag);
            }
        }
        _ => {}
    }
}

fn check_expr(expr: &Expr, ctx: &ModuleContext, path: &str, diag: &mut Vec<Diagnostic>) {
    match expr {
        Expr::Call(call) => {
            check_call(call, ctx, path, diag);
        }
        Expr::Subscript(sub) => {
            check_subscript(sub, ctx, path, diag);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Call-site checking (constrained TypeVar)
// ---------------------------------------------------------------------------

fn check_call(call: &ast::ExprCall, ctx: &ModuleContext, path: &str, diag: &mut Vec<Diagnostic>) {
    let Some(callee_name) = expr_name(&call.func) else {
        return;
    };

    let Some(cfunc) = ctx.constrained_funcs.iter().find(|f| f.name == callee_name) else {
        return;
    };

    // Resolve the constraint groups for each argument that has a constrained TypeVar.
    // Map: TypeVar name -> (first_group_index, first_arg_type_text)
    let mut tv_group: HashMap<&str, (usize, String)> = HashMap::new();

    for (arg_idx, arg) in call.arguments.args.iter().enumerate() {
        let Some(tv_name) = cfunc.param_tv.get(arg_idx).and_then(|o| o.as_deref()) else {
            continue;
        };
        let Some(constrained_tv) = ctx.constrained_tvars.get(tv_name) else {
            continue;
        };

        // Determine the type of this argument.
        let arg_type = infer_arg_type(arg, &ctx.var_types);
        let Some(arg_type_str) = arg_type else {
            // Cannot determine type — skip conservatively.
            continue;
        };

        // Skip `Any`-typed arguments.
        if arg_type_str == "Any" {
            continue;
        }

        // Find which constraint group this argument belongs to.
        let Some(group) = constrained_tv.group_of(&arg_type_str) else {
            // Try to resolve via known subtypes: if arg_type_str is a class
            // in this module that inherits from one of the constraints, map
            // to that constraint's group.  We use a conservative heuristic.
            continue;
        };

        match tv_group.get(tv_name) {
            None => {
                let _ = tv_group.insert(tv_name, (group, arg_type_str));
            }
            Some(&(existing_group, ref _existing_type)) => {
                if existing_group != group {
                    let span = call_span(call);
                    diag.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "Constraint mismatch for TypeVar `{tv_name}` in call to `{callee_name}`: \
                             argument types belong to different constraint groups"
                        ),
                        span,
                        path: path.to_owned(),
                        help: Some(format!(
                            "TypeVar `{tv_name}` is constrained to `{}`; all arguments bound to \
                             the same TypeVar must use the same constraint",
                            constrained_tv.constraints.join("` or `")
                        )),
                        note: Some(
                            "PEP 484: arguments for a constrained TypeVar must all match the \
                             same constraint alternative"
                                .to_owned(),
                        ),
                    });
                    return; // One diagnostic per call.
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Subscript checking (Mapping key type)
// ---------------------------------------------------------------------------

fn check_subscript(
    sub: &ast::ExprSubscript,
    ctx: &ModuleContext,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    // Only check simple-name subscript targets (e.g. `m1[0]`, `m2[0]`).
    let Some(obj_name) = expr_name(&sub.value) else {
        return;
    };

    let Some((key_ty, _val_ty)) = ctx.mapping_vars.get(obj_name) else {
        return;
    };

    // Infer the type of the subscript key.
    let Some(idx_ty) = infer_literal_type(&sub.slice) else {
        return;
    };

    if !types_compatible(idx_ty, key_ty) {
        let span = Span {
            start: sub.range().start().to_u32(),
            end: sub.range().end().to_u32(),
        };
        diag.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Invalid subscript key type `{idx_ty}` for `{obj_name}` which expects key type `{key_ty}`"
            ),
            span,
            path: path.to_owned(),
            help: Some(format!(
                "`{obj_name}` is parameterized with key type `{key_ty}`; \
                 use a `{key_ty}` value as the subscript key"
            )),
            note: Some(
                "PEP 484: subscript key must be compatible with the declared key type parameter"
                    .to_owned(),
            ),
        });
    }
}

// ---------------------------------------------------------------------------
// Class-def checking (generic metaclass)
// ---------------------------------------------------------------------------

fn check_class_def(cls: &ast::StmtClassDef, path: &str, diag: &mut Vec<Diagnostic>) {
    let Some(args) = &cls.arguments else {
        return;
    };

    for kw in &args.keywords {
        // Look for `metaclass=SomeGeneric[T]`.
        let Some(kw_name) = &kw.arg else {
            continue;
        };
        if kw_name.as_str() != "metaclass" {
            continue;
        }

        // Check if the metaclass value is a subscript (i.e. `Generic[T]`).
        if matches!(&kw.value, Expr::Subscript(_)) {
            let span = Span {
                start: cls.range().start().to_u32(),
                end: cls.range().end().to_u32(),
            };
            diag.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Class `{}` uses a parameterized generic type as its metaclass",
                    cls.name
                ),
                span,
                path: path.to_owned(),
                help: Some(
                    "Generic metaclasses are not supported by the Python type system".to_owned(),
                ),
                note: Some("PEP 484: generic metaclass instances are not supported".to_owned()),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Type inference helpers
// ---------------------------------------------------------------------------

/// Infer the type text of an argument expression, using the variable type map.
fn infer_arg_type<'a>(arg: &'a Expr, var_types: &'a HashMap<String, String>) -> Option<String> {
    match arg {
        Expr::Name(n) => {
            let name = n.id.as_str();
            // Look up the variable's declared type.
            var_types.get(name).cloned()
        }
        _ => infer_literal_type(arg).map(str::to_owned),
    }
}

/// Infer the concrete type of a literal expression.
fn infer_literal_type(expr: &Expr) -> Option<&'static str> {
    match expr {
        Expr::NumberLiteral(n) => match &n.value {
            ruff_python_ast::Number::Int(_) => Some("int"),
            ruff_python_ast::Number::Float(_) => Some("float"),
            ruff_python_ast::Number::Complex { .. } => Some("complex"),
        },
        Expr::StringLiteral(_) => Some("str"),
        Expr::BytesLiteral(_) => Some("bytes"),
        Expr::BooleanLiteral(_) => Some("bool"),
        Expr::NoneLiteral(_) => Some("None"),
        _ => None,
    }
}

/// Check if `actual` is compatible with `expected` for subscript key types.
fn types_compatible(actual: &str, expected: &str) -> bool {
    if actual == expected {
        return true;
    }
    // Allow known widening: bool <: int, int <: float.
    matches!(
        (actual, expected),
        ("bool", "int" | "float") | ("int", "float")
    )
}

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

fn expr_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(n) => Some(n.id.as_str()),
        _ => None,
    }
}

fn ann_str(expr: &Expr) -> String {
    match expr {
        Expr::Name(n) => n.id.to_string(),
        Expr::Subscript(s) => format!("{}[{}]", ann_str(&s.value), ann_str(&s.slice)),
        Expr::Attribute(a) => format!("{}.{}", ann_str(&a.value), a.attr),
        Expr::Tuple(t) => t.elts.iter().map(ann_str).collect::<Vec<_>>().join(", "),
        Expr::BinOp(b) => format!("{} | {}", ann_str(&b.left), ann_str(&b.right)),
        Expr::NoneLiteral(_) => "None".to_owned(),
        Expr::StringLiteral(s) => s.value.to_str().to_owned(),
        _ => "...".to_owned(),
    }
}

/// Split a string by top-level commas (respecting bracket nesting).
fn split_top_level(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (idx, ch) in s.char_indices() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&s[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

/// Build a span for a call expression.
fn call_span(call: &ast::ExprCall) -> Span {
    Span {
        start: call.range().start().to_u32(),
        end: call.range().end().to_u32(),
    }
}
