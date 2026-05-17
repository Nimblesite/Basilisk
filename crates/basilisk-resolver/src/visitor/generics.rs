//! Generics visitor functions.

use ruff_python_ast::{Expr, Stmt, StmtClassDef, TypeParam};
use ruff_text_size::Ranged;

use crate::scope::{GenericParamInfo, GenericSubscriptSite, Pep695BoundViolation, Span};

use super::class_info_ext::expr_simple_name;
use super::core::text_range_to_span;
use super::type_alias::type_param_name;
use super::typevar::check_typevar_bound_expr;

pub(super) fn extract_generic_params(class: &StmtClassDef) -> (Vec<GenericParamInfo>, Vec<Span>) {
    let args = match class.arguments.as_ref() {
        Some(a) => &a.args,
        None => return (Vec::new(), Vec::new()),
    };
    for base in args {
        let Expr::Subscript(sub) = base else { continue };
        let is_generic_or_protocol = matches!(sub.value.as_ref(), Expr::Name(n) if n.id.as_str() == "Generic" || n.id.as_str() == "Protocol")
            || matches!(sub.value.as_ref(), Expr::Attribute(a) if a.attr.as_str() == "Generic" || a.attr.as_str() == "Protocol");
        if !is_generic_or_protocol {
            continue;
        }
        let elts: &[Expr] = match sub.slice.as_ref() {
            Expr::Tuple(tuple) => &tuple.elts,
            other => std::slice::from_ref(other),
        };
        let mut params = Vec::new();
        let mut non_typevar = Vec::new();
        for e in elts {
            // A starred expression like `*Ts` is a valid TypeVarTuple unpack.
            let (name_opt, is_typevartuple) = if let Expr::Starred(starred) = e {
                (expr_simple_name(&starred.value), true)
            } else {
                (expr_simple_name(e), false)
            };
            match name_opt {
                Some(name) => params.push(GenericParamInfo {
                    span: text_range_to_span(e.range()),
                    name,
                    is_typevartuple,
                }),
                None => non_typevar.push(text_range_to_span(e.range())),
            }
        }
        return (params, non_typevar);
    }
    (Vec::new(), Vec::new())
}

pub(super) fn collect_pep695_bound_violations(stmts: &[Stmt]) -> Vec<Pep695BoundViolation> {
    let bare_names: std::collections::HashSet<String> = stmts
        .iter()
        .filter_map(|stmt| {
            let Stmt::Assign(node) = stmt else {
                return None;
            };
            node.targets.first().and_then(expr_simple_name)
        })
        .collect();

    let mut out = Vec::new();
    collect_pep695_violations_from_stmts(
        stmts,
        &bare_names,
        &std::collections::HashSet::new(),
        &mut out,
    );
    out
}

pub(super) fn collect_pep695_violations_from_stmts(
    stmts: &[Stmt],
    bare_names: &std::collections::HashSet<String>,
    outer_typeparams: &std::collections::HashSet<String>,
    out: &mut Vec<Pep695BoundViolation>,
) {
    for stmt in stmts {
        let Stmt::ClassDef(cls) = stmt else {
            continue;
        };
        let class_name = cls.name.to_string();

        // Collect the current class's TypeParam names.
        let current_typeparams: std::collections::HashSet<String> = cls
            .type_params
            .as_ref()
            .map(|tp| tp.type_params.iter().map(type_param_name).collect())
            .unwrap_or_default();

        if let Some(type_params) = &cls.type_params {
            for tp in &type_params.type_params {
                if let TypeParam::TypeVar(tv) = tp {
                    if let Some(bound) = &tv.bound {
                        check_typevar_bound_expr(
                            bound,
                            &class_name,
                            tv.name.as_str(),
                            bare_names,
                            &current_typeparams,
                            outer_typeparams,
                            out,
                        );
                    }
                }
            }
        }

        // When recursing into nested classes, the current TypeParams become outer TypeParams.
        let mut new_outer = outer_typeparams.clone();
        new_outer.extend(current_typeparams);
        collect_pep695_violations_from_stmts(&cls.body, bare_names, &new_outer, out);
    }
}

pub(super) fn collect_generic_subscript_sites(stmts: &[Stmt]) -> Vec<GenericSubscriptSite> {
    use ruff_text_size::Ranged as _;
    let mut out = Vec::new();
    for stmt in stmts {
        match stmt {
            // Bare expression statement: `LinkedList[int, str]`
            Stmt::Expr(expr_stmt) => {
                if let Expr::Subscript(sub) = expr_stmt.value.as_ref() {
                    if let Some(base_name) = expr_simple_name(sub.value.as_ref()) {
                        let arg_count = match sub.slice.as_ref() {
                            Expr::Tuple(t) => t.elts.len(),
                            _ => 1,
                        };
                        out.push(GenericSubscriptSite {
                            base_name,
                            arg_count,
                            span: Span::from(sub.range()),
                        });
                    }
                }
            }
            // Annotated assignment: `x: LinkedList[int, str]` or `x: LinkedList[int, str] = ...`
            Stmt::AnnAssign(ann_assign) => {
                if let Expr::Subscript(sub) = ann_assign.annotation.as_ref() {
                    if let Some(base_name) = expr_simple_name(sub.value.as_ref()) {
                        let arg_count = match sub.slice.as_ref() {
                            Expr::Tuple(t) => t.elts.len(),
                            _ => 1,
                        };
                        out.push(GenericSubscriptSite {
                            base_name,
                            arg_count,
                            span: Span::from(sub.range()),
                        });
                    }
                }
            }
            // Function definitions: check parameter annotations for subscripts
            Stmt::FunctionDef(func_def) => {
                for param in func_def
                    .parameters
                    .args
                    .iter()
                    .chain(func_def.parameters.posonlyargs.iter())
                    .chain(func_def.parameters.kwonlyargs.iter())
                {
                    if let Some(ann) = &param.parameter.annotation {
                        if let Expr::Subscript(sub) = ann.as_ref() {
                            if let Some(base_name) = expr_simple_name(sub.value.as_ref()) {
                                let arg_count = match sub.slice.as_ref() {
                                    Expr::Tuple(t) => t.elts.len(),
                                    _ => 1,
                                };
                                out.push(GenericSubscriptSite {
                                    base_name,
                                    arg_count,
                                    span: Span::from(sub.range()),
                                });
                            }
                        }
                    }
                }
                // Also recurse into the function body for nested classes etc.
                out.extend(collect_generic_subscript_sites(&func_def.body));
            }
            // Class definitions: recurse into body
            Stmt::ClassDef(class_def) => {
                out.extend(collect_generic_subscript_sites(&class_def.body));
            }
            _ => {}
        }
    }
    out
}
