//! constructors_call_type: Invalid constructor call via `type[T]` parameter.
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

mod helpers;

use std::collections::HashMap;

use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged as _;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic};

use super::Rule;

use helpers::{
    build_typevar_bound_map, check_kwarg_types, check_positional_arg_types, expr_simple_name,
    resolve_constructor_sig, ConstructorSig, CODE,
};

/// Emits constructors_call_type for invalid constructor calls via `type[T]` parameters.
pub(crate) struct TypeCallConstructorViolation;

impl Rule for TypeCallConstructorViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };

        // Collect class info and method maps from resolved module.
        let class_map: HashMap<&str, &basilisk_resolver::ClassInfo> =
            basilisk_resolver::name_lookup(&module.classes);

        let method_map = super::shared::method_name_map(&module.functions);

        // Collect TypeVar names (module-level).
        let typevar_names: Vec<&str> = basilisk_resolver::collect_names(&module.typevar_calls);

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

    let cctx = CheckCtx {
        source,
        path,
        type_param_map: &type_param_map,
        class_map,
        method_map,
        typevar_names,
        typevar_bounds,
    };
    // Walk function body.
    for stmt in &func.body {
        check_stmt_inner(stmt, &cctx, diagnostics);
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

    check_type_call(call, inner_type, cctx, diagnostics);
}

// ---------------------------------------------------------------------------
// Core validation
// ---------------------------------------------------------------------------

/// Validate a call `cls(...)` where `cls: type[inner_type]`.
fn check_type_call(
    call: &ast::ExprCall,
    inner_type: &str,
    cctx: &CheckCtx<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let span = Span::from(call.range());
    let total_args = call.arguments.args.len() + call.arguments.keywords.len();

    // Is inner_type an unbound TypeVar (no bound)?
    if cctx.typevar_names.contains(&inner_type) && !cctx.typevar_bounds.contains_key(inner_type) {
        check_unbound_typevar_call(inner_type, total_args, span, cctx.path, diagnostics);
        return;
    }

    // Resolve the effective class name (follow TypeVar bounds).
    let class_name = cctx
        .typevar_bounds
        .get(inner_type)
        .copied()
        .unwrap_or(inner_type);

    // Look up the class.
    let Some(class_info) = cctx.class_map.get(class_name) else {
        return;
    };

    let constructor_sig = resolve_constructor_sig(
        class_name,
        class_info,
        cctx.class_map,
        cctx.method_map,
        cctx.source,
    );

    check_constructor_call(
        call,
        class_name,
        &constructor_sig,
        total_args,
        span,
        cctx.source,
        cctx.path,
        cctx.method_map,
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
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Cannot pass arguments to constructor of unbound type variable `{inner_type}`; \
                 its constructor signature is unknown"
            ),
            span,
            path,
            Some(format!(
                "Add a `bound=` constraint to TypeVar `{inner_type}` if arguments are required"
            )),
            None,
        ));
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
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "`{class_name}` constructor takes no arguments \
                         but {total_args} argument(s) were provided via `type[{class_name}]`"
                    ),
                    span,
                    path,
                    Some(format!("Call `{class_name}()` with no arguments")),
                    None,
                ));
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
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "`{class_name}` constructor requires at least {min} argument(s) \
                         but {total_args} were provided via `type[{class_name}]`"
                    ),
                    span,
                    path,
                    Some(format!(
                        "Provide at least {min} argument(s) when calling `{class_name}` via a `type[{class_name}]` variable"
                    )),
                    None,
                ));
            } else if total_args > *max {
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "`{class_name}` constructor accepts at most {max} argument(s) \
                         but {total_args} were provided via `type[{class_name}]`"
                    ),
                    span,
                    path,
                    Some(format!(
                        "Pass at most {max} argument(s) when calling `{class_name}` via a `type[{class_name}]` variable"
                    )),
                    None,
                ));
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
