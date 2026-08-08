//! Implements [`generics_typevartuple_args`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `generics_typevartuple_args`: `TypeVarTuple` argument count mismatch.
//!
//! When a constructor with `TypeVarTuple` parameters is called, the number of
//! arguments must match the expected count inferred from the `TypeVarTuple`.
//!
//! ```python
//! Ts = TypeVarTuple("Ts")
//!
//! class Array(Generic[*Ts]):
//!     def __init__(self, shape: tuple[*Ts]) -> None: ...
//!
//! Array[Height, Width]((Height(1), Width(2)))  # OK
//! Array[Height, Width](Height(1))              # E: expected 2 arguments, got 1
//! ```

use std::collections::HashMap;

use basilisk_resolver::{equivalent, BindingTable, ResolvedModule, Span, TypeNode};
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged as _;

use super::Rule;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

mod star_args;

const CODE: ErrorCode = ErrorCode {
    code: "generics_typevartuple_args",
    docs_url: "https://www.basilisk-python.dev/errors/generics_typevartuple_args",
};

/// Emits `generics_typevartuple_args` when a constructor call has incorrect argument count for `TypeVarTuple`.
pub(crate) struct TypeVarTupleArgCountMismatch;

impl Rule for TypeVarTupleArgCountMismatch {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };

        // Collect TypeVarTuple names. Needed by the unpacked-tuple `*args`
        // validation to recognise the `*args: tuple[*Ts]` shared-binding form.
        let tvt_names = super::shared::typevar_tuple_names(&module.typevar_calls);

        // Unpacked-tuple `*args` validation applies with or without any
        // `TypeVarTuple` declarations in the module (the `*tuple[...]` forms need
        // none; the `tuple[*Ts]` form consults `tvt_names`).
        star_args::check_star_args_calls(
            &parsed.ast.body,
            &module.bindings,
            &module.source,
            &tvt_names,
            &module.path,
            diagnostics,
        );

        if tvt_names.is_empty() {
            return;
        }

        check_shared_tvt_call_consistency(module, &parsed.ast.body, &tvt_names, diagnostics);

        // Find classes that use TypeVarTuple in their generic params.
        let tvt_classes: HashMap<&str, &basilisk_resolver::ClassInfo> = module
            .classes
            .iter()
            .filter(|cls| {
                cls.generic_params.iter().any(|p| p.is_typevartuple)
                    || cls
                        .base_expression_names
                        .iter()
                        .any(|n| tvt_names.contains(n.as_str()))
            })
            .map(|cls| (cls.name.as_str(), cls))
            .collect();

        if tvt_classes.is_empty() {
            return;
        }

        // Collect __init__ parameter info for TypeVarTuple classes: does any
        // parameter annotation reference a `TypeVarTuple`? Decided on the AST
        // — a `Name` node whose identifier is a declared `TypeVarTuple` —
        // never on annotation text ([ASTREBUILD-LAW]).
        let mut tvt_init_info: HashMap<&str, bool> = HashMap::new();
        for stmt in &parsed.ast.body {
            let Stmt::ClassDef(cls) = stmt else {
                continue;
            };
            if !tvt_classes.contains_key(cls.name.as_str()) {
                continue;
            }
            for body_stmt in &cls.body {
                let Stmt::FunctionDef(func) = body_stmt else {
                    continue;
                };
                if func.name.as_str() != "__init__" {
                    continue;
                }
                let has_tvt_param = basilisk_resolver::iter_all_params(&func.parameters).any(
                    |param| {
                        param
                            .parameter
                            .annotation
                            .as_deref()
                            .is_some_and(|ann| expr_references_tvt(ann, &tvt_names))
                    },
                );
                let _ = tvt_init_info.insert(cls.name.as_str(), has_tvt_param);
            }
        }

        walk_stmts_for_tvt_calls(
            &parsed.ast.body,
            &tvt_classes,
            &tvt_init_info,
            &module.path,
            diagnostics,
        );
        check_bare_constructor_shapes(
            &parsed.ast.body,
            &tvt_classes,
            &tvt_init_info,
            &module.path,
            diagnostics,
        );
    }
}

/// `var: C[A1, ..., An] = C(shape)` — when `C` is generic over a `TypeVarTuple`
/// and its `__init__` takes a `tuple[*Ts]`-typed argument, the constructor
/// argument must be a tuple expression of arity `n`.
fn check_bare_constructor_shapes(
    stmts: &[Stmt],
    tvt_classes: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    tvt_init_info: &HashMap<&str, bool>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    basilisk_resolver::walk_function_stmts(stmts, &mut |stmt| {
        let Stmt::AnnAssign(ann) = stmt else { return };
        let Some(value) = ann.value.as_deref() else {
            return;
        };
        let Expr::Subscript(ann_sub) = ann.annotation.as_ref() else {
            return;
        };
        let Some(class_name) = expr_simple_name(ann_sub.value.as_ref()) else {
            return;
        };
        if !tvt_classes.contains_key(class_name)
            || !tvt_init_info.get(class_name).copied().unwrap_or(false)
        {
            return;
        }
        let Expr::Call(call) = value else { return };
        if expr_simple_name(call.func.as_ref()) != Some(class_name) {
            return;
        }
        let [single_arg] = call.arguments.args.as_ref() else {
            return;
        };
        let type_arg_count = match ann_sub.slice.as_ref() {
            Expr::Tuple(t) => t.elts.len(),
            _ => 1,
        };
        let supplied = match single_arg {
            Expr::Tuple(t) => Some(t.elts.len()),
            // A call result (e.g. `Height(1)`) is not a tuple expression.
            Expr::Call(_) => None,
            // Anything else (a variable, unpacking, ...) is not provably wrong.
            _ => return,
        };
        if supplied == Some(type_arg_count) {
            check_element_order(
                class_name,
                ann_sub.slice.as_ref(),
                single_arg,
                call,
                path,
                diagnostics,
            );
            return;
        }
        let range = call.range();
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "TypeVarTuple shape mismatch: `{class_name}` is declared with \
                 {type_arg_count} type argument{}, but the constructor argument {}",
                if type_arg_count == 1 { "" } else { "s" },
                supplied.map_or_else(
                    || "is not a tuple expression".to_owned(),
                    |n| format!("is a tuple of length {n}")
                ),
            ),
            Span {
                start: range.start().to_u32(),
                end: range.end().to_u32(),
            },
            path,
            Some(format!(
                "Pass a tuple of {type_arg_count} element{} matching the declared \
                 specialization",
                if type_arg_count == 1 { "" } else { "s" }
            )),
            None,
        ));
    });
}

/// The simple name of a declared type-argument element (`Height` → `"Height"`).
fn type_arg_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(name) => Some(name.id.as_str()),
        _ => None,
    }
}

/// The simple name of a constructor-tuple element: the callee of `Height(1)` or
/// a bare `Height` (`"Height"`).
fn ctor_elt_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Call(call) => expr_simple_name(call.func.as_ref()),
        Expr::Name(name) => Some(name.id.as_str()),
        _ => None,
    }
}

/// When the constructor tuple has the right arity but its element types are a
/// *permutation* of the declared specialization (`Array[A, B] = Array((B(), A()))`),
/// the dimensions are out of order. Only a pure reordering is flagged — a
/// differing/subtype element name is left alone to avoid false positives.
fn check_element_order(
    class_name: &str,
    decl_slice: &Expr,
    ctor_arg: &Expr,
    call: &ruff_python_ast::ExprCall,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (Expr::Tuple(decl), Expr::Tuple(ctor)) = (decl_slice, ctor_arg) else {
        return;
    };
    let Some(declared) = decl
        .elts
        .iter()
        .map(type_arg_name)
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    let Some(got) = ctor
        .elts
        .iter()
        .map(ctor_elt_name)
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    if declared.len() != got.len() || declared == got {
        return;
    }
    let (mut sorted_declared, mut sorted_got) = (declared.clone(), got.clone());
    sorted_declared.sort_unstable();
    sorted_got.sort_unstable();
    if sorted_declared != sorted_got {
        return; // not a permutation — could be a subtype; stay conservative
    }
    let range = call.range();
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "TypeVarTuple element order mismatch: `{class_name}` is declared `[{}]` but the \
             constructor provides `[{}]`",
            declared.join(", "),
            got.join(", ")
        ),
        Span {
            start: range.start().to_u32(),
            end: range.end().to_u32(),
        },
        path,
        Some("Reorder the constructor arguments to match the declared specialization".to_owned()),
        None,
    ));
}

/// When a function binds the same `TypeVarTuple` in several parameters
/// (`def f(a: tuple[*Ts], b: tuple[*Ts])`), every call must bind it
/// identically: tuple-literal arguments must have equal lengths, and
/// parameter-reference arguments must carry provably-equivalent annotations.
///
/// A parameter participates when its annotation is exactly the resolved
/// tuple form subscripted by one unpacked `TypeVarTuple` — decided through
/// the binding table and the AST, never annotation text ([ASTREBUILD-LAW]).
/// Annotations that merely mention a `TypeVarTuple` in a wider shape
/// (`tuple[int, *Ts]`) are outside this check's model and abstain
/// ([ASTREBUILD-PHASE-RESOLVER]).
fn check_shared_tvt_call_consistency(
    module: &ResolvedModule,
    stmts: &[Stmt],
    tvt_names: &std::collections::HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Module-level functions binding the same `TypeVarTuple` in two or more
    // positional parameters: name → positions of those parameters.
    let mut shared: HashMap<&str, Vec<usize>> = HashMap::new();
    for stmt in stmts {
        let Stmt::FunctionDef(func) = stmt else {
            continue;
        };
        let mut by_tvt: HashMap<&str, Vec<usize>> = HashMap::new();
        let positional = func
            .parameters
            .posonlyargs
            .iter()
            .chain(func.parameters.args.iter());
        for (idx, param) in positional.enumerate() {
            let Some(ann) = param.parameter.annotation.as_deref() else {
                continue;
            };
            if let Some(tvt) = star_args::shared_tvt_name(&module.bindings, ann, tvt_names) {
                by_tvt.entry(tvt).or_default().push(idx);
            }
        }
        if let Some(positions) = by_tvt.into_values().find(|p| p.len() >= 2) {
            let _ = shared.insert(func.name.as_str(), positions);
        }
    }
    if shared.is_empty() {
        return;
    }

    let scope = HashMap::new();
    walk_calls_with_scope(
        stmts,
        &scope,
        &shared,
        &module.bindings,
        &module.path,
        diagnostics,
    );
}

/// Parameter-name → resolved annotation scope for one function.
type ParamScope = HashMap<String, TypeNode>;

/// Walk statements tracking the enclosing function's parameter annotations
/// (lowered through the binding table), checking shared-`TypeVarTuple` calls
/// in expression positions.
fn walk_calls_with_scope(
    stmts: &[Stmt],
    scope: &ParamScope,
    shared: &HashMap<&str, Vec<usize>>,
    bindings: &BindingTable,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDef(func) => {
                let inner: ParamScope = basilisk_resolver::iter_all_params(&func.parameters)
                    .filter_map(|p| {
                        p.parameter.annotation.as_deref().map(|ann| {
                            (
                                p.parameter.name.to_string(),
                                TypeNode::lower(bindings, ann),
                            )
                        })
                    })
                    .collect();
                walk_calls_with_scope(&func.body, &inner, shared, bindings, path, diagnostics);
            }
            Stmt::ClassDef(cls) => {
                walk_calls_with_scope(&cls.body, scope, shared, bindings, path, diagnostics);
            }
            Stmt::If(if_stmt) => {
                walk_calls_with_scope(&if_stmt.body, scope, shared, bindings, path, diagnostics);
                for clause in &if_stmt.elif_else_clauses {
                    walk_calls_with_scope(
                        &clause.body,
                        scope,
                        shared,
                        bindings,
                        path,
                        diagnostics,
                    );
                }
            }
            Stmt::For(for_stmt) => {
                walk_calls_with_scope(&for_stmt.body, scope, shared, bindings, path, diagnostics);
            }
            Stmt::While(while_stmt) => {
                walk_calls_with_scope(
                    &while_stmt.body,
                    scope,
                    shared,
                    bindings,
                    path,
                    diagnostics,
                );
            }
            Stmt::Expr(node) => scan_expr_calls(&node.value, scope, shared, path, diagnostics),
            Stmt::Assign(node) => scan_expr_calls(&node.value, scope, shared, path, diagnostics),
            Stmt::AnnAssign(node) => {
                if let Some(value) = node.value.as_deref() {
                    scan_expr_calls(value, scope, shared, path, diagnostics);
                }
            }
            Stmt::Return(node) => {
                if let Some(value) = node.value.as_deref() {
                    scan_expr_calls(value, scope, shared, path, diagnostics);
                }
            }
            _ => {}
        }
    }
}

/// Recursively check call expressions (including nested call arguments).
fn scan_expr_calls(
    expr: &Expr,
    scope: &ParamScope,
    shared: &HashMap<&str, Vec<usize>>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Expr::Call(call) = expr else { return };
    if let Some(callee) = expr_simple_name(call.func.as_ref()) {
        if let Some(positions) = shared.get(callee) {
            check_call_binding_consistency(call, positions, scope, callee, path, diagnostics);
        }
    }
    for arg in &call.arguments.args {
        scan_expr_calls(arg, scope, shared, path, diagnostics);
    }
}

/// Validate one call against the shared-`TypeVarTuple` parameter positions.
fn check_call_binding_consistency(
    call: &ruff_python_ast::ExprCall,
    positions: &[usize],
    scope: &ParamScope,
    callee: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let bindings: Vec<Binding> = positions
        .iter()
        .filter_map(|&pos| call.arguments.args.get(pos).map(arg_binding))
        .collect();
    if bindings.len() < 2 {
        return;
    }
    let consistent = bindings.windows(2).all(|pair| match pair {
        [first, second] => binding_matches(first, second, scope),
        _ => true,
    });
    if consistent {
        return;
    }
    let range = call.range();
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "TypeVarTuple binding mismatch: arguments to `{callee}` must bind the \
             shared TypeVarTuple to the same types"
        ),
        Span {
            start: range.start().to_u32(),
            end: range.end().to_u32(),
        },
        path,
        Some(
            "When the same TypeVarTuple appears in multiple parameters, the type \
             parameters must be identical across arguments"
                .to_owned(),
        ),
        None,
    ));
}

/// How a call argument binds a `TypeVarTuple`.
enum Binding {
    /// A tuple literal of the given length.
    TupleLen(usize),
    /// A name reference (resolved via the enclosing function's parameters).
    Name(String),
    /// Not analyzable.
    Opaque,
}

fn arg_binding(arg: &Expr) -> Binding {
    match arg {
        Expr::Tuple(t) => Binding::TupleLen(t.elts.len()),
        Expr::Name(n) => Binding::Name(n.id.to_string()),
        _ => Binding::Opaque,
    }
}

/// `true` when two bindings are provably consistent (or not provably wrong).
///
/// Two name references are inconsistent only when the relation REFUTES the
/// equivalence of their resolved annotations; an abstention (`None`) — e.g.
/// annotations with unresolved parts — never produces a verdict
/// ([ASTREBUILD-LAW]).
fn binding_matches(a: &Binding, b: &Binding, scope: &ParamScope) -> bool {
    match (a, b) {
        (Binding::TupleLen(la), Binding::TupleLen(lb)) => la == lb,
        (Binding::Name(na), Binding::Name(nb)) => {
            if na == nb {
                return true;
            }
            match (scope.get(na), scope.get(nb)) {
                (Some(node_a), Some(node_b)) => equivalent(node_a, node_b) != Some(false),
                _ => true,
            }
        }
        _ => true,
    }
}

/// Walk statements looking for calls to TypeVarTuple-specialized classes.
fn walk_stmts_for_tvt_calls(
    stmts: &[Stmt],
    tvt_classes: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    tvt_init_info: &HashMap<&str, bool>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Expr(expr_stmt) => {
                check_expr_for_tvt_call(
                    &expr_stmt.value,
                    tvt_classes,
                    tvt_init_info,
                    path,
                    diagnostics,
                );
            }
            Stmt::Assign(assign) => {
                check_expr_for_tvt_call(
                    &assign.value,
                    tvt_classes,
                    tvt_init_info,
                    path,
                    diagnostics,
                );
            }
            Stmt::AnnAssign(ann_assign) => {
                if let Some(value) = &ann_assign.value {
                    check_expr_for_tvt_call(value, tvt_classes, tvt_init_info, path, diagnostics);
                }
            }
            Stmt::FunctionDef(func_def) => {
                walk_stmts_for_tvt_calls(
                    &func_def.body,
                    tvt_classes,
                    tvt_init_info,
                    path,
                    diagnostics,
                );
            }
            Stmt::ClassDef(class_def) => {
                walk_stmts_for_tvt_calls(
                    &class_def.body,
                    tvt_classes,
                    tvt_init_info,
                    path,
                    diagnostics,
                );
            }
            Stmt::If(if_stmt) => {
                walk_stmts_for_tvt_calls(
                    &if_stmt.body,
                    tvt_classes,
                    tvt_init_info,
                    path,
                    diagnostics,
                );
                for clause in &if_stmt.elif_else_clauses {
                    walk_stmts_for_tvt_calls(
                        &clause.body,
                        tvt_classes,
                        tvt_init_info,
                        path,
                        diagnostics,
                    );
                }
            }
            Stmt::For(for_stmt) => {
                walk_stmts_for_tvt_calls(
                    &for_stmt.body,
                    tvt_classes,
                    tvt_init_info,
                    path,
                    diagnostics,
                );
            }
            Stmt::While(while_stmt) => {
                walk_stmts_for_tvt_calls(
                    &while_stmt.body,
                    tvt_classes,
                    tvt_init_info,
                    path,
                    diagnostics,
                );
            }
            _ => {}
        }
    }
}

/// Check an expression for a call to a TypeVarTuple-specialized class.
fn check_expr_for_tvt_call(
    expr: &Expr,
    tvt_classes: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    tvt_init_info: &HashMap<&str, bool>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Look for `ClassName[T1, T2, ...](args...)` pattern.
    let Expr::Call(call) = expr else {
        return;
    };

    let Expr::Subscript(sub) = call.func.as_ref() else {
        return;
    };

    let Some(class_name) = expr_simple_name(sub.value.as_ref()) else {
        return;
    };

    if !tvt_classes.contains_key(class_name) {
        return;
    }

    // Only check if the class __init__ has a TypeVarTuple-related param.
    if !tvt_init_info.get(class_name).copied().unwrap_or(false) {
        return;
    }

    // Count the type arguments in the specialization.
    let type_arg_count = match sub.slice.as_ref() {
        Expr::Tuple(t) => t.elts.len(),
        _ => 1,
    };

    // Count the positional call arguments.
    let call_arg_count = call.arguments.args.len();

    // For a TypeVarTuple class with `tuple[*Ts]` param, if the class is
    // specialized with N type args, the constructor should receive N positional
    // arguments (one per TypeVarTuple element) OR a single tuple argument.
    // If the call has fewer args than type args and isn't passing a single arg,
    // flag it.
    if call_arg_count < type_arg_count && call_arg_count > 0 {
        let range = call.range();
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "TypeVarTuple argument count mismatch: `{class_name}` specialized with \
                 {type_arg_count} type arguments, but constructor received {call_arg_count}"
            ),
            Span {
                start: range.start().to_u32(),
                end: range.end().to_u32(),
            },
            path,
            Some(format!(
                "Provide {type_arg_count} arguments matching the type specialization"
            )),
            Some(
                "When a class uses `TypeVarTuple`, the constructor arguments must match \
                 the number of type arguments in the specialization"
                    .to_owned(),
            ),
        ));
    }
}

/// Extract a simple name from an expression.
fn expr_simple_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(name) => Some(name.id.as_str()),
        _ => None,
    }
}

/// Does a type expression reference one of the module's `TypeVarTuple`s?
///
/// Walks the annotation's AST looking for a `Name` node whose identifier is a
/// declared `TypeVarTuple` — the structural positions a type expression can
/// carry one in (subscript slices, starred unpacks, unions, tuples) — never
/// the annotation's source text ([ASTREBUILD-LAW]).
fn expr_references_tvt(expr: &Expr, tvt_names: &std::collections::HashSet<&str>) -> bool {
    match expr {
        Expr::Name(name) => tvt_names.contains(name.id.as_str()),
        Expr::Starred(starred) => expr_references_tvt(&starred.value, tvt_names),
        Expr::Subscript(sub) => {
            expr_references_tvt(&sub.value, tvt_names) || expr_references_tvt(&sub.slice, tvt_names)
        }
        Expr::Tuple(tuple) => tuple
            .elts
            .iter()
            .any(|element| expr_references_tvt(element, tvt_names)),
        Expr::BinOp(op) => {
            expr_references_tvt(&op.left, tvt_names) || expr_references_tvt(&op.right, tvt_names)
        }
        _ => false,
    }
}
