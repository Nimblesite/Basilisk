//! BSK-E0144: Invalid constructor call via `type[T]` parameter.
//!
//! When a parameter is typed as `type[T]` (where `T` is a concrete class or a
//! type variable), calling it as a constructor is equivalent to calling `T(...)`.
//! This rule checks that the arguments passed to such calls are consistent with
//! the constructor of `T`.
//!
//! Specification: <https://typing.readthedocs.io/en/latest/spec/constructors.html#constructor-calls-for-type-t>
//!
//! ## Cases detected
//!
//! 1. `cls: type[Class]` where `Class.__init__` / `Class.__new__` / metaclass
//!    `__call__` requires arguments but `cls()` is called with none.
//! 2. `cls: type[Class]` where `Class` has no custom constructor but `cls(arg)`
//!    is called with extra arguments.
//! 3. `cls: type[T]` (unbound `TypeVar`) called with any arguments — the
//!    constraint is unknown, so no arguments are permitted.
//! 4. `cls: type[T]` where `T` is bounded: same rules as the bound class.

use std::collections::HashMap;

use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged as _;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0144",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0144",
};

/// Emits BSK-E0144 for invalid constructor calls via `type[T]` parameters.
pub(crate) struct TypeCallConstructorViolation;

impl Rule for TypeCallConstructorViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let Ok(parsed) = basilisk_parser::parse_source(module.source.clone(), module.path.clone())
        else {
            return;
        };

        // Collect class info and method maps from resolved module.
        let class_map: HashMap<&str, &basilisk_resolver::ClassInfo> = module
            .classes
            .iter()
            .map(|c| (c.name.as_str(), c))
            .collect();

        let mut method_map: HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>> =
            HashMap::new();
        for func in &module.functions {
            if let Some(ref class_name) = func.class_name {
                method_map
                    .entry((class_name.as_str(), func.name.as_str()))
                    .or_default()
                    .push(func);
            }
        }

        // Collect TypeVar names (module-level).
        let typevar_names: Vec<&str> = module
            .typevar_calls
            .iter()
            .map(|tv| tv.name.as_str())
            .collect();

        // Build TypeVar bound map: typevar_name -> bound_class_name.
        let typevar_bounds = build_typevar_bound_map(&parsed.ast.body, &typevar_names);

        // Walk all top-level function definitions.
        for stmt in &parsed.ast.body {
            if let Stmt::FunctionDef(func) = stmt {
                check_function(
                    func,
                    &module.source,
                    &module.path,
                    &class_map,
                    &method_map,
                    &typevar_names,
                    &typevar_bounds,
                    diagnostics,
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TypeVar bound map
// ---------------------------------------------------------------------------

/// Build a map from `TypeVar` name to the bound class name, by scanning
/// `T = TypeVar("T", bound=SomeClass)` assignments.
fn build_typevar_bound_map<'src>(
    stmts: &'src [Stmt],
    typevar_names: &[&str],
) -> HashMap<&'src str, &'src str> {
    let mut map: HashMap<&str, &str> = HashMap::new();
    for stmt in stmts {
        let Stmt::Assign(assign) = stmt else { continue };
        if assign.targets.len() != 1 {
            continue;
        }
        let Some(var_name) = assign.targets.first().and_then(expr_simple_name) else {
            continue;
        };
        if !typevar_names.contains(&var_name) {
            continue;
        }
        // Look for TypeVar(..., bound=X) call.
        let Expr::Call(call) = assign.value.as_ref() else {
            continue;
        };
        let Some(callee_name) = expr_simple_name(&call.func) else {
            continue;
        };
        if callee_name != "TypeVar" {
            continue;
        }
        // Find `bound=` keyword.
        for kw in &call.arguments.keywords {
            if kw.arg.as_deref() == Some("bound") {
                if let Some(bound_class) = expr_simple_name(&kw.value) {
                    let _ = map.insert(var_name, bound_class);
                }
            }
        }
    }
    map
}

// ---------------------------------------------------------------------------
// Function-level checking
// ---------------------------------------------------------------------------

/// Check all call expressions inside a function whose parameters include
/// `type[X]`-typed variables.
#[expect(
    clippy::too_many_arguments,
    reason = "type checking requires full context"
)]
fn check_function(
    func: &ast::StmtFunctionDef,
    source: &str,
    path: &str,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    typevar_names: &[&str],
    typevar_bounds: &HashMap<&str, &str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Build a map from parameter-name -> type[X] inner type for every
    // parameter annotated as `type[Something]`.
    let type_param_map = collect_type_params(func, source);
    if type_param_map.is_empty() {
        return;
    }

    // Walk function body.
    for stmt in &func.body {
        check_stmt(
            stmt,
            source,
            path,
            &type_param_map,
            class_map,
            method_map,
            typevar_names,
            typevar_bounds,
            diagnostics,
        );
    }
}

/// Extract all parameters annotated as `type[X]` in a function definition.
/// Returns a map from parameter name to the inner type name `X`.
fn collect_type_params(func: &ast::StmtFunctionDef, source: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let params = &func.parameters;

    let all_params: Vec<&ast::ParameterWithDefault> = params
        .posonlyargs
        .iter()
        .chain(params.args.iter())
        .collect();

    for param_wd in all_params {
        let Some(ann) = &param_wd.parameter.annotation else {
            continue;
        };
        if let Some(inner) = extract_type_subscript_text(ann, source) {
            let _ = map.insert(param_wd.parameter.name.to_string(), inner);
        }
    }

    map
}

/// If `expr` is `type[X]`, return the text of `X` (trimmed).
fn extract_type_subscript_text(expr: &Expr, source: &str) -> Option<String> {
    let Expr::Subscript(sub) = expr else {
        return None;
    };
    // The subscript value must be the bare name `type`.
    let Expr::Name(name_node) = sub.value.as_ref() else {
        return None;
    };
    if name_node.id.as_str() != "type" {
        return None;
    }
    // Extract the inner text from source.
    let range = sub.slice.range();
    let text = source
        .get(range.start().to_usize()..range.end().to_usize())?
        .trim()
        .to_owned();
    Some(text)
}

// ---------------------------------------------------------------------------
// Statement / expression walking
// ---------------------------------------------------------------------------

/// Shared context for statement/expression checking to reduce argument count.
struct CheckCtx<'a> {
    source: &'a str,
    path: &'a str,
    type_param_map: &'a HashMap<String, String>,
    class_map: &'a HashMap<&'a str, &'a basilisk_resolver::ClassInfo>,
    method_map: &'a HashMap<(&'a str, &'a str), Vec<&'a basilisk_resolver::FunctionInfo>>,
    typevar_names: &'a [&'a str],
    typevar_bounds: &'a HashMap<&'a str, &'a str>,
}

#[expect(
    clippy::too_many_arguments,
    reason = "type checking requires full context"
)]
fn check_stmt(
    stmt: &Stmt,
    source: &str,
    path: &str,
    type_param_map: &HashMap<String, String>,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    typevar_names: &[&str],
    typevar_bounds: &HashMap<&str, &str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let cctx = CheckCtx {
        source,
        path,
        type_param_map,
        class_map,
        method_map,
        typevar_names,
        typevar_bounds,
    };
    check_stmt_inner(stmt, &cctx, diagnostics);
}

fn check_stmt_inner(stmt: &Stmt, cctx: &CheckCtx<'_>, diagnostics: &mut Vec<Diagnostic>) {
    match stmt {
        Stmt::Expr(e) => check_expr_inner(&e.value, cctx, diagnostics),
        Stmt::Assign(a) => check_expr_inner(&a.value, cctx, diagnostics),
        Stmt::AnnAssign(a) => {
            if let Some(val) = a.value.as_deref() {
                check_expr_inner(val, cctx, diagnostics);
            }
        }
        Stmt::Return(r) => {
            if let Some(val) = &r.value {
                check_expr_inner(val, cctx, diagnostics);
            }
        }
        Stmt::If(i) => {
            for s in i
                .body
                .iter()
                .chain(i.elif_else_clauses.iter().flat_map(|c| c.body.iter()))
            {
                check_stmt_inner(s, cctx, diagnostics);
            }
        }
        Stmt::For(f) => {
            for s in f.body.iter().chain(f.orelse.iter()) {
                check_stmt_inner(s, cctx, diagnostics);
            }
        }
        Stmt::While(w) => {
            for s in w.body.iter().chain(w.orelse.iter()) {
                check_stmt_inner(s, cctx, diagnostics);
            }
        }
        _ => {}
    }
}

fn check_expr_inner(expr: &Expr, cctx: &CheckCtx<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let Expr::Call(call) = expr else { return };

    // Recurse into arguments.
    for arg in &call.arguments.args {
        check_expr_inner(arg, cctx, diagnostics);
    }

    // Check whether the callee is a `type[X]`-typed parameter.
    let Some(callee_name) = expr_simple_name(&call.func) else {
        return;
    };
    let Some(inner_type) = cctx.type_param_map.get(callee_name) else {
        return;
    };

    check_type_call(
        call,
        inner_type,
        cctx.source,
        cctx.path,
        cctx.class_map,
        cctx.method_map,
        cctx.typevar_names,
        cctx.typevar_bounds,
        diagnostics,
    );
}

// ---------------------------------------------------------------------------
// Core validation
// ---------------------------------------------------------------------------

/// Validate a call `cls(...)` where `cls: type[inner_type]`.
#[expect(
    clippy::too_many_arguments,
    reason = "type checking requires full context"
)]
fn check_type_call(
    call: &ast::ExprCall,
    inner_type: &str,
    source: &str,
    path: &str,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    typevar_names: &[&str],
    typevar_bounds: &HashMap<&str, &str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let span = Span {
        start: call.range().start().to_u32(),
        end: call.range().end().to_u32(),
    };

    let total_args = call.arguments.args.len() + call.arguments.keywords.len();

    // Is inner_type an unbound TypeVar (no bound)?
    if typevar_names.contains(&inner_type) && !typevar_bounds.contains_key(inner_type) {
        check_unbound_typevar_call(inner_type, total_args, span, path, diagnostics);
        return;
    }

    // Resolve the effective class name (follow TypeVar bounds).
    let class_name = typevar_bounds
        .get(inner_type)
        .copied()
        .unwrap_or(inner_type);

    // Look up the class.
    let Some(class_info) = class_map.get(class_name) else {
        return;
    };

    let constructor_sig =
        resolve_constructor_sig(class_name, class_info, class_map, method_map, source);

    check_constructor_call(
        call,
        class_name,
        &constructor_sig,
        total_args,
        span,
        source,
        path,
        method_map,
        diagnostics,
    );
}

/// Emit diagnostic for calling an unbound `TypeVar` constructor with arguments.
fn check_unbound_typevar_call(
    inner_type: &str,
    total_args: usize,
    span: Span,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if total_args > 0 {
        diagnostics.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Cannot pass arguments to constructor of unbound type variable `{inner_type}`; \
                 its constructor signature is unknown"
            ),
            span,
            path: path.to_owned(),
            help: Some(format!(
                "Add a `bound=` constraint to TypeVar `{inner_type}` if arguments are required"
            )),
            note: None,
        });
    }
}

/// Check a constructor call against its resolved signature.
#[expect(
    clippy::too_many_arguments,
    reason = "type checking requires full context"
)]
fn check_constructor_call(
    call: &ast::ExprCall,
    class_name: &str,
    constructor_sig: &ConstructorSig,
    total_args: usize,
    span: Span,
    source: &str,
    path: &str,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match constructor_sig {
        ConstructorSig::NoArgs => {
            if total_args > 0 {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "`{class_name}` constructor takes no arguments \
                         but {total_args} argument(s) were provided via `type[{class_name}]`"
                    ),
                    span,
                    path: path.to_owned(),
                    help: Some(format!("Call `{class_name}()` with no arguments")),
                    note: None,
                });
            }
        }
        ConstructorSig::Required { min, max } => {
            let kw_names: Vec<&str> = call
                .arguments
                .keywords
                .iter()
                .filter_map(|kw| kw.arg.as_deref())
                .collect();

            if total_args < *min {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "`{class_name}` constructor requires at least {min} argument(s) \
                         but {total_args} were provided via `type[{class_name}]`"
                    ),
                    span,
                    path: path.to_owned(),
                    help: Some(format!(
                        "Provide at least {min} argument(s) when calling `{class_name}` via a `type[{class_name}]` variable"
                    )),
                    note: None,
                });
            } else if total_args > *max {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "`{class_name}` constructor accepts at most {max} argument(s) \
                         but {total_args} were provided via `type[{class_name}]`"
                    ),
                    span,
                    path: path.to_owned(),
                    help: Some(format!(
                        "Pass at most {max} argument(s) when calling `{class_name}` via a `type[{class_name}]` variable"
                    )),
                    note: None,
                });
            } else {
                check_kwarg_types(
                    call,
                    class_name,
                    &kw_names,
                    method_map,
                    source,
                    path,
                    span,
                    diagnostics,
                );
                check_positional_arg_types(
                    call,
                    class_name,
                    method_map,
                    source,
                    path,
                    span,
                    diagnostics,
                );
            }
        }
        ConstructorSig::Unknown => {
            // Passthrough (*args/**kwargs) – any call is OK.
        }
    }
}

// ---------------------------------------------------------------------------
// Constructor signature resolution
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum ConstructorSig {
    /// No non-self arguments (e.g. bare `object.__init__`).
    NoArgs,
    /// The constructor requires `min..=max` non-self arguments.
    Required { min: usize, max: usize },
    /// Cannot determine (varargs / kwargs present) — anything is OK.
    Unknown,
}

/// Resolve the constructor argument signature for a class by inspecting its
/// metaclass `__call__`, `__new__`, or `__init__` (in that priority order).
fn resolve_constructor_sig(
    class_name: &str,
    class_info: &basilisk_resolver::ClassInfo,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    source: &str,
) -> ConstructorSig {
    // 1. Check metaclass __call__ first.
    if let Some(meta_sig) = check_metaclass_call(class_info, class_map, method_map, source) {
        return meta_sig;
    }

    // 2. Check __new__.
    if let Some(new_sig) = method_map.get(&(class_name, "__new__")) {
        return sig_from_funcs(new_sig);
    }

    // 3. Check __init__.
    if let Some(init_sig) = method_map.get(&(class_name, "__init__")) {
        return sig_from_funcs(init_sig);
    }

    // 4. Walk base classes (simplified MRO).
    for base_name in class_bases(class_info) {
        if matches!(base_name, "object" | "Generic" | "Protocol") {
            continue;
        }
        if let Some(base_class) = class_map.get(base_name) {
            let sig = resolve_constructor_sig(base_name, base_class, class_map, method_map, source);
            if !matches!(sig, ConstructorSig::NoArgs) {
                return sig;
            }
        }
    }

    // Default: object() — no args.
    ConstructorSig::NoArgs
}

/// Extract the metaclass name and check its `__call__` method.
fn check_metaclass_call(
    class_info: &basilisk_resolver::ClassInfo,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    _source: &str,
) -> Option<ConstructorSig> {
    let meta_name = class_info.metaclass_name.as_deref()?;
    // Resolve metaclass through class_map (it may be defined in the same file).
    let _ = class_map.get(meta_name); // just check existence
    if let Some(call_funcs) = method_map.get(&(meta_name, "__call__")) {
        return Some(sig_from_funcs(call_funcs));
    }
    None
}

/// Derive a `ConstructorSig` from one or more `FunctionInfo` entries.
fn sig_from_funcs(funcs: &[&basilisk_resolver::FunctionInfo]) -> ConstructorSig {
    // Pick the first non-overload function.
    for func in funcs {
        if func.decorators.iter().any(|d| d == "overload") {
            continue;
        }
        // If it has *args or **kwargs, we can't know the exact arity.
        if func.vararg.is_some() || func.kwarg.is_some() {
            return ConstructorSig::Unknown;
        }
        // Skip the first parameter (self / cls).
        let params: Vec<&basilisk_resolver::ParameterInfo> =
            func.parameters.iter().skip(1).collect();
        let min = params.iter().filter(|p| !p.has_default).count();
        let max = params.len();
        return ConstructorSig::Required { min, max };
    }
    ConstructorSig::NoArgs
}

/// Return the simple base class names for a class.
fn class_bases(class_info: &basilisk_resolver::ClassInfo) -> Vec<&str> {
    let mut names: Vec<&str> = class_info
        .bases
        .iter()
        .map(|b| {
            let s: &str = b.as_str();
            s.split('[').next().unwrap_or(s)
        })
        .collect();
    for entry in &class_info.base_subscripts {
        let s: &str = entry.base_name.as_str();
        if !names.contains(&s) {
            names.push(s);
        }
    }
    names
}

// ---------------------------------------------------------------------------
// Keyword and positional argument type checking
// ---------------------------------------------------------------------------

/// Check that keyword arguments match the expected parameter types.
#[expect(
    clippy::too_many_arguments,
    reason = "type checking requires full context"
)]
fn check_kwarg_types(
    call: &ast::ExprCall,
    class_name: &str,
    kw_names: &[&str],
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    source: &str,
    path: &str,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Find the constructor function (prefer __new__ then __init__).
    let func = find_constructor_func(class_name, method_map);
    let Some(func) = func else { return };

    for kw in &call.arguments.keywords {
        let Some(kw_name) = kw.arg.as_deref() else {
            continue;
        };
        // Find the matching parameter.
        let Some(param) = func.parameters.iter().skip(1).find(|p| p.name == kw_name) else {
            continue;
        };
        let Some(ann_span) = param.annotation_span else {
            continue;
        };
        let Some(ann_text) = slice_span(source, ann_span) else {
            continue;
        };
        let expected_type = ann_text.trim();
        let Some(arg_type) = classify_literal_type(&kw.value) else {
            continue;
        };
        if !is_type_compatible(arg_type, expected_type) {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Keyword argument `{kw_name}={arg_type}` is incompatible with \
                     parameter `{kw_name}: {expected_type}` of `{class_name}` constructor"
                ),
                span,
                path: path.to_owned(),
                help: Some(format!(
                    "Pass a `{expected_type}` value for keyword argument `{kw_name}`"
                )),
                note: None,
            });
        }
    }
    // Suppress unused warning.
    let _ = kw_names;
}

/// Check positional arguments against the constructor parameter types.
fn check_positional_arg_types(
    call: &ast::ExprCall,
    class_name: &str,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    source: &str,
    path: &str,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let func = find_constructor_func(class_name, method_map);
    let Some(func) = func else { return };

    // Skip self/cls param.
    let params: Vec<&basilisk_resolver::ParameterInfo> = func.parameters.iter().skip(1).collect();

    for (idx, arg_expr) in call.arguments.args.iter().enumerate() {
        let Some(param) = params.get(idx) else { break };
        let Some(ann_span) = param.annotation_span else {
            continue;
        };
        let Some(ann_text) = slice_span(source, ann_span) else {
            continue;
        };
        let expected_type = ann_text.trim();
        let Some(arg_type) = classify_literal_type(arg_expr) else {
            continue;
        };
        if !is_type_compatible(arg_type, expected_type) {
            let arg_span = Span {
                start: arg_expr.range().start().to_u32(),
                end: arg_expr.range().end().to_u32(),
            };
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Argument {n} has type `{arg_type}` but parameter `{pname}` of \
                     `{class_name}` constructor expects `{expected_type}`",
                    n = idx + 1,
                    pname = param.name,
                ),
                span: arg_span,
                path: path.to_owned(),
                help: Some(format!(
                    "Pass a `{expected_type}` value as argument {n}",
                    n = idx + 1
                )),
                note: None,
            });
            // Use outer span to silence unreachable — not actually needed but
            // keeps the variable used.
            let _ = span;
        }
    }
}

/// Find the primary (non-overload) constructor function for a class.
fn find_constructor_func<'a>(
    class_name: &str,
    method_map: &'a HashMap<(&str, &str), Vec<&'a basilisk_resolver::FunctionInfo>>,
) -> Option<&'a basilisk_resolver::FunctionInfo> {
    for method in &["__new__", "__init__"] {
        if let Some(funcs) = method_map.get(&(class_name, method)) {
            for func in funcs {
                if !func.decorators.iter().any(|d| d == "overload") {
                    return Some(func);
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// If `expr` is a simple `Name` node, return its identifier string.
fn expr_simple_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(n) => Some(n.id.as_str()),
        _ => None,
    }
}

/// Classify the Python type of a literal expression.
fn classify_literal_type(expr: &Expr) -> Option<&'static str> {
    match expr {
        Expr::StringLiteral(_) => Some("str"),
        Expr::NumberLiteral(num) => {
            if num.value.is_int() {
                Some("int")
            } else {
                Some("float")
            }
        }
        Expr::BooleanLiteral(_) => Some("bool"),
        Expr::BytesLiteral(_) => Some("bytes"),
        Expr::NoneLiteral(_) => Some("None"),
        _ => None,
    }
}

/// Check if `arg_type` is compatible with `param_type`.
fn is_type_compatible(arg_type: &str, param_type: &str) -> bool {
    if arg_type == param_type {
        return true;
    }
    if param_type == "Any" || param_type == "object" {
        return true;
    }
    if param_type == "int" && arg_type == "bool" {
        return true;
    }
    if param_type == "float" && (arg_type == "int" || arg_type == "bool") {
        return true;
    }
    if param_type == "complex" && (arg_type == "int" || arg_type == "float" || arg_type == "bool") {
        return true;
    }
    if param_type.contains('|') {
        return param_type
            .split('|')
            .any(|part| is_type_compatible(arg_type, part.trim()));
    }
    false
}
