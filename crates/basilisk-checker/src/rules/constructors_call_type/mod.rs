//! `constructors_call_type`: Invalid constructor call via `type[T]` parameter.
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

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic};

use super::Rule;

use helpers::{
    build_typevar_bound_map, check_kwarg_types, check_positional_arg_types, expr_simple_name,
    ConstructorSig, CODE,
};

/// Emits `constructors_call_type` for invalid constructor calls via `type[T]` parameters.
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
        let index = super::shared::ExprIndex::build(&parsed.ast);

        // Collect class info and method maps from resolved module.
        let class_map: HashMap<&str, &basilisk_resolver::ClassInfo> =
            basilisk_resolver::name_lookup(&module.classes);

        let graph = basilisk_resolver::ClassGraph::new(&module.classes);
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
                    &module.bindings,
                    &index,
                    &class_map,
                    &graph,
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
    bindings: &basilisk_resolver::BindingTable,
    index: &super::shared::ExprIndex<'_>,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    graph: &basilisk_resolver::ClassGraph<'_>,
    method_map: &HashMap<(basilisk_resolver::Span, &str), Vec<&basilisk_resolver::FunctionInfo>>,
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
        bindings,
        index,
        type_param_map: &type_param_map,
        class_map,
        graph,
        method_map,
        typevar_names,
        typevar_bounds,
    };
    // Walk function body.
    for stmt in &func.body {
        check_stmt_inner(stmt, &cctx, diagnostics);
    }
}

// DELETED: this body built the `type[X]` parameter map from rendered annotation
// text. Rebuild it from the annotation AST and resolved definition identities.
fn collect_type_params(_func: &ast::StmtFunctionDef, _source: &str) -> HashMap<String, String> {
    panic!(concat!(
        "basilisk-checker: `collect_type_params` was DELETED because it built constructor ",
        "targets by extracting `X` from the SOURCE SPELLING of `type[X]`. It panics ",
        "because the real implementation — resolving the annotation head to ",
        "`TypingForm::TypeClass` and its slice to a class or TypeVar definition site — ",
        "DOES NOT EXIST YET. Do not restore the text extractor and do not return an ",
        "empty map in its place."
    ))
}

// ##########################################################################
// # DELETED BODY — `extract_type_subscript_text`. DO NOT RESTORE IT.       #
// #                                                                         #
// #   if name_node.id.as_str() != "type" { return None; }                  #
// #   source.get(range.start()..range.end())?.trim().to_owned()            #
// #                                                                         #
// # Two spelling operations. `type` was recognised only when written as    #
// # that exact bare word — CLAUDE.md: builtins are not an exception, a     #
// # name resolves through the binding table like any other — so:           #
// #                                                                         #
// #   * `builtins.type[X]` is the identical object and was never seen;     #
// #   * a module declaring its own `class type` had its subscript treated  #
// #     as the typing form, producing diagnostics about a contract that    #
// #     does not apply;                                                     #
// #   * `from builtins import type as Class` was invisible.                #
// #                                                                         #
// # `X` was then taken as SOURCE TEXT and joined to classes, `TypeVar`s    #
// # and bounds through name-keyed maps, so an aliased class was missed and #
// # a same-named one matched.                                              #
// #                                                                         #
// # The lawful replacement resolves `sub.value` with                        #
// # `BindingTable::form_of_with_builtins` against `TypingForm::TypeClass`, #
// # then resolves `sub.slice` to a definition site with                    #
// # `local_class_definition` / `local_value_binding` and joins on THAT.    #
// #                                                                         #
// # Pinned by: tests/constructor_identity_tests.rs                          #
// ##########################################################################

/// DELETED — panics; see the banner above.
#[expect(
    dead_code,
    reason = "the rendered-text caller was deleted; retained as an explicit reconstruction boundary"
)]
fn extract_type_subscript_text(_expr: &Expr, _source: &str) -> Option<String> {
    panic!(
        "basilisk-checker: `extract_type_subscript_text` was DELETED because it \
         recognised `type[X]` only by the bare spelling `type` and then read `X` as raw \
         SOURCE TEXT. It panics because the real implementation — `sub.value` resolved \
         through the binding table to `TypingForm::TypeClass`, and `sub.slice` resolved \
         to a definition site — DOES NOT EXIST YET. Do not restore the name comparison \
         or the slice, and do not return `None` in its place."
    )
}

// ---------------------------------------------------------------------------
// Statement / expression walking
// ---------------------------------------------------------------------------

/// Shared context for statement/expression checking to reduce argument count.
#[expect(
    dead_code,
    reason = "definition-site inputs are retained for the identity-based rebuild after deleting the name-keyed verdict"
)]
struct CheckCtx<'a> {
    source: &'a str,
    path: &'a str,
    bindings: &'a basilisk_resolver::BindingTable,
    index: &'a super::shared::ExprIndex<'a>,
    type_param_map: &'a HashMap<String, String>,
    class_map: &'a HashMap<&'a str, &'a basilisk_resolver::ClassInfo>,
    graph: &'a basilisk_resolver::ClassGraph<'a>,
    method_map:
        &'a HashMap<(basilisk_resolver::Span, &'a str), Vec<&'a basilisk_resolver::FunctionInfo>>,
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

// DELETED: this body joined a rendered inner-type name to TypeVars, bounds, and
// classes. Rebuild the whole judgment on resolved definition sites.
fn check_type_call(
    _call: &ast::ExprCall,
    _inner_type: &str,
    _cctx: &CheckCtx<'_>,
    _diagnostics: &mut Vec<Diagnostic>,
) {
    panic!(concat!(
        "basilisk-checker: `check_type_call` was DELETED because it decided whether the ",
        "inner type was a TypeVar, followed its bound, and selected a class by comparing ",
        "RENDERED NAMES. It panics because the real implementation — a constructor target ",
        "carried as a resolved TypeVar or class definition site — DOES NOT EXIST YET. Do ",
        "not restore the name maps and do not substitute a default verdict."
    ))
}

/// Emit diagnostic for calling an unbound `TypeVar` constructor with arguments.
#[expect(
    dead_code,
    reason = "orphaned by the deleted name-keyed `check_type_call`; retained for the identity-based rebuild"
)]
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
    reason = "the class arrives as both an identity (for lookups) and a rendering (for messages)"
)]
#[expect(
    dead_code,
    reason = "orphaned by the deleted name-keyed `check_type_call`; retained for the identity-based rebuild"
)]
fn check_constructor_call(
    call: &ast::ExprCall,
    class_name: &str,
    class_site: basilisk_resolver::Span,
    constructor_sig: &ConstructorSig,
    total_args: usize,
    span: Span,
    cctx: &CheckCtx<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let path = cctx.path;
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
                    class_site,
                    class_name,
                    &kw_names,
                    cctx.method_map,
                    cctx.bindings,
                    cctx.index,
                    cctx.source,
                    path,
                    span,
                    diagnostics,
                );
                check_positional_arg_types(
                    call,
                    class_site,
                    class_name,
                    cctx.method_map,
                    cctx.bindings,
                    cctx.index,
                    cctx.source,
                    path,
                    diagnostics,
                );
            }
        }
        ConstructorSig::Unknown => {
            // Passthrough (*args/**kwargs) – any call is OK.
        }
    }
}
