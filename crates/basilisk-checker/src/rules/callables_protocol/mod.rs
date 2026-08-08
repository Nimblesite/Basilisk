//! Implements [`callables_protocol`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `callables_protocol`: Callable call-site arity and argument validation.
//!
//! Calls to a callable member of a `ParamSpec`-generic class specialization must
//! match the parameter list the class was specialized with. Such members are
//! implicitly positional-only, so keyword arguments are not allowed.
//!
//! Every verdict comes from the resolved semantic model ([ASTREBUILD-LAW]):
//! `Callable` heads resolve through the binding table, specialization
//! arguments lower to [`TypeNode`]s, and argument compatibility is decided by
//! [`assignable`] — a diagnostic is emitted only on a definite `Some(false)`.
//! Source text appears in diagnostic messages only.

use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::{assignable, ResolvedModule, Span, TypeNode, TypingForm};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::node_message_text;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "callables_protocol",
    docs_url: "https://www.basilisk-python.dev/errors/callables_protocol",
};

mod hof_paramspec;
mod paramspec_components;

/// Emits `callables_protocol` for invalid call-site usage of `Callable`-typed parameters.
pub(crate) struct CallableCallSiteViolation;

impl Rule for CallableCallSiteViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };
        let attr_callables = collect_paramspec_attr_callables(module, &parsed.ast.body);
        for stmt in &parsed.ast.body {
            walk_stmt_for_functions(stmt, &attr_callables, module, diagnostics);
        }
        hof_paramspec::check_hof_paramspec_args(module, &parsed.ast.body, diagnostics);
        paramspec_components::check_paramspec_components(module, &parsed.ast.body, diagnostics);
    }
}

/// Index of `ParamSpec`-generic classes with callable attributes.
#[derive(Default)]
struct AttrCallables {
    /// Class name → `ParamSpec`-binding attribute names.
    classes: std::collections::HashMap<String, Vec<String>>,
    /// Declared `ParamSpec` names in the module.
    paramspec_names: std::collections::HashSet<String>,
}

fn walk_stmt_for_functions(
    stmt: &Stmt,
    attr_callables: &AttrCallables,
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match stmt {
        Stmt::FunctionDef(func) => check_function(func, attr_callables, module, diagnostics),
        Stmt::ClassDef(cls) => {
            for s in &cls.body {
                walk_stmt_for_functions(s, attr_callables, module, diagnostics);
            }
        }
        _ => {}
    }
}

struct CallableParam {
    name: String,
    expected_args: Option<usize>,
    /// Resolved parameter types of the specialization — the verdict source
    /// ([ASTREBUILD-LAW]).
    arg_nodes: Vec<TypeNode>,
    /// Source renderings of the parameter types, diagnostic messages only.
    arg_display: Vec<String>,
}

fn check_function(
    func: &ast::StmtFunctionDef,
    attr_callables: &AttrCallables,
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let callable_params = collect_paramspec_member_params(func, attr_callables, module);
    if callable_params.is_empty() {
        return;
    }
    check_body_calls(&func.body, &callable_params, module, diagnostics);
}

/// Find classes generic over exactly one `ParamSpec` and collect their
/// callable attributes.
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
            .filter_map(|s| paramspec_callable_attr(module, s, single))
            .collect();
        if !attrs.is_empty() {
            let _ = result.classes.insert(cls.name.to_string(), attrs);
        }
    }
    result
}

/// Name of an attribute whose `Callable[P, R]` annotation binds `ParamSpec`
/// `P`, if any. The `Callable` head is recognised by what it RESOLVES to
/// ([ASTREBUILD-LAW]), never by its spelling.
fn paramspec_callable_attr(module: &ResolvedModule, stmt: &Stmt, paramspec: &str) -> Option<String> {
    let Stmt::AnnAssign(ann) = stmt else {
        return None;
    };
    let Expr::Name(target) = ann.target.as_ref() else {
        return None;
    };
    let Expr::Subscript(sub) = ann.annotation.as_ref() else {
        return None;
    };
    if module.bindings.form_of_with_builtins(&sub.value) != Some(TypingForm::Callable) {
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
/// PEP 612 shorthand `x: ClassC[int, str]`), lowering each specialization
/// argument through the module's bindings.
fn collect_paramspec_member_params(
    func: &ast::StmtFunctionDef,
    attr_callables: &AttrCallables,
    module: &ResolvedModule,
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
        let arg_nodes: Vec<TypeNode> = specialization
            .iter()
            .map(|e| TypeNode::lower(&module.bindings, e))
            .collect();
        let arg_display: Vec<String> = specialization
            .iter()
            .map(|e| node_message_text(&module.source, *e).to_owned())
            .collect();
        for attr in attrs {
            result.push(CallableParam {
                name: format!("{}.{attr}", param.parameter.name),
                expected_args: Some(arg_nodes.len()),
                arg_nodes: arg_nodes.clone(),
                arg_display: arg_display.clone(),
            });
        }
    }
    result
}

/// The parameter list a ParamSpec-generic class is specialized with:
/// `C[[int, str]]` (explicit list) or `C[int, str]` (implicit shorthand).
/// `None` for non-concrete forms (`...`, a bare `ParamSpec`).
fn paramspec_specialization_args<'a>(
    slice: &'a Expr,
    paramspec_names: &std::collections::HashSet<String>,
) -> Option<Vec<&'a Expr>> {
    let is_concrete_type = |e: &Expr| match e {
        Expr::Name(n) => !paramspec_names.contains(n.id.as_str()),
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

fn check_body_calls(
    stmts: &[Stmt],
    cp: &[CallableParam],
    module: &ResolvedModule,
    diag: &mut Vec<Diagnostic>,
) {
    basilisk_resolver::walk_function_stmts(stmts, &mut |stmt| match stmt {
        Stmt::Expr(node) => check_expr_for_call(&node.value, cp, module, diag),
        Stmt::Assign(node) => check_expr_for_call(&node.value, cp, module, diag),
        Stmt::AnnAssign(node) => {
            if let Some(v) = &node.value {
                check_expr_for_call(v, cp, module, diag);
            }
        }
        Stmt::Return(node) => {
            if let Some(v) = &node.value {
                check_expr_for_call(v, cp, module, diag);
            }
        }
        _ => {}
    });
}

fn check_expr_for_call(
    expr: &Expr,
    cp: &[CallableParam],
    module: &ResolvedModule,
    diag: &mut Vec<Diagnostic>,
) {
    if let Expr::Call(call) = expr {
        if let Some(callee) = callee_key(call.func.as_ref()) {
            if let Some(param) = cp.iter().find(|p| p.name == callee) {
                validate_call(call, param, module, diag);
            }
        }
        for arg in &call.arguments.args {
            check_expr_for_call(arg, cp, module, diag);
        }
        for kw in &call.arguments.keywords {
            check_expr_for_call(&kw.value, cp, module, diag);
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

fn validate_call(
    call: &ast::ExprCall,
    cp: &CallableParam,
    module: &ResolvedModule,
    diag: &mut Vec<Diagnostic>,
) {
    let path = &module.path;
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
                    cp.arg_display.join(", ")
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
                    cp.arg_display.join(", ")
                )),
                None,
            ));
        }
        std::cmp::Ordering::Equal => {
            check_arg_types(call, cp, module, diag);
        }
    }
}

/// Relate each literal argument to the resolved specialization type via
/// [`assignable`]; only a definite `Some(false)` reports
/// ([RESOLV-CANONICAL-RELATION]). Non-literal arguments and unresolvable
/// annotations abstain.
fn check_arg_types(
    call: &ast::ExprCall,
    cp: &CallableParam,
    module: &ResolvedModule,
    diag: &mut Vec<Diagnostic>,
) {
    for (idx, (arg_expr, expected_node)) in call
        .arguments
        .args
        .iter()
        .zip(cp.arg_nodes.iter())
        .enumerate()
    {
        let actual = TypeNode::of_literal_expr(arg_expr);
        if assignable(&actual, expected_node) == Some(false) {
            let span = Span::from(arg_expr.range());
            let expected_display = cp
                .arg_display
                .get(idx)
                .map_or("<type>", String::as_str);
            diag.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Argument {} to `{}` (`{}`) is not assignable to `{}`",
                    idx + 1,
                    cp.name,
                    node_message_text(&module.source, arg_expr),
                    expected_display
                ),
                span,
                &module.path,
                None,
                None,
            ));
        }
    }
}
