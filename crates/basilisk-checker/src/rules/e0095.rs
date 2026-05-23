//! BSK-E0095: `InitVar` field validation in dataclasses.
//!
//! Detects two categories of `InitVar` violations:
//!
//! 1. **`__post_init__` signature mismatch**: A dataclass with `InitVar` fields
//!    must declare a `__post_init__` method whose parameters (after `self`)
//!    match the `InitVar` fields in count and type.
//!
//! 2. **Access to `InitVar` fields as instance attributes**: `InitVar[T]` fields
//!    are constructor-only parameters passed to `__post_init__`; they are not
//!    stored as instance attributes and cannot be accessed as `instance.field`.
//!
//! ```python
//! from dataclasses import InitVar, dataclass
//!
//! @dataclass
//! class DC1:
//!     x: InitVar[int]
//!     y: InitVar[str]
//!
//!     def __post_init__(self, x: int, y: int) -> None:  # E: y should be str
//!         pass
//!
//! dc1 = DC1(1, "")
//! dc1.x  # E: cannot access InitVar field as attribute
//! ```

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::shared::extract_callee_name;
use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0095",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0095",
};

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    error_diagnostic(
        CODE.clone(),
        message,
        span,
        path,
        Some("`InitVar` fields are constructor-only parameters, not instance attributes"),
        Some(
            "PEP 557: `InitVar[T]` fields are passed to `__post_init__` and not stored on the instance",
        ),
    )
}

fn span_text(source: &str, span: Option<Span>) -> Option<&str> {
    let span = span?;
    slice_span(source, span)
}

/// Extract the inner type from an `InitVar[T]` annotation text.
/// Returns `Some("T")` for `InitVar[T]` or `dataclasses.InitVar[T]`.
fn extract_initvar_inner(ann: &str) -> Option<&str> {
    let s = ann.trim();
    let rest = s
        .strip_prefix("InitVar[")
        .or_else(|| s.strip_prefix("dataclasses.InitVar["))?;
    rest.strip_suffix(']').map(str::trim)
}

/// Emits BSK-E0095 for `InitVar` field violations in dataclasses.
pub(crate) struct InitVarViolation;

impl Rule for InitVarViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        check_post_init_signatures(module, diagnostics);
        check_initvar_attribute_access(module, diagnostics);
    }
}

/// Validate `__post_init__` signatures against the class's `InitVar` fields.
fn check_post_init_signatures(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    let source = &module.source;
    let path = &module.path;

    let class_map: HashMap<&str, _> = module
        .classes
        .iter()
        .map(|c| (c.name.as_str(), c))
        .collect();

    for cls in &module.classes {
        if !cls.is_dataclass {
            continue;
        }

        let initvar_fields: Vec<_> = cls.attributes.iter().filter(|a| a.is_init_var).collect();
        if initvar_fields.is_empty() {
            continue;
        }

        // Skip classes that inherit InitVar fields to avoid false positives.
        let has_parent_with_initvar = cls.bases.iter().any(|base_expr| {
            let base_name = base_expr.split('[').next().unwrap_or(base_expr).trim();
            class_map
                .get(base_name)
                .is_some_and(|base_cls| base_cls.attributes.iter().any(|a| a.is_init_var))
        });
        if has_parent_with_initvar {
            continue;
        }

        let post_init = module.functions.iter().find(|f| {
            f.class_name.as_deref() == Some(cls.name.as_str()) && f.name == "__post_init__"
        });

        let Some(post_init_fn) = post_init else {
            continue;
        };

        let params: Vec<_> = post_init_fn.parameters.iter().skip(1).collect();

        if params.len() != initvar_fields.len() {
            diagnostics.push(make_diagnostic(
                format!(
                    "`__post_init__` in `{}` has {} parameter{} after `self`, \
                     but {} `InitVar` field{} declared",
                    cls.name,
                    params.len(),
                    if params.len() == 1 { "" } else { "s" },
                    initvar_fields.len(),
                    if initvar_fields.len() == 1 { "" } else { "s" },
                ),
                post_init_fn.name_span,
                path,
            ));
            continue;
        }

        for (param, field) in params.iter().zip(initvar_fields.iter()) {
            let Some(field_ann) = span_text(source, field.annotation_span) else {
                continue;
            };
            let Some(inner_type) = extract_initvar_inner(field_ann.trim()) else {
                continue;
            };
            let Some(param_ann) = span_text(source, param.annotation_span) else {
                continue;
            };
            if inner_type != param_ann.trim() {
                diagnostics.push(make_diagnostic(
                    format!(
                        "`__post_init__` parameter `{}` has type `{}` but `InitVar` field \
                         `{}` declares inner type `{}`",
                        param.name,
                        param_ann.trim(),
                        field.name,
                        inner_type,
                    ),
                    param.name_span,
                    path,
                ));
            }
        }
    }
}

/// Check for access to `InitVar` fields as instance attributes at module level.
fn check_initvar_attribute_access(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    let source = &module.source;
    let path = &module.path;

    let initvar_field_map: HashMap<&str, HashSet<&str>> = module
        .classes
        .iter()
        .filter(|c| c.is_dataclass)
        .filter_map(|c| {
            let names: HashSet<&str> =
                basilisk_resolver::collect_name_set_where(&c.attributes, |a| a.is_init_var);
            if names.is_empty() {
                None
            } else {
                Some((c.name.as_str(), names))
            }
        })
        .collect();

    if initvar_field_map.is_empty() {
        return;
    }

    let var_class_map: HashMap<&str, &str> = module
        .module_vars
        .iter()
        .filter(|v| v.rhs_kind == basilisk_resolver::RhsKind::CallExpr)
        .filter_map(|v| {
            let rhs_span = v.rhs_span?;
            let rhs_text = slice_span(source, rhs_span)?;
            let class_name = extract_callee_name(rhs_text)?;
            if initvar_field_map.contains_key(class_name) {
                Some((v.name.as_str(), class_name))
            } else {
                None
            }
        })
        .collect();

    if var_class_map.is_empty() {
        return;
    }

    let Ok(parsed) = basilisk_parser::parse_source(source.clone(), path.clone()) else {
        return;
    };

    for stmt in &parsed.ast.body {
        check_stmt_for_initvar_access(stmt, path, &var_class_map, &initvar_field_map, diagnostics);
    }
}

/// Recursively walk a statement looking for `InitVar` attribute accesses.
fn check_stmt_for_initvar_access(
    stmt: &ruff_python_ast::Stmt,
    path: &str,
    var_class_map: &HashMap<&str, &str>,
    initvar_field_map: &HashMap<&str, HashSet<&str>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Stmt;

    match stmt {
        Stmt::Expr(node) => {
            check_expr_for_initvar_access(
                &node.value,
                path,
                var_class_map,
                initvar_field_map,
                diagnostics,
            );
        }
        Stmt::If(if_stmt) => {
            check_expr_for_initvar_access(
                &if_stmt.test,
                path,
                var_class_map,
                initvar_field_map,
                diagnostics,
            );
            for s in &if_stmt.body {
                check_stmt_for_initvar_access(
                    s,
                    path,
                    var_class_map,
                    initvar_field_map,
                    diagnostics,
                );
            }
            for clause in &if_stmt.elif_else_clauses {
                if let Some(test) = &clause.test {
                    check_expr_for_initvar_access(
                        test,
                        path,
                        var_class_map,
                        initvar_field_map,
                        diagnostics,
                    );
                }
                for s in &clause.body {
                    check_stmt_for_initvar_access(
                        s,
                        path,
                        var_class_map,
                        initvar_field_map,
                        diagnostics,
                    );
                }
            }
        }
        Stmt::Assign(assign) => {
            check_expr_for_initvar_access(
                &assign.value,
                path,
                var_class_map,
                initvar_field_map,
                diagnostics,
            );
        }
        Stmt::AnnAssign(ann_assign) => {
            if let Some(value) = &ann_assign.value {
                check_expr_for_initvar_access(
                    value,
                    path,
                    var_class_map,
                    initvar_field_map,
                    diagnostics,
                );
            }
        }
        _ => {}
    }
}

/// Collect child expressions that need recursive `InitVar` access checking.
fn collect_child_exprs(expr: &ruff_python_ast::Expr) -> Vec<&ruff_python_ast::Expr> {
    use ruff_python_ast::Expr;

    match expr {
        Expr::Call(call) => {
            let mut children = vec![call.func.as_ref()];
            children.extend(call.arguments.args.iter());
            children.extend(call.arguments.keywords.iter().map(|kw| &kw.value));
            children
        }
        Expr::Tuple(tup) => tup.elts.iter().collect(),
        Expr::List(lst) => lst.elts.iter().collect(),
        Expr::BinOp(bin) => vec![&bin.left, &bin.right],
        _ => vec![],
    }
}

/// Recursively walk an expression looking for `var.attr` where `attr` is an `InitVar` field.
fn check_expr_for_initvar_access(
    expr: &ruff_python_ast::Expr,
    path: &str,
    var_class_map: &HashMap<&str, &str>,
    initvar_field_map: &HashMap<&str, HashSet<&str>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Expr;
    use ruff_text_size::Ranged as _;

    if let Expr::Attribute(attr_expr) = expr {
        if let Expr::Name(obj_name) = attr_expr.value.as_ref() {
            let var_name = obj_name.id.as_str();
            if let Some(&class_name) = var_class_map.get(var_name) {
                if let Some(initvar_fields) = initvar_field_map.get(class_name) {
                    let attr_name = attr_expr.attr.as_str();
                    if initvar_fields.contains(attr_name) {
                        let range = expr.range();
                        let span = Span {
                            start: range.start().to_u32(),
                            end: range.end().to_u32(),
                        };
                        diagnostics.push(make_diagnostic(
                            format!(
                                "Cannot access `InitVar` field `{attr_name}` on `{var_name}`: \
                                 `InitVar` fields are not stored as instance attributes"
                            ),
                            span,
                            path,
                        ));
                        return;
                    }
                }
            }
        }
        check_expr_for_initvar_access(
            &attr_expr.value,
            path,
            var_class_map,
            initvar_field_map,
            diagnostics,
        );
        return;
    }

    for child in collect_child_exprs(expr) {
        check_expr_for_initvar_access(child, path, var_class_map, initvar_field_map, diagnostics);
    }
}
