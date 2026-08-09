//! Implements [`dataclasses_postinit`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `dataclasses_postinit`: `InitVar` field validation in dataclasses.
//!
//! Detects two categories of `InitVar` violations:
//!
//! 1. **`__post_init__` signature mismatch**: A dataclass with `InitVar` fields
//!    must declare a `__post_init__` method whose parameters (after `self`)
//!    match the `InitVar` fields in count.
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
//!     def __post_init__(self, x: int) -> None:  # E: one parameter, two fields
//!         pass
//!
//! dc1 = DC1(1, "")
//! dc1.x  # E: cannot access InitVar field as attribute
//! ```

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "dataclasses_postinit",
    docs_url: "https://www.basilisk-python.dev/errors/dataclasses_postinit",
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

/// Emits `dataclasses_postinit` for `InitVar` field violations in dataclasses.
pub(crate) struct InitVarViolation;

impl Rule for InitVarViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        check_post_init_signatures(module, diagnostics);
        check_initvar_attribute_access(module, diagnostics);
    }
}

// ##########################################################################
// # DELETED BODY — `check_post_init_signatures`. DO NOT RESTORE IT AND DO  #
// # NOT REPLACE IT WITH A PLACEHOLDER THAT RETURNS WITHOUT CHECKING.       #
// #                                                                        #
// # The rule's gate — "does a base class already declare `InitVar` fields, #
// # so this subclass legitimately inherits them?" — was answered by        #
// # looking the base up by its SOURCE TEXT:                                #
// #                                                                        #
// #   let base_name = base_expr.split('[').next().unwrap_or(base).trim();  #
// #   class_map.get(base_name)                                             #
// #                                                                        #
// # A base written `Parent [T]`, reached under an alias, or merely sharing #
// # a rendered name with an unrelated class produced the wrong answer, and #
// # the wrong answer here decides whether the rule fires at all. That is   #
// # not a detail inside a working rule; it IS the rule.                    #
// #                                                                        #
// # `InitVar` inheritance is a question about resolved base classes and    #
// # the fields they declare. Ask the binding table.                        #
// #                                                                        #
// # Pinned by: tests/no_type_spelling_surgery_tests.rs                     #
// ##########################################################################

/// DELETED — panics. The signature survives only so its caller stays visible
/// as the rebuild map; see the banner above.
fn check_post_init_signatures(_module: &ResolvedModule, _diagnostics: &mut Vec<Diagnostic>) {
    panic!(
        "basilisk-checker: `check_post_init_signatures` was DELETED because its \
         inherited-`InitVar` gate looked a base class up by splitting its SOURCE TEXT \
         at `[`. It panics because the real implementation — resolving each base \
         expression to a class symbol through the binding table — DOES NOT EXIST YET. \
         Do not restore the split and do not return without checking in its place."
    )
}

/// Check for access to `InitVar` fields as instance attributes at module level.
fn check_initvar_attribute_access(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
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

    let Some(parsed) = super::shared::parse_module(module) else {
        return;
    };
    let index = super::shared::ExprIndex::build(&parsed.ast);

    // Map module variables to the dataclass they instantiate. The callee is
    // identified from the AST call node behind the resolver's RHS span and
    // must be a bare name referring to this module's own class definition —
    // never a parse of the right-hand side's source text ([ASTREBUILD-LAW]).
    let var_class_map: HashMap<&str, &str> = module
        .module_vars
        .iter()
        .filter(|v| v.rhs_kind == basilisk_resolver::RhsKind::CallExpr)
        .filter_map(|v| {
            let rhs_span = v.rhs_span?;
            let Some(ruff_python_ast::Expr::Call(call)) = index.expr(rhs_span) else {
                return None;
            };
            let ruff_python_ast::Expr::Name(callee) = call.func.as_ref() else {
                return None;
            };
            if !module.bindings.refers_to_local_definition(call.func.as_ref()) {
                return None;
            }
            let class_name = callee.id.as_str();
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
