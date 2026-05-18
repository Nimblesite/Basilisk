//! BSK-E0081: `TypeVarTuple` unpack minimum type argument violation.
//!
//! When a function parameter has a type annotation containing a `TypeVarTuple`
//! unpack pattern like `Array[Batch, *tuple[Any, ...], Channels]`, the type has
//! fixed prefix and suffix type arguments around a variadic middle.  Any value
//! passed to that parameter must have at least `prefix_count + suffix_count`
//! type arguments.
//!
//! ```python
//! Ts = TypeVarTuple("Ts")
//!
//! class Array(Generic[*Ts]): ...
//!
//! def process(x: Array[Batch, *tuple[Any, ...], Channels]) -> None: ...
//!
//! def func(z: Array[Batch]):
//!     process(z)  # E -- Array[Batch] has 1 type arg, need at least 2
//! ```

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, error_diagnostic_owned};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0081",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0081",
};

/// Emits BSK-E0081 when a function-body call passes a value whose generic type
/// does not have enough type arguments to satisfy a `TypeVarTuple` unpack pattern.
pub(crate) struct TypeVarTupleUnpackViolation;

impl Rule for TypeVarTupleUnpackViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        let mut variadic_params: HashMap<&str, Vec<VariadicParam>> = HashMap::new();
        for func in &module.functions {
            if func.class_name.is_some() {
                continue;
            }
            for (idx, param) in func.parameters.iter().enumerate() {
                let Some(ann_span) = param.annotation_span else {
                    continue;
                };
                let Some(ann_text) = slice_span(source, ann_span) else {
                    continue;
                };
                if let Some(vparam) = parse_variadic_annotation(ann_text.trim(), idx) {
                    variadic_params
                        .entry(func.name.as_str())
                        .or_default()
                        .push(vparam);
                }
            }
        }

        if variadic_params.is_empty() {
            return;
        }

        let Ok(parsed) = basilisk_parser::parse_source(source.clone(), path.clone()) else {
            return;
        };

        for stmt in &parsed.ast.body {
            visit_stmt_for_variadic_calls(
                stmt,
                source,
                path,
                &variadic_params,
                &module.functions,
                diagnostics,
            );
        }
    }
}

struct VariadicParam {
    param_idx: usize,
    base_class: String,
    min_type_args: usize,
}

fn parse_variadic_annotation(ann: &str, param_idx: usize) -> Option<VariadicParam> {
    if !ann.contains("*tuple[") {
        return None;
    }
    let bracket_pos = ann.find('[')?;
    let base_class = ann.get(..bracket_pos)?.trim().to_owned();
    if base_class.is_empty() {
        return None;
    }
    let inner = ann.get(bracket_pos + 1..ann.len().checked_sub(1)?)?;
    let args = split_type_args_at_commas(inner);
    let fixed_count = args
        .iter()
        .filter(|arg| !arg.trim().starts_with("*tuple["))
        .count();
    if fixed_count == 0 {
        return None;
    }
    Some(VariadicParam {
        param_idx,
        base_class,
        min_type_args: fixed_count,
    })
}

fn split_type_args_at_commas(inner: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    for (idx, ch) in inner.char_indices() {
        match ch {
            '[' | '(' => depth = depth.saturating_add(1),
            ']' | ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                if let Some(part) = inner.get(start..idx) {
                    parts.push(part);
                }
                start = idx + 1;
            }
            _ => {}
        }
    }
    if let Some(part) = inner.get(start..) {
        parts.push(part);
    }
    parts
}

fn visit_stmt_for_variadic_calls(
    stmt: &ruff_python_ast::Stmt,
    source: &str,
    path: &str,
    variadic_params: &HashMap<&str, Vec<VariadicParam>>,
    functions: &[FunctionInfo],
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Stmt;
    if let Stmt::FunctionDef(func_def) = stmt {
        let param_types = build_param_type_map(func_def, source);
        for body_stmt in &func_def.body {
            check_body_stmt(
                body_stmt,
                source,
                path,
                variadic_params,
                &param_types,
                functions,
                diagnostics,
            );
        }
    } else if let Stmt::ClassDef(cls) = stmt {
        for body_stmt in &cls.body {
            visit_stmt_for_variadic_calls(
                body_stmt,
                source,
                path,
                variadic_params,
                functions,
                diagnostics,
            );
        }
    }
}

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
                let _ = map.insert(param.name.to_string(), text.trim().to_string());
            }
        }
    }
    map
}

fn check_body_stmt(
    stmt: &ruff_python_ast::Stmt,
    _source: &str,
    path: &str,
    variadic_params: &HashMap<&str, Vec<VariadicParam>>,
    param_types: &HashMap<String, String>,
    functions: &[FunctionInfo],
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::{Expr, Stmt};
    use ruff_text_size::Ranged as _;
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
    let Some(vparams) = variadic_params.get(callee_name) else {
        return;
    };
    for vparam in vparams {
        let Some(arg_expr) = call.arguments.args.get(vparam.param_idx) else {
            continue;
        };
        let Expr::Name(arg_name) = arg_expr else {
            continue;
        };
        let Some(arg_type) = param_types.get(arg_name.id.as_str()) else {
            continue;
        };
        let arg_type_arg_count = count_type_args(arg_type, &vparam.base_class);
        if let Some(count) = arg_type_arg_count {
            if count < vparam.min_type_args {
                let range = call.range();
                let span = Span {
                    start: range.start().to_u32(),
                    end: range.end().to_u32(),
                };
                let _ = functions;
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "`{arg_type}` has {count} type argument{}, but `{callee_name}` requires at least {} for the `TypeVarTuple` unpack pattern",
                        if count == 1 { "" } else { "s" },
                        vparam.min_type_args
                    ),
                    span,
                    path,
                    Some(format!(
                        "The parameter expects `{}[...]` with at least {} fixed type argument{}",
                        vparam.base_class, vparam.min_type_args,
                        if vparam.min_type_args == 1 { "" } else { "s" }
                    )),
                    Some("A `TypeVarTuple` unpack like `*tuple[Any, ...]` absorbs zero or more type arguments, but the fixed parts must be present".to_owned()),
                ));
            }
        }
    }
}

fn count_type_args(annotation: &str, expected_base: &str) -> Option<usize> {
    let bracket_pos = annotation.find('[')?;
    let base = annotation.get(..bracket_pos)?.trim();
    if base != expected_base {
        return None;
    }
    let inner = annotation.get(bracket_pos + 1..annotation.len().checked_sub(1)?)?;
    let args = split_type_args_at_commas(inner);
    Some(args.iter().filter(|a| !a.trim().is_empty()).count())
}
