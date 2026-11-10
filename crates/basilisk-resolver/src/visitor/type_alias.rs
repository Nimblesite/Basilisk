//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Type Alias visitor functions.

use ruff_python_ast::{Expr, Stmt, TypeParam};
use ruff_text_size::Ranged;

use crate::scope::{Span, TypeAliasDefInfo, TypeStatementInfo};

use super::class_info_ext::expr_simple_name;
use super::core::text_range_to_span;
use super::function_info::{collect_name_refs_from_expr, collect_string_refs_from_expr};

pub(super) fn type_param_name(tp: &TypeParam) -> String {
    match tp {
        TypeParam::TypeVar(tv) => tv.name.to_string(),
        TypeParam::TypeVarTuple(tvt) => tvt.name.to_string(),
        TypeParam::ParamSpec(ps) => ps.name.to_string(),
    }
}

pub(super) fn collect_type_alias_defs(stmts: &[Stmt]) -> Vec<TypeAliasDefInfo> {
    let mut out = Vec::new();
    for stmt in stmts {
        // Handle bare `Name = Expr[...]` assignments (implicit type aliases).
        if let Stmt::Assign(assign) = stmt {
            if assign.targets.len() != 1 {
                continue;
            }
            let Some(first_target) = assign.targets.first() else {
                continue;
            };
            let Some(name) = expr_simple_name(first_target) else {
                continue;
            };
            // Only treat subscript RHS as implicit alias (Name = Something[...])
            if matches!(assign.value.as_ref(), Expr::Subscript(_)) {
                out.push(build_type_alias_info(name, &assign.value, stmt));
            }
        }
    }
    out
}

/// Helper to build a `TypeAliasDefInfo` from an alias name and RHS expression.
pub(super) fn build_type_alias_info(name: String, rhs: &Expr, stmt: &Stmt) -> TypeAliasDefInfo {
    let mut rhs_names = Vec::new();
    collect_name_refs_from_expr(rhs, &mut rhs_names);

    let (rhs_base_name, rhs_type_arg_names) = match rhs {
        Expr::Subscript(sub) => {
            let base = expr_simple_name(&sub.value);
            let arg_names = match sub.slice.as_ref() {
                Expr::Tuple(tup) => tup.elts.iter().filter_map(expr_simple_name).collect(),
                single => expr_simple_name(single).into_iter().collect(),
            };
            (base, arg_names)
        }
        _ => (None, Vec::new()),
    };

    let mut rhs_string_refs = Vec::new();
    collect_string_refs_from_expr(rhs, &mut rhs_string_refs);

    let span = Span::from(stmt.range());

    TypeAliasDefInfo {
        name,
        rhs_names,
        rhs_base_name,
        rhs_type_arg_names,
        rhs_string_refs,
        span,
    }
}

pub(super) fn collect_type_statements(stmts: &[Stmt]) -> Vec<TypeStatementInfo> {
    let mut out = Vec::new();
    for stmt in stmts {
        match stmt {
            Stmt::TypeAlias(ta) => {
                if let Some(name_str) = expr_simple_name(&ta.name) {
                    out.push(TypeStatementInfo {
                        name: name_str,
                        rhs_span: text_range_to_span(ta.value.range()),
                        name_span: text_range_to_span(ta.name.range()),
                        param_names: ta
                            .type_params
                            .as_deref()
                            .map(|tps| tps.type_params.iter().map(type_param_name).collect())
                            .unwrap_or_default(),
                    });
                }
            }
            Stmt::ClassDef(cls) => out.extend(collect_type_statements(&cls.body)),
            Stmt::FunctionDef(func) => out.extend(collect_type_statements(&func.body)),
            _ => {}
        }
    }
    out
}
