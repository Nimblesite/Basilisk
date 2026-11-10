//! Implements [`overloads_evaluation`] from [CHKARCH-DIAG-OPTIONAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OPTIONAL
//! `overloads_evaluation`: Overload union expansion failure.
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
use crate::types::InferredType;

use super::shared::judge::TypeJudge;
use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "overloads_evaluation",
    docs_url: "https://www.basilisk-python.dev/errors/overloads_evaluation",
};

/// Emits `overloads_evaluation` when union expansion of arguments to an overloaded function
/// fails for some union member across all overloads.
pub(crate) struct OverloadUnionExpansionFailure;

impl Rule for OverloadUnionExpansionFailure {
    fn check(
        &self,
        module: &ResolvedModule,
        ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        super::check_with_own_types(self, module, ctx, diagnostics);
    }

    fn check_with_types(
        &self,
        module: &ResolvedModule,
        types: &super::shared::module_types::ModuleTypes<'_>,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Bail on parse errors — those are reported separately as BSK-0000.
        if types.annotations().is_none() {
            return;
        }
        let path = &module.path;

        // Collect overloaded function groups. A group is every `@overload`
        // stub bound to one name in the module scope — which is what Python
        // itself accumulates — so the group's own key is that name. What must
        // NOT come from a spelling is the join from a CALL to a group; see
        // `overload_group_for`.
        let mut overload_groups: HashMap<&str, Vec<&FunctionInfo>> = HashMap::new();
        // Definition site of every module-level `def` → the name it binds, so
        // a resolved callee can be turned back into its group.
        let mut definitions: HashMap<Span, &str> = HashMap::new();
        for func in &module.functions {
            if func.class_name.is_some() {
                continue;
            }
            let _ = definitions.insert(func.name_span, func.name.as_str());
            if !func.is_stub_body {
                continue;
            }
            if !func.is_overload {
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
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };

        // Walk each function definition looking for calls inside function bodies.
        let Some(resolver) = types.annotations() else {
            return;
        };
        let judge = TypeJudge::new(types.oracle(), resolver, types.nominal());
        let ctx = OverloadCtx {
            groups: &overload_groups,
            definitions: &definitions,
            bindings: &module.bindings,
        };
        for stmt in &parsed.ast.body {
            visit_stmt_for_overload_calls(&judge, stmt, path, &ctx, diagnostics);
        }
    }
}

/// The module's overload groups plus the two things needed to reach one from a
/// call: the definition site of every module-level `def`, and the bindings that
/// resolve a callee expression to one.
struct OverloadCtx<'m, 'g> {
    /// Overload stubs, grouped by the name they are bound to.
    groups: &'g HashMap<&'m str, Vec<&'m FunctionInfo>>,
    /// `def` definition site → the name it binds.
    definitions: &'g HashMap<Span, &'m str>,
    /// The module's binding table.
    bindings: &'m basilisk_resolver::BindingTable,
}

impl<'m, 'g: 'm> OverloadCtx<'m, 'g> {
    /// The overload group a callee expression targets, with the name to render
    /// in a diagnostic.
    ///
    /// The callee resolves through the binding table to the `def` it denotes —
    /// following assignment aliases, and positional, so a rebinding before the
    /// call is honoured. That definition's name selects the group.
    fn group_for(
        &self,
        callee: &ruff_python_ast::Expr,
    ) -> Option<(&'m str, &'m [&'m FunctionInfo])> {
        let site = Span::from(self.bindings.local_function_definition(callee)?);
        let name = *self.definitions.get(&site)?;
        self.groups.get(name).map(|group| (name, group.as_slice()))
    }
}

/// Walk a statement recursively to find function definitions and check their bodies.
fn visit_stmt_for_overload_calls(
    judge: &TypeJudge<'_, '_>,
    stmt: &ruff_python_ast::Stmt,
    path: &str,
    ctx: &OverloadCtx<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Stmt;

    if let Stmt::FunctionDef(func_def) = stmt {
        // Build parameter type map for this function: param_name -> annotation_text.
        let param_types = build_param_type_map(judge, func_def);

        // Walk the function body for call expressions.
        for body_stmt in &func_def.body {
            check_stmt_for_calls(judge, body_stmt, path, ctx, &param_types, diagnostics);
        }

        // Also recurse into nested function definitions.
        for body_stmt in &func_def.body {
            visit_stmt_for_overload_calls(judge, body_stmt, path, ctx, diagnostics);
        }
    } else if let Stmt::ClassDef(cls) = stmt {
        for body_stmt in &cls.body {
            visit_stmt_for_overload_calls(judge, body_stmt, path, ctx, diagnostics);
        }
    }
}

/// Build a map from parameter name to its DECLARED TYPE for a function
/// definition.
///
/// The annotation goes through the module's cascade — alias expansion,
/// same-file classes, shadowing — rather than being kept as the text between
/// the colon and the comma. That is what makes the union expansion below
/// structural: `X | Y`, `Union[X, Y]`, `Optional[X]`, and an alias that
/// expands to any of them all arrive as [`InferredType::Union`].
fn build_param_type_map(
    judge: &TypeJudge<'_, '_>,
    func_def: &ruff_python_ast::StmtFunctionDef,
) -> HashMap<String, InferredType> {
    use ruff_text_size::Ranged as _;

    let mut map = HashMap::new();
    for param_with_default in &func_def.parameters.args {
        let param = &param_with_default.parameter;
        if let Some(ann) = &param.annotation {
            if let Some(declared) = judge.declared_at(Span::from(ann.range())) {
                let _ = map.insert(param.name.to_string(), declared);
            }
        }
    }
    map
}

/// Check a statement inside a function body for calls to overloaded functions.
fn check_stmt_for_calls(
    judge: &TypeJudge<'_, '_>,
    stmt: &ruff_python_ast::Stmt,
    path: &str,
    ctx: &OverloadCtx<'_, '_>,
    param_types: &HashMap<String, InferredType>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Stmt;

    match stmt {
        Stmt::Expr(expr_stmt) => {
            check_expr_for_overload_call(
                judge,
                &expr_stmt.value,
                path,
                ctx,
                param_types,
                diagnostics,
            );
        }
        Stmt::Assign(assign) => {
            check_expr_for_overload_call(judge, &assign.value, path, ctx, param_types, diagnostics);
        }
        Stmt::AnnAssign(ann_assign) => {
            if let Some(val) = &ann_assign.value {
                check_expr_for_overload_call(judge, val, path, ctx, param_types, diagnostics);
            }
        }
        Stmt::Return(ret) => {
            if let Some(val) = &ret.value {
                check_expr_for_overload_call(judge, val, path, ctx, param_types, diagnostics);
            }
        }
        _ => {}
    }
}

/// Check a call expression to see if it is calling an overloaded function
/// with union-typed arguments that fail expansion.
fn check_expr_for_overload_call(
    judge: &TypeJudge<'_, '_>,
    expr: &ruff_python_ast::Expr,
    path: &str,
    ctx: &OverloadCtx<'_, '_>,
    param_types: &HashMap<String, InferredType>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Expr;
    use ruff_text_size::Ranged as _;

    let Expr::Call(call) = expr else {
        return;
    };

    // Which overload group this call targets is a BINDING question. The
    // deleted join read `Expr::Name.id` and looked that spelling up in the
    // group map, so `shorthand = example; shorthand(v, v, 1)` was missed
    // entirely, and a name rebound to something else was still attributed to
    // the old overload group.
    let Some((callee_name, overloads)) = ctx.group_for(&call.func) else {
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
                overload
                    .parameters
                    .get(arg_idx)
                    .and_then(|param| param.annotation_span)
                    .and_then(|ann_span| judge.declared_at(ann_span))
                    .is_some_and(|declared| judge.fits(member, &declared))
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
///
/// REBUILT on the resolved type. The deleted version took the parameter's
/// annotation TEXT and split it on the `|` CHARACTER with a bracket-depth
/// counter, so `Literal["a|b"]` split in the middle of a string literal,
/// `Union[X, Y]` and `Optional[X]` were not unions at all, and an alias that
/// expanded to one was invisible.
fn resolve_arg_union_types(
    expr: &ruff_python_ast::Expr,
    param_types: &HashMap<String, InferredType>,
) -> Option<Vec<InferredType>> {
    use ruff_python_ast::Expr;

    let Expr::Name(name) = expr else {
        return None;
    };

    match param_types.get(name.id.as_str())? {
        InferredType::Union(members) if members.len() > 1 => Some(members.clone()),
        // `Optional[T]` is `T | None`; both arms must match an overload.
        InferredType::Optional(inner) => Some(vec![inner.as_ref().clone(), InferredType::None_]),
        _ => None,
    }
}
