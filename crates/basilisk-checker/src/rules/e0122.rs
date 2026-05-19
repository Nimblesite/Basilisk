//! BSK-E0122: Callable call-site arity and argument validation.
//!
//! When a parameter is annotated as `Callable[[int, str], T]`, calls to that
//! parameter must match the expected argument count. Additionally, `Callable`
//! parameters are implicitly positional-only, so keyword arguments are not
//! allowed.

use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, error_diagnostic_owned};
use crate::rules::shared::{infer_expr_literal_type, is_type_compatible};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0122",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0122",
};

/// Emits BSK-E0122 for invalid call-site usage of `Callable`-typed parameters.
pub(crate) struct CallableCallSiteViolation;

impl Rule for CallableCallSiteViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };
        for stmt in &parsed.ast.body {
            walk_stmt_for_functions(stmt, &module.path, diagnostics);
        }
    }
}

fn walk_stmt_for_functions(stmt: &Stmt, path: &str, diagnostics: &mut Vec<Diagnostic>) {
    match stmt {
        Stmt::FunctionDef(func) => check_function(func, path, diagnostics),
        Stmt::ClassDef(cls) => {
            for s in &cls.body {
                walk_stmt_for_functions(s, path, diagnostics);
            }
        }
        _ => {}
    }
}

struct CallableParam {
    name: String,
    expected_args: Option<usize>,
    arg_types: Vec<String>,
}

fn check_function(func: &ast::StmtFunctionDef, path: &str, diagnostics: &mut Vec<Diagnostic>) {
    let callable_params = collect_callable_params(func);
    if callable_params.is_empty() {
        return;
    }
    check_body_calls(&func.body, &callable_params, path, diagnostics);
}

fn collect_callable_params(func: &ast::StmtFunctionDef) -> Vec<CallableParam> {
    let mut result = Vec::new();
    let params = &func.parameters;
    let all_params = params
        .posonlyargs
        .iter()
        .chain(params.args.iter())
        .chain(params.kwonlyargs.iter());
    for param in all_params {
        if let Some(annotation) = &param.parameter.annotation {
            if let Some(cp) = parse_callable_annotation(param.parameter.name.as_str(), annotation) {
                result.push(cp);
            }
        }
    }
    result
}

fn parse_callable_annotation(param_name: &str, annotation: &Expr) -> Option<CallableParam> {
    let Expr::Subscript(subscript) = annotation else {
        return None;
    };
    let is_callable =
        basilisk_resolver::is_name_or_attr_named(subscript.value.as_ref(), "Callable");
    if !is_callable {
        return None;
    }
    let tuple_elts = match subscript.slice.as_ref() {
        Expr::Tuple(tup) => &tup.elts,
        _ => return None,
    };
    if tuple_elts.len() != 2 {
        return None;
    }
    let first_arg = tuple_elts.first()?;
    if matches!(first_arg, Expr::EllipsisLiteral(_)) {
        return Some(CallableParam {
            name: param_name.to_owned(),
            expected_args: None,
            arg_types: Vec::new(),
        });
    }
    if matches!(first_arg, Expr::Subscript(sub) if matches!(sub.value.as_ref(), Expr::Name(n) if n.id.as_str() == "Concatenate"))
    {
        return None;
    }
    let arg_list = match first_arg {
        Expr::List(list) => &list.elts,
        _ => return None,
    };
    let arg_types: Vec<String> = arg_list.iter().map(annotation_to_string).collect();
    Some(CallableParam {
        name: param_name.to_owned(),
        expected_args: Some(arg_list.len()),
        arg_types,
    })
}

fn annotation_to_string(expr: &Expr) -> String {
    match expr {
        Expr::Name(name) => name.id.to_string(),
        Expr::Subscript(sub) => format!(
            "{}[{}]",
            annotation_to_string(&sub.value),
            annotation_to_string(&sub.slice)
        ),
        Expr::Attribute(attr) => format!("{}.{}", annotation_to_string(&attr.value), attr.attr),
        Expr::Tuple(tup) => tup
            .elts
            .iter()
            .map(annotation_to_string)
            .collect::<Vec<_>>()
            .join(", "),
        Expr::BinOp(b) => format!(
            "{} | {}",
            annotation_to_string(&b.left),
            annotation_to_string(&b.right)
        ),
        Expr::NoneLiteral(_) => "None".to_owned(),
        _ => "...".to_owned(),
    }
}

fn check_body_calls(stmts: &[Stmt], cp: &[CallableParam], path: &str, diag: &mut Vec<Diagnostic>) {
    basilisk_resolver::walk_function_stmts(stmts, &mut |stmt| match stmt {
        Stmt::Expr(node) => check_expr_for_call(&node.value, cp, path, diag),
        Stmt::Assign(node) => check_expr_for_call(&node.value, cp, path, diag),
        Stmt::AnnAssign(node) => {
            if let Some(v) = &node.value {
                check_expr_for_call(v, cp, path, diag);
            }
        }
        Stmt::Return(node) => {
            if let Some(v) = &node.value {
                check_expr_for_call(v, cp, path, diag);
            }
        }
        _ => {}
    });
}

fn check_expr_for_call(expr: &Expr, cp: &[CallableParam], path: &str, diag: &mut Vec<Diagnostic>) {
    if let Expr::Call(call) = expr {
        if let Expr::Name(name) = call.func.as_ref() {
            if let Some(param) = cp.iter().find(|p| p.name == name.id.as_str()) {
                validate_call(call, param, path, diag);
            }
        }
        for arg in &call.arguments.args {
            check_expr_for_call(arg, cp, path, diag);
        }
        for kw in &call.arguments.keywords {
            check_expr_for_call(&kw.value, cp, path, diag);
        }
    }
}

fn validate_call(call: &ast::ExprCall, cp: &CallableParam, path: &str, diag: &mut Vec<Diagnostic>) {
    let span = Span::from(call.range());
    let positional_count = call.arguments.args.len();
    let has_kwargs = !call.arguments.keywords.is_empty();

    if has_kwargs {
        diag.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "`Callable` parameter `{}` does not accept keyword arguments",
                cp.name
            ),
            span,
            path,
            Some("Callable parameters are positional-only".to_owned()),
            None,
        ));
        return;
    }

    let Some(expected) = cp.expected_args else {
        return;
    };
    match positional_count.cmp(&expected) {
        std::cmp::Ordering::Less => {
            diag.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Too few arguments for `{}`: expected {} but got {}",
                    cp.name, expected, positional_count
                ),
                span,
                path,
                Some(format!(
                    "`{}` is typed as `Callable[[{}], ...]`",
                    cp.name,
                    cp.arg_types.join(", ")
                )),
                None,
            ));
        }
        std::cmp::Ordering::Greater => {
            diag.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Too many arguments for `{}`: expected {} but got {}",
                    cp.name, expected, positional_count
                ),
                span,
                path,
                Some(format!(
                    "`{}` is typed as `Callable[[{}], ...]`",
                    cp.name,
                    cp.arg_types.join(", ")
                )),
                None,
            ));
        }
        std::cmp::Ordering::Equal => {
            check_arg_types(call, cp, path, diag);
        }
    }
}

fn check_arg_types(
    call: &ast::ExprCall,
    cp: &CallableParam,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    for (idx, (arg_expr, expected_type)) in call
        .arguments
        .args
        .iter()
        .zip(cp.arg_types.iter())
        .enumerate()
    {
        if let Some(actual) = infer_expr_literal_type(arg_expr) {
            if !is_type_compatible(actual, expected_type) {
                let span = Span::from(arg_expr.range());
                diag.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Argument {} to `{}` has incompatible type `{}`; expected `{}`",
                        idx + 1,
                        cp.name,
                        actual,
                        expected_type
                    ),
                    span,
                    path,
                    None,
                    None,
                ));
            }
        }
    }
}
