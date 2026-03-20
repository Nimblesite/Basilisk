//! Helper types and functions for BSK-E0144.
//!
//! Contains constructor signature resolution, argument type checking,
//! and shared AST utilities used by the main rule.

use std::collections::HashMap;

use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged as _;

use basilisk_resolver::Span;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};
use crate::rules::shared::{infer_expr_literal_type, is_type_compatible};
use crate::span_util::slice_span;

pub(super) const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0144",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0144",
};

// ---------------------------------------------------------------------------
// TypeVar bound map
// ---------------------------------------------------------------------------

/// Build a map from `TypeVar` name to the bound class name, by scanning
/// `T = TypeVar("T", bound=SomeClass)` assignments.
pub(super) fn build_typevar_bound_map<'src>(
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
// Constructor signature resolution
// ---------------------------------------------------------------------------

/// The resolved arity contract for a class constructor.
#[derive(Debug)]
pub(super) enum ConstructorSig {
    /// No non-self arguments (e.g. bare `object.__init__`).
    NoArgs,
    /// The constructor requires `min..=max` non-self arguments.
    Required { min: usize, max: usize },
    /// Cannot determine (varargs / kwargs present) — anything is OK.
    Unknown,
}

/// Resolve the constructor argument signature for a class by inspecting its
/// metaclass `__call__`, `__new__`, or `__init__` (in that priority order).
pub(super) fn resolve_constructor_sig(
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
pub(super) fn sig_from_funcs(funcs: &[&basilisk_resolver::FunctionInfo]) -> ConstructorSig {
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
pub(super) fn class_bases(class_info: &basilisk_resolver::ClassInfo) -> Vec<&str> {
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
pub(super) fn check_kwarg_types(
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
        let Some(arg_type) = infer_expr_literal_type(&kw.value) else {
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
pub(super) fn check_positional_arg_types(
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
        let Some(arg_type) = infer_expr_literal_type(arg_expr) else {
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
pub(super) fn find_constructor_func<'a>(
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
// Shared AST helpers
// ---------------------------------------------------------------------------

/// If `expr` is a simple `Name` node, return its identifier string.
pub(super) fn expr_simple_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(n) => Some(n.id.as_str()),
        _ => None,
    }
}
