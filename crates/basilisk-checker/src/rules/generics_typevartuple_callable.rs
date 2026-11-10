//! Implements [`generics_typevartuple_callable`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `generics_typevartuple_callable`: `TypeVarTuple` callable/tuple argument mismatch.
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

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::infer_expr_literal_type;
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "generics_typevartuple_callable",
    docs_url: "https://www.basilisk-python.dev/errors/generics_typevartuple_callable",
};

/// Emits `generics_typevartuple_callable` when a tuple literal argument has elements whose types do
/// not match the order inferred from a `TypeVarTuple`-linked `Callable` argument.
pub(crate) struct TypeVarTupleCallableMismatch;

impl Rule for TypeVarTupleCallableMismatch {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let source = &module.source;
        let path = &module.path;

        // Step 1: Re-parse source to walk module-level call expressions.
        let Some(parsed) = super::shared::parse_module(module) else {
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
        let class_names: Vec<&str> = basilisk_resolver::collect_names(&module.classes);

        // Step 3: Walk module-level statements for calls. Element verdicts
        // route through the module-seeded context ([NARROWPLAN-SUBTYPING]).
        let lookups = TvtLookups {
            subtyping: crate::subtyping::module_context(module),
            func_sigs,
            method_sigs,
            class_names,
        };
        for stmt in &parsed.ast.body {
            check_stmt_for_tvt_mismatch(stmt, source, path, &lookups, diagnostics);
        }
    }
}

/// Read-only lookups for the `TypeVarTuple` mismatch walk.
struct TvtLookups<'a> {
    subtyping: crate::subtyping::SubtypingContext,
    func_sigs: HashMap<&'a str, &'a FunctionInfo>,
    method_sigs: HashMap<(&'a str, &'a str), &'a FunctionInfo>,
    class_names: Vec<&'a str>,
}

/// Walk statements to find constructor calls with `TypeVarTuple`-linked
/// parameters.
#[expect(
    clippy::too_many_lines,
    reason = "TypeVarTuple mismatch detection requires extensive AST traversal"
)]
fn check_stmt_for_tvt_mismatch(
    stmt: &ruff_python_ast::Stmt,
    source: &str,
    path: &str,
    lookups: &TvtLookups<'_>,
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
    if !lookups.class_names.contains(&callee_name) {
        return;
    }

    // Find the __init__ method.
    let Some(init_fn) = lookups.method_sigs.get(&(callee_name, "__init__")) else {
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
            let _ = kw_map.insert(arg_name.as_str(), &kw.value);
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
    let Some(target_fn) = lookups.func_sigs.get(callable_name.id.as_str()) else {
        return;
    };

    // Extract expected parameter types from the target function.
    let expected_types: Vec<String> = target_fn
        .parameters
        .iter()
        .filter_map(|p| {
            let ann_span = p.annotation_span?;
            slice_span(source, ann_span).map(|s| s.trim().to_owned())
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
        let actual_type = infer_expr_literal_type(elt);
        let Some(actual) = actual_type else {
            continue;
        };

        if !lookups.subtyping.is_subtype(actual, expected) {
            let range = call.range;
            let span = Span {
                start: range.start().to_u32(),
                end: range.end().to_u32(),
            };
            let _ = &tvt_name;
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Tuple element at index {idx} has type `{actual}` but \
                     `{callable_param_name}` expects `{expected}` (inferred via \
                     `TypeVarTuple`)"
                ),
                span,
                path,
                Some(format!(
                    "The `{tuple_param_name}` argument must match the parameter \
                     types of the function passed as `{callable_param_name}`"
                )),
                Some(
                    "When `Callable[[*Ts], R]` and `tuple[*Ts]` share the same \
                     `TypeVarTuple`, the tuple elements must match the callable's \
                     parameter types in order"
                        .to_owned(),
                ),
            ));
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
        let Some(ann_text) = slice_span(source, ann_span) else {
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
    let param_list = inner.get(param_start + 1..param_end)?;

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
    basilisk_resolver::is_simple_ascii_python_identifier(text)
}
