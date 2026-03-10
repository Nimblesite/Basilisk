//! BSK-E0082: `TypeVarTuple` callable/tuple argument mismatch.
//!
//! When a constructor (or function) links two parameters via a `TypeVarTuple`
//! -- one as `Callable[[*Ts], R]` and the other as `tuple[*Ts]` -- passing a
//! known function as the callable infers the expected element types for the
//! tuple.  If the tuple literal has elements whose types do not match the
//! inferred order, Basilisk reports the mismatch.
//!
//! ```python
//! Ts = TypeVarTuple("Ts")
//!
//! class Process:
//!     def __init__(self, target: Callable[[*Ts], None], args: tuple[*Ts]) -> None: ...
//!
//! def func1(arg1: int, arg2: str) -> None: ...
//!
//! Process(target=func1, args=(0, ""))   # OK
//! Process(target=func1, args=("", 0))  # E -- str, int does not match int, str
//! ```

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0082",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0082",
};

/// Emits BSK-E0082 when a tuple literal argument has elements whose types do
/// not match the order inferred from a `TypeVarTuple`-linked `Callable` argument.
pub(crate) struct TypeVarTupleCallableMismatch;

impl Rule for TypeVarTupleCallableMismatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        // Step 1: Re-parse source to walk module-level call expressions.
        let Ok(parsed) = basilisk_parser::parse_source(source.clone(), path.clone()) else {
            return;
        };

        // Step 2: Build lookup maps.
        // Function signatures by name (module-level, non-method functions).
        let func_sigs: HashMap<&str, &FunctionInfo> = module
            .functions
            .iter()
            .filter(|f| f.class_name.is_none())
            .map(|f| (f.name.as_str(), f))
            .collect();

        // Method signatures by (class_name, method_name).
        let method_sigs: HashMap<(&str, &str), &FunctionInfo> = module
            .functions
            .iter()
            .filter_map(|f| {
                let cls = f.class_name.as_deref()?;
                Some(((cls, f.name.as_str()), f))
            })
            .collect();

        // Class names for constructor detection.
        let class_names: Vec<&str> = module.classes.iter().map(|c| c.name.as_str()).collect();

        // Step 3: Walk module-level statements for calls.
        for stmt in &parsed.ast.body {
            check_stmt_for_tvt_mismatch(
                stmt,
                source,
                path,
                &func_sigs,
                &method_sigs,
                &class_names,
                diagnostics,
            );
        }
    }
}

/// Walk statements to find constructor calls with TypeVarTuple-linked parameters.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn check_stmt_for_tvt_mismatch(
    stmt: &ruff_python_ast::Stmt,
    source: &str,
    path: &str,
    func_sigs: &HashMap<&str, &FunctionInfo>,
    method_sigs: &HashMap<(&str, &str), &FunctionInfo>,
    class_names: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::{Expr, Stmt};

    let call = match stmt {
        Stmt::Expr(expr_stmt) => {
            if let Expr::Call(c) = expr_stmt.value.as_ref() {
                c
            } else {
                return;
            }
        }
        Stmt::Assign(assign) => {
            if let Expr::Call(c) = assign.value.as_ref() {
                c
            } else {
                return;
            }
        }
        _ => return,
    };

    let callee_name = match call.func.as_ref() {
        Expr::Name(name) => name.id.as_str(),
        _ => return,
    };

    // Only handle constructor calls (callee is a class name).
    if !class_names.contains(&callee_name) {
        return;
    }

    // Find the __init__ method.
    let Some(init_fn) = method_sigs.get(&(callee_name, "__init__")) else {
        return;
    };

    // Look for linked TypeVarTuple parameters:
    // One param has `Callable[[*Ts], ...]` and another has `tuple[*Ts]`.
    let linked = find_linked_tvt_params(init_fn, source);
    let Some((callable_param_name, tuple_param_name, tvt_name)) = linked else {
        return;
    };

    // Extract keyword arguments from the call.
    let mut kw_map: HashMap<&str, &ruff_python_ast::Expr> = HashMap::new();
    for kw in &call.arguments.keywords {
        if let Some(arg_name) = &kw.arg {
            kw_map.insert(arg_name.as_str(), &kw.value);
        }
    }

    // Get the callable argument (must be a function name).
    let Some(callable_expr) = kw_map.get(callable_param_name.as_str()) else {
        return;
    };
    let Expr::Name(callable_name) = callable_expr else {
        return;
    };

    // Resolve the function signature for the callable.
    let Some(target_fn) = func_sigs.get(callable_name.id.as_str()) else {
        return;
    };

    // Extract expected parameter types from the target function.
    let expected_types: Vec<String> = target_fn
        .parameters
        .iter()
        .filter_map(|p| {
            let ann_span = p.annotation_span?;
            source
                .get(ann_span.start as usize..ann_span.end as usize)
                .map(|s| s.trim().to_owned())
        })
        .collect();

    if expected_types.is_empty() {
        return;
    }

    // Get the tuple argument.
    let Some(tuple_expr) = kw_map.get(tuple_param_name.as_str()) else {
        return;
    };
    let Expr::Tuple(tuple_lit) = tuple_expr else {
        return;
    };

    // Match tuple elements against expected types.
    if tuple_lit.elts.len() != expected_types.len() {
        return; // Arity mismatch is a different error.
    }

    for (idx, (elt, expected)) in tuple_lit.elts.iter().zip(expected_types.iter()).enumerate() {
        let actual_type = infer_literal_type(elt);
        let Some(actual) = actual_type else {
            continue;
        };

        if !type_compatible(actual, expected) {
            let range = call.range;
            let span = Span {
                start: range.start().to_u32(),
                end: range.end().to_u32(),
            };
            let _ = &tvt_name;
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Tuple element at index {idx} has type `{actual}` but \
                     `{callable_param_name}` expects `{expected}` (inferred via \
                     `TypeVarTuple`)"
                ),
                span,
                path: path.to_owned(),
                help: Some(format!(
                    "The `{tuple_param_name}` argument must match the parameter \
                     types of the function passed as `{callable_param_name}`"
                )),
                note: Some(
                    "When `Callable[[*Ts], R]` and `tuple[*Ts]` share the same \
                     `TypeVarTuple`, the tuple elements must match the callable's \
                     parameter types in order"
                        .to_owned(),
                ),
            });
            return; // One diagnostic per call is enough.
        }
    }
}

/// Find linked `TypeVarTuple` parameters in a function signature.
///
/// Looks for a pattern where one parameter has `Callable[[*Ts], ...]` and
/// another has `tuple[*Ts]`, sharing the same `TypeVarTuple` name.
///
/// Returns `(callable_param_name, tuple_param_name, tvt_name)`.
fn find_linked_tvt_params(func: &FunctionInfo, source: &str) -> Option<(String, String, String)> {
    let mut callable_param: Option<(String, String)> = None; // (param_name, tvt_name)
    let mut tuple_params: Vec<(String, String)> = Vec::new(); // (param_name, tvt_name)

    for param in &func.parameters {
        let Some(ann_span) = param.annotation_span else {
            continue;
        };
        let Some(ann_text) = source.get(ann_span.start as usize..ann_span.end as usize) else {
            continue;
        };
        let ann_text = ann_text.trim();

        // Check for `Callable[[*Ts], ...]` pattern.
        if ann_text.starts_with("Callable[") {
            if let Some(tvt) = extract_tvt_from_callable(ann_text) {
                callable_param = Some((param.name.clone(), tvt));
            }
        }

        // Check for `tuple[*Ts]` pattern.
        if ann_text.starts_with("tuple[") {
            if let Some(tvt) = extract_tvt_from_tuple(ann_text) {
                tuple_params.push((param.name.clone(), tvt));
            }
        }
    }

    let (callable_name, callable_tvt) = callable_param?;
    let matching_tuple = tuple_params
        .into_iter()
        .find(|(_, tvt)| *tvt == callable_tvt)?;

    Some((callable_name, matching_tuple.0, callable_tvt))
}

/// Extract the `TypeVarTuple` name from a `Callable[[*Ts], ...]` annotation.
fn extract_tvt_from_callable(ann: &str) -> Option<String> {
    // Pattern: `Callable[[*Ts], None]` or `Callable[[*Ts], R]`
    let inner = ann.strip_prefix("Callable[")?;
    let inner = inner.strip_suffix(']')?;

    // Find the parameter list: `[*Ts]` or `[int, *Ts, str]`
    let param_start = inner.find('[')?;
    let param_end = inner.find(']')?;
    let param_list = &inner[param_start + 1..param_end];

    // Look for `*Ts` pattern.
    for part in param_list.split(',') {
        let trimmed = part.trim();
        if let Some(name) = trimmed.strip_prefix('*') {
            let name = name.trim();
            if !name.is_empty() && is_identifier(name) {
                return Some(name.to_owned());
            }
        }
    }
    None
}

/// Extract the `TypeVarTuple` name from a `tuple[*Ts]` annotation.
fn extract_tvt_from_tuple(ann: &str) -> Option<String> {
    let inner = ann.strip_prefix("tuple[")?;
    let inner = inner.strip_suffix(']')?;

    // Look for `*Ts` pattern among the elements.
    for part in inner.split(',') {
        let trimmed = part.trim();
        if let Some(name) = trimmed.strip_prefix('*') {
            let name = name.trim();
            if !name.is_empty() && is_identifier(name) {
                return Some(name.to_owned());
            }
        }
    }
    None
}

/// Check if a string is a valid Python identifier.
fn is_identifier(text: &str) -> bool {
    !text.is_empty()
        && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && text
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
}

/// Infer the type of a literal expression.
fn infer_literal_type(expr: &ruff_python_ast::Expr) -> Option<&'static str> {
    use ruff_python_ast::Expr;
    match expr {
        Expr::NumberLiteral(num) => {
            if num.value.is_int() {
                Some("int")
            } else {
                Some("float")
            }
        }
        Expr::StringLiteral(_) => Some("str"),
        Expr::BytesLiteral(_) => Some("bytes"),
        Expr::BooleanLiteral(_) => Some("bool"),
        Expr::NoneLiteral(_) => Some("None"),
        _ => None,
    }
}

/// Check basic type compatibility.
fn type_compatible(actual: &str, expected: &str) -> bool {
    if actual == expected {
        return true;
    }
    // Numeric tower: bool <: int <: float <: complex
    if expected == "int" && actual == "bool" {
        return true;
    }
    if expected == "float" && (actual == "int" || actual == "bool") {
        return true;
    }
    if expected == "complex" && matches!(actual, "int" | "float" | "bool") {
        return true;
    }
    // Any accepts everything.
    if expected == "Any" || actual == "Any" {
        return true;
    }
    // object accepts everything.
    if expected == "object" {
        return true;
    }
    false
}
