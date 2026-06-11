//! Implements [BSK-E0122] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! BSK-E0122: Callable call-site arity and argument validation.
//!
//! When a parameter is annotated as `Callable[[int, str], T]`, calls to that
//! parameter must match the expected argument count. Additionally, `Callable`
//! parameters are implicitly positional-only, so keyword arguments are not
//! allowed.

use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::{infer_expr_literal_type, is_type_compatible};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0122",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0122",
};

mod hof_paramspec;

/// Emits BSK-E0122 for invalid call-site usage of `Callable`-typed parameters.
pub(crate) struct CallableCallSiteViolation;

impl Rule for CallableCallSiteViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };
        let attr_callables = collect_paramspec_attr_callables(module, &parsed.ast.body);
        for stmt in &parsed.ast.body {
            walk_stmt_for_functions(stmt, &attr_callables, &module.path, diagnostics);
        }
        hof_paramspec::check_hof_paramspec_args(module, &parsed.ast.body, diagnostics);
    }
}

/// Index of `ParamSpec`-generic classes with callable attributes.
#[derive(Default)]
struct AttrCallables {
    /// Class name → `Callable[P, R]`-annotated attribute names.
    classes: std::collections::HashMap<String, Vec<String>>,
    /// Declared `ParamSpec` names in the module.
    paramspec_names: std::collections::HashSet<String>,
}

fn walk_stmt_for_functions(
    stmt: &Stmt,
    attr_callables: &AttrCallables,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match stmt {
        Stmt::FunctionDef(func) => check_function(func, attr_callables, path, diagnostics),
        Stmt::ClassDef(cls) => {
            for s in &cls.body {
                walk_stmt_for_functions(s, attr_callables, path, diagnostics);
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

fn check_function(
    func: &ast::StmtFunctionDef,
    attr_callables: &AttrCallables,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut callable_params = collect_callable_params(func);
    callable_params.extend(collect_paramspec_member_params(func, attr_callables));
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

/// Find classes generic over exactly one `ParamSpec` and collect their
/// `Callable[P, R]`-annotated attributes.
fn collect_paramspec_attr_callables(module: &ResolvedModule, stmts: &[Stmt]) -> AttrCallables {
    let mut result = AttrCallables {
        classes: std::collections::HashMap::new(),
        paramspec_names: module
            .typevar_calls
            .iter()
            .filter(|tv| tv.is_paramspec)
            .map(|tv| tv.name.clone())
            .collect(),
    };
    if result.paramspec_names.is_empty() {
        return result;
    }

    for stmt in stmts {
        let Stmt::ClassDef(cls) = stmt else { continue };
        let generic_params = super::shared::class_generic_param_names(cls);
        let [single] = generic_params.as_slice() else {
            continue;
        };
        if !result.paramspec_names.contains(single.as_str()) {
            continue;
        }
        let attrs: Vec<String> = cls
            .body
            .iter()
            .filter_map(|s| paramspec_callable_attr(s, single))
            .collect();
        if !attrs.is_empty() {
            let _ = result.classes.insert(cls.name.to_string(), attrs);
        }
    }
    result
}

/// Name of an attribute annotated `Callable[P, R]` for `ParamSpec` `P`, if any.
fn paramspec_callable_attr(stmt: &Stmt, paramspec: &str) -> Option<String> {
    let Stmt::AnnAssign(ann) = stmt else {
        return None;
    };
    let Expr::Name(target) = ann.target.as_ref() else {
        return None;
    };
    let Expr::Subscript(sub) = ann.annotation.as_ref() else {
        return None;
    };
    if !basilisk_resolver::is_name_or_attr_named(sub.value.as_ref(), "Callable") {
        return None;
    }
    let Expr::Tuple(tup) = sub.slice.as_ref() else {
        return None;
    };
    let [first, _ret] = tup.elts.as_slice() else {
        return None;
    };
    matches!(first, Expr::Name(n) if n.id.as_str() == paramspec).then(|| target.id.to_string())
}

/// Build `obj.attr` callable entries for parameters typed as a
/// ParamSpec-generic class specialization (`x: ClassC[[int, str]]` or the
/// PEP 612 shorthand `x: ClassC[int, str]`).
fn collect_paramspec_member_params(
    func: &ast::StmtFunctionDef,
    attr_callables: &AttrCallables,
) -> Vec<CallableParam> {
    let mut result = Vec::new();
    let params = &func.parameters;
    let all_params = params
        .posonlyargs
        .iter()
        .chain(params.args.iter())
        .chain(params.kwonlyargs.iter());

    for param in all_params {
        let Some(Expr::Subscript(sub)) = param.parameter.annotation.as_deref() else {
            continue;
        };
        let Expr::Name(base) = sub.value.as_ref() else {
            continue;
        };
        let Some(attrs) = attr_callables.classes.get(base.id.as_str()) else {
            continue;
        };
        let Some(specialization) =
            paramspec_specialization_args(sub.slice.as_ref(), &attr_callables.paramspec_names)
        else {
            continue;
        };
        let arg_types: Vec<String> = specialization
            .iter()
            .map(|e| annotation_to_string(e))
            .collect();
        for attr in attrs {
            result.push(CallableParam {
                name: format!("{}.{attr}", param.parameter.name),
                expected_args: Some(arg_types.len()),
                arg_types: arg_types.clone(),
            });
        }
    }
    result
}

/// The parameter list a ParamSpec-generic class is specialized with:
/// `C[[int, str]]` (explicit list) or `C[int, str]` (implicit shorthand).
/// `None` for non-concrete forms (`...`, a bare `ParamSpec`, `Concatenate[...]`).
fn paramspec_specialization_args<'a>(
    slice: &'a Expr,
    paramspec_names: &std::collections::HashSet<String>,
) -> Option<Vec<&'a Expr>> {
    let is_concrete_type = |e: &Expr| match e {
        Expr::Name(n) => !paramspec_names.contains(n.id.as_str()),
        Expr::Subscript(sub) => {
            !basilisk_resolver::is_name_or_attr_named(sub.value.as_ref(), "Concatenate")
        }
        Expr::NoneLiteral(_) | Expr::BinOp(_) => true,
        _ => false,
    };
    match slice {
        Expr::List(list) => Some(list.elts.iter().collect()),
        Expr::Tuple(tup) => tup
            .elts
            .iter()
            .all(&is_concrete_type)
            .then(|| tup.elts.iter().collect()),
        Expr::Name(_) | Expr::Subscript(_) => is_concrete_type(slice).then(|| vec![slice]),
        _ => None,
    }
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
        if let Some(callee) = callee_key(call.func.as_ref()) {
            if let Some(param) = cp.iter().find(|p| p.name == callee) {
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

/// Lookup key for a call's callee: `name` or `obj.attr`.
fn callee_key(func: &Expr) -> Option<String> {
    match func {
        Expr::Name(name) => Some(name.id.to_string()),
        Expr::Attribute(attr) => match attr.value.as_ref() {
            Expr::Name(obj) => Some(format!("{}.{}", obj.id, attr.attr)),
            _ => None,
        },
        _ => None,
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
