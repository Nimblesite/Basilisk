//! BSK-E0076: Overload union expansion failure.
//!
//! When a function-body call passes a union-typed argument to an overloaded
//! function and, after expanding the union, at least one member fails to
//! match any overload signature, Basilisk reports the error.
//!
//! ```python
//! @overload
//! def example(x: int, y: str, z: int) -> str: ...
//! @overload
//! def example(x: int, y: int, z: int) -> int: ...
//! def example(x: int, y: int | str, z: int) -> int | str:
//!     return 1
//!
//! def check(v: int | str) -> None:
//!     example(v, v, 1)  # E -- str not assignable to int in any overload
//! ```

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0076",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0076",
};

/// Emits BSK-E0076 when union expansion of arguments to an overloaded function
/// fails for some union member across all overloads.
pub(crate) struct OverloadUnionExpansionFailure;

impl Rule for OverloadUnionExpansionFailure {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        // Collect overloaded function groups: name -> Vec<&FunctionInfo> (overload stubs only).
        let mut overload_groups: HashMap<&str, Vec<&FunctionInfo>> = HashMap::new();
        for func in &module.functions {
            if func.class_name.is_some() {
                continue;
            }
            if !func.is_stub_body {
                continue;
            }
            if !func
                .decorators
                .iter()
                .any(|d| d == "overload" || d.ends_with(".overload"))
            {
                continue;
            }
            overload_groups
                .entry(func.name.as_str())
                .or_default()
                .push(func);
        }

        if overload_groups.is_empty() {
            return;
        }

        // Re-parse source to walk function bodies.
        let Ok(parsed) = basilisk_parser::parse_source(source.clone(), path.clone()) else {
            return;
        };

        // Walk each function definition looking for calls inside function bodies.
        for stmt in &parsed.ast.body {
            visit_stmt_for_overload_calls(stmt, source, path, &overload_groups, diagnostics);
        }
    }
}

/// Walk a statement recursively to find function definitions and check their bodies.
fn visit_stmt_for_overload_calls(
    stmt: &ruff_python_ast::Stmt,
    source: &str,
    path: &str,
    overload_groups: &HashMap<&str, Vec<&FunctionInfo>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Stmt;

    if let Stmt::FunctionDef(func_def) = stmt {
        // Build parameter type map for this function: param_name -> annotation_text.
        let param_types = build_param_type_map(func_def, source);

        // Walk the function body for call expressions.
        for body_stmt in &func_def.body {
            check_stmt_for_calls(
                body_stmt,
                source,
                path,
                overload_groups,
                &param_types,
                diagnostics,
            );
        }

        // Also recurse into nested function definitions.
        for body_stmt in &func_def.body {
            visit_stmt_for_overload_calls(body_stmt, source, path, overload_groups, diagnostics);
        }
    } else if let Stmt::ClassDef(cls) = stmt {
        for body_stmt in &cls.body {
            visit_stmt_for_overload_calls(body_stmt, source, path, overload_groups, diagnostics);
        }
    }
}

/// Build a map from parameter name to its annotation text for a function definition.
fn build_param_type_map(
    func_def: &ruff_python_ast::StmtFunctionDef,
    source: &str,
) -> HashMap<String, String> {
    use ruff_text_size::Ranged as _;

    let mut map = HashMap::new();
    for param_with_default in &func_def.parameters.args {
        let param = &param_with_default.parameter;
        if let Some(ann) = &param.annotation {
            let range = ann.range();
            if let Some(text) = source.get(range.start().to_usize()..range.end().to_usize()) {
                let _ = map.insert(param.name.to_string(), text.to_string());
            }
        }
    }
    map
}

/// Check a statement inside a function body for calls to overloaded functions.
fn check_stmt_for_calls(
    stmt: &ruff_python_ast::Stmt,
    source: &str,
    path: &str,
    overload_groups: &HashMap<&str, Vec<&FunctionInfo>>,
    param_types: &HashMap<String, String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Stmt;

    match stmt {
        Stmt::Expr(expr_stmt) => {
            check_expr_for_overload_call(
                &expr_stmt.value,
                source,
                path,
                overload_groups,
                param_types,
                diagnostics,
            );
        }
        Stmt::Assign(assign) => {
            check_expr_for_overload_call(
                &assign.value,
                source,
                path,
                overload_groups,
                param_types,
                diagnostics,
            );
        }
        Stmt::AnnAssign(ann_assign) => {
            if let Some(val) = &ann_assign.value {
                check_expr_for_overload_call(
                    val,
                    source,
                    path,
                    overload_groups,
                    param_types,
                    diagnostics,
                );
            }
        }
        Stmt::Return(ret) => {
            if let Some(val) = &ret.value {
                check_expr_for_overload_call(
                    val,
                    source,
                    path,
                    overload_groups,
                    param_types,
                    diagnostics,
                );
            }
        }
        _ => {}
    }
}

/// Check a call expression to see if it is calling an overloaded function
/// with union-typed arguments that fail expansion.
fn check_expr_for_overload_call(
    expr: &ruff_python_ast::Expr,
    source: &str,
    path: &str,
    overload_groups: &HashMap<&str, Vec<&FunctionInfo>>,
    param_types: &HashMap<String, String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Expr;
    use ruff_text_size::Ranged as _;

    let Expr::Call(call) = expr else {
        return;
    };

    // Get the callee name.
    let callee_name = match call.func.as_ref() {
        Expr::Name(name) => name.id.as_str(),
        _ => return,
    };

    // Check if the callee is an overloaded function.
    let Some(overloads) = overload_groups.get(callee_name) else {
        return;
    };

    // Skip if there are keyword arguments or star-args (too complex).
    if !call.arguments.keywords.is_empty() {
        return;
    }

    let arg_count = call.arguments.args.len();

    // Filter overloads by arity.
    let arity_matches: Vec<&&FunctionInfo> = overloads
        .iter()
        .filter(|f| {
            if f.vararg.is_some() {
                return true;
            }
            let required = f.parameters.iter().filter(|p| !p.has_default).count();
            let total = f.parameters.len();
            arg_count >= required && arg_count <= total
        })
        .collect();

    if arity_matches.is_empty() {
        return;
    }

    // For each argument, determine its type(s).
    // If the argument is a parameter reference with a union type, check each
    // union member against the overloads.
    for (arg_idx, arg_expr) in call.arguments.args.iter().enumerate() {
        let Some(union_members) = resolve_arg_union_types(arg_expr, param_types) else {
            continue;
        };

        if union_members.len() <= 1 {
            continue;
        }

        // For each union member, check if there exists at least one overload
        // where this member is compatible with the parameter at arg_idx.
        for member in &union_members {
            let matches_any = arity_matches.iter().any(|overload| {
                if let Some(param) = overload.parameters.get(arg_idx) {
                    if let Some(ann_span) = param.annotation_span {
                        if let Some(ann_text) = slice_span(source, ann_span) {
                            return is_type_assignable(member, ann_text);
                        }
                    }
                }
                false
            });

            if !matches_any {
                let span = Span::from(call.range());
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!("No overload of `{callee_name}` matches when argument is `{member}`"),
                    span,
                    path,
                    Some(format!(
                        "The union member `{member}` is not compatible with \
                         parameter at position {arg_idx} in any `@overload` signature"
                    )),
                    Some(
                        "When calling an overloaded function with a union-typed argument, \
                         each member of the union must match at least one overload"
                            .to_owned(),
                    ),
                ));
                // Only report once per call (not per member).
                return;
            }
        }
    }
}

/// Resolve the type(s) of an argument expression if it references a union-typed parameter.
///
/// Returns `Some(members)` if the argument is a name referencing a parameter
/// with a union type annotation, where `members.len() > 1`.
/// Returns `None` for non-parameter-reference arguments or non-union types.
fn resolve_arg_union_types(
    expr: &ruff_python_ast::Expr,
    param_types: &HashMap<String, String>,
) -> Option<Vec<String>> {
    use ruff_python_ast::Expr;

    let Expr::Name(name) = expr else {
        return None;
    };

    let param_name = name.id.as_str();
    let ann_text = param_types.get(param_name)?;

    let members = split_union_type(ann_text);
    if members.len() > 1 {
        Some(members)
    } else {
        None
    }
}

/// Split a union type annotation into its constituent members.
///
/// Handles both `X | Y` syntax and `Union[X, Y]` syntax.
fn split_union_type(annotation: &str) -> Vec<String> {
    let trimmed = annotation.trim();

    // Try `Union[X, Y]` syntax first.
    if let Some(inner) = trimmed
        .strip_prefix("Union[")
        .and_then(|s| s.strip_suffix(']'))
    {
        return split_type_args(inner)
            .into_iter()
            .map(str::to_owned)
            .collect();
    }

    // Try `X | Y` syntax.
    if trimmed.contains('|') {
        return split_pipe_union(trimmed)
            .into_iter()
            .map(str::to_owned)
            .collect();
    }

    // Not a union -- single member.
    vec![trimmed.to_owned()]
}

/// Split pipe-union `X | Y | Z` respecting bracket nesting.
fn split_pipe_union(annotation: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    for (idx, ch) in annotation.char_indices() {
        match ch {
            '[' | '(' => depth = depth.saturating_add(1),
            ']' | ')' => depth = depth.saturating_sub(1),
            '|' if depth == 0 => {
                let part = annotation[start..idx].trim();
                if !part.is_empty() {
                    parts.push(part);
                }
                start = idx + 1;
            }
            _ => {}
        }
    }
    let remainder = annotation[start..].trim();
    if !remainder.is_empty() {
        parts.push(remainder);
    }
    parts
}

/// Split type arguments respecting bracket nesting.
fn split_type_args(inner: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    for (idx, ch) in inner.char_indices() {
        match ch {
            '[' => depth = depth.saturating_add(1),
            ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let part = inner[start..idx].trim();
                if !part.is_empty() {
                    parts.push(part);
                }
                start = idx + 1;
            }
            _ => {}
        }
    }
    let remainder = inner[start..].trim();
    if !remainder.is_empty() {
        parts.push(remainder);
    }
    parts
}

/// Check if a type is assignable to an annotation.
fn is_type_assignable(source_type: &str, target_type: &str) -> bool {
    let src = source_type.trim();
    let tgt = target_type.trim();

    if src == tgt {
        return true;
    }

    // `Any` accepts everything.
    if tgt == "Any" || src == "Any" {
        return true;
    }

    // `object` accepts everything.
    if tgt == "object" {
        return true;
    }

    // Union in target: X | Y
    if tgt.contains('|') {
        return split_pipe_union(tgt)
            .iter()
            .any(|part| is_type_assignable(src, part));
    }

    // `Union[X, Y]` in target.
    if let Some(inner) = tgt.strip_prefix("Union[").and_then(|s| s.strip_suffix(']')) {
        return split_type_args(inner)
            .iter()
            .any(|part| is_type_assignable(src, part));
    }

    // Numeric tower: bool <: int <: float <: complex
    if tgt == "int" && src == "bool" {
        return true;
    }
    if tgt == "float" && (src == "int" || src == "bool") {
        return true;
    }
    if tgt == "complex" && (src == "int" || src == "float" || src == "bool") {
        return true;
    }

    false
}
