//! BSK-E0085: `TypeVarTuple` argument count mismatch.
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

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{ResolvedModule, Span};
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged as _;

use super::Rule;
use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0085",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0085",
};

/// Emits BSK-E0085 when a constructor call has incorrect argument count for `TypeVarTuple`.
pub(crate) struct TypeVarTupleArgCountMismatch;

impl Rule for TypeVarTupleArgCountMismatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Collect TypeVarTuple names.
        let tvt_names: HashSet<&str> = module
            .typevar_calls
            .iter()
            .filter(|tv| tv.is_typevartuple)
            .map(|tv| tv.name.as_str())
            .collect();

        if tvt_names.is_empty() {
            return;
        }

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

        // Collect __init__ parameter info for TypeVarTuple classes.
        // We need to know which params use `tuple[*Ts]`.
        let mut tvt_init_info: HashMap<&str, bool> = HashMap::new();
        for func in &module.functions {
            let Some(ref class_name) = func.class_name else {
                continue;
            };
            if !tvt_classes.contains_key(class_name.as_str()) {
                continue;
            }
            if func.name == "__init__" {
                // Check if any parameter annotation contains a TypeVarTuple reference.
                let has_tvt_param = func.parameters.iter().any(|p| {
                    if let Some(ref ann_span) = p.annotation_span {
                        let ann_text = module.ann_span.slice_source(source).unwrap_or("");
                        tvt_names.iter().any(|tvt| ann_text.contains(tvt))
                    } else {
                        false
                    }
                });
                let _ = tvt_init_info.insert(class_name.as_str(), has_tvt_param);
            }
        }

        // Re-parse to walk the AST for call expressions.
        let Ok(parsed) = basilisk_parser::parse_source(module.source.clone(), module.path.clone())
        else {
            return;
        };

        walk_stmts_for_tvt_calls(
            &parsed.ast.body,
            &tvt_classes,
            &tvt_init_info,
            &module.path,
            diagnostics,
        );
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
        diagnostics.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "TypeVarTuple argument count mismatch: `{class_name}` specialized with \
                 {type_arg_count} type arguments, but constructor received {call_arg_count}"
            ),
            span: Span {
                start: range.start().to_u32(),
                end: range.end().to_u32(),
            },
            path: path.to_owned(),
            help: Some(format!(
                "Provide {type_arg_count} arguments matching the type specialization"
            )),
            note: Some(
                "When a class uses `TypeVarTuple`, the constructor arguments must match \
                 the number of type arguments in the specialization"
                    .to_owned(),
            ),
        });
    }
}

/// Extract a simple name from an expression.
fn expr_simple_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(name) => Some(name.id.as_str()),
        _ => None,
    }
}
