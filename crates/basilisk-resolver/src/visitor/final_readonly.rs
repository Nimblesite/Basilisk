//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Final Readonly visitor functions.

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use crate::scope::{ClassGraph, ClassInfo, ReadOnlyViolationInfo, ReadOnlyViolationKind, Span};

use crate::canonical::BindingTable;

use super::annotations::annotation_is_final;
use super::core::text_range_to_span;
use super::typeddict::{annotation_local_class, kwargs_unpacked_local_class};

/// The `ReadOnly` field names of every `TypedDict` that has any, keyed by the
/// class's DEFINITION SITE ([`ClassInfo::name_span`]).
///
/// The predecessor of this function was deleted for keying the same field
/// sets by `ClassInfo::name`, so an annotation was matched to a class by its
/// characters: an aliased annotation missed, a dotted one fell out, and an
/// ordinary class sharing a `TypedDict`'s name collapsed onto its entry. The
/// effective (post-inheritance) field set is unchanged: a subclass that does
/// NOT redeclare an inherited `ReadOnly` field keeps it read-only, while a
/// subclass that redeclares it as mutable drops the status (the most-derived
/// declaration wins). [`super::typeddict_schema::EffectiveField::readonly`]
/// is computed from `AttributeInfo::is_readonly`, which the collection walk
/// resolved through the bindings — no text is read here.
fn build_typeddict_readonly_map<'a>(
    graph: &ClassGraph<'a>,
) -> std::collections::HashMap<Span, std::collections::HashSet<&'a str>> {
    graph
        .typed_dicts()
        .into_iter()
        .filter_map(|cls| {
            let fields: std::collections::HashSet<&str> =
                super::typeddict_schema::effective_fields(cls, graph)
                    .into_iter()
                    .filter(|f| f.readonly)
                    .map(|f| f.name)
                    .collect();
            if fields.is_empty() {
                None
            } else {
                Some((cls.name_span, fields))
            }
        })
        .collect()
}

/// Associate each annotated variable in `stmts` with the definition site of
/// the read-only-bearing `TypedDict` its annotation denotes.
///
/// The predecessor of this function was deleted for looking the annotation's
/// SPELLING up in a map keyed by class SPELLING. Here the annotation
/// expression resolves through the module's bindings —
/// [`annotation_local_class`] follows assignment aliases, subscripts, and
/// quoted forward references — and only an annotation that resolves to a
/// class in `td_readonly_fields` is recorded; everything else is abstention.
fn build_var_type_map(
    bindings: &BindingTable,
    stmts: &[Stmt],
    td_readonly_fields: &std::collections::HashMap<Span, std::collections::HashSet<&str>>,
) -> std::collections::HashMap<String, Span> {
    let mut map = std::collections::HashMap::new();
    for stmt in stmts {
        let Stmt::AnnAssign(ann) = stmt else { continue };
        let Expr::Name(var_name) = ann.target.as_ref() else {
            continue;
        };
        let Some(site) = annotation_local_class(bindings, &ann.annotation) else {
            continue;
        };
        let span = text_range_to_span(site);
        if td_readonly_fields.contains_key(&span) {
            let _ = map.insert(var_name.id.to_string(), span);
        }
    }
    map
}

/// Collect writes to `ReadOnly` `TypedDict` fields (PEP 705), from
/// module-level statements and function bodies.
pub(super) fn collect_readonly_violations(
    bindings: &BindingTable,
    stmts: &[Stmt],
    classes: &[ClassInfo],
) -> Vec<ReadOnlyViolationInfo> {
    let graph = ClassGraph::new(classes);
    let td_readonly_fields = build_typeddict_readonly_map(&graph);
    if td_readonly_fields.is_empty() {
        return Vec::new();
    }
    let var_type = build_var_type_map(bindings, stmts, &td_readonly_fields);
    let mut out = Vec::new();
    check_readonly_stmts(bindings, &td_readonly_fields, &var_type, stmts, &mut out);
    out
}

/// The walk behind [`collect_readonly_violations`]: flat over `stmts`, and
/// into each function body with the parameters — `p: Album` and PEP 692's
/// `**kwargs: Unpack[Album]` — added to the scope's variable associations.
fn check_readonly_stmts(
    bindings: &BindingTable,
    td_readonly_fields: &std::collections::HashMap<Span, std::collections::HashSet<&str>>,
    var_type: &std::collections::HashMap<String, Span>,
    stmts: &[Stmt],
    out: &mut Vec<ReadOnlyViolationInfo>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Assign(assign) => {
                for target in &assign.targets {
                    let Expr::Subscript(sub) = target else {
                        continue;
                    };
                    let Expr::Name(var_name) = sub.value.as_ref() else {
                        continue;
                    };
                    let Some(fields) = var_type
                        .get(var_name.id.as_str())
                        .and_then(|site| td_readonly_fields.get(site))
                    else {
                        continue;
                    };
                    let Expr::StringLiteral(key_str) = sub.slice.as_ref() else {
                        continue;
                    };
                    let key = key_str.value.to_str();
                    if fields.contains(key) {
                        out.push(ReadOnlyViolationInfo {
                            var_name: var_name.id.to_string(),
                            field_name: Some(key.to_owned()),
                            kind: ReadOnlyViolationKind::SubscriptAssign,
                            span: text_range_to_span(assign.range()),
                        });
                    }
                }
            }
            Stmt::Expr(expr_stmt) => {
                let Expr::Call(call) = expr_stmt.value.as_ref() else {
                    continue;
                };
                let Expr::Attribute(attr) = call.func.as_ref() else {
                    continue;
                };
                if attr.attr.as_str() != "update" {
                    continue;
                }
                let Expr::Name(var_name) = attr.value.as_ref() else {
                    continue;
                };
                if var_type.contains_key(var_name.id.as_str()) {
                    out.push(ReadOnlyViolationInfo {
                        var_name: var_name.id.to_string(),
                        field_name: None,
                        kind: ReadOnlyViolationKind::UpdateCall,
                        span: text_range_to_span(expr_stmt.value.range()),
                    });
                }
            }
            Stmt::FunctionDef(func) => {
                let mut local_vars = var_type.clone();
                local_vars.extend(build_var_type_map(bindings, &func.body, td_readonly_fields));
                for param in super::walks::iter_all_params(&func.parameters) {
                    if let Some(ann) = &param.parameter.annotation {
                        if let Some(site) = annotation_local_class(bindings, ann) {
                            let span = text_range_to_span(site);
                            if td_readonly_fields.contains_key(&span) {
                                let _ = local_vars.insert(param.parameter.name.to_string(), span);
                            }
                        }
                    }
                }
                if let Some(kwargs) = func.parameters.kwarg.as_deref() {
                    if let Some(ann) = &kwargs.annotation {
                        if let Some(site) = kwargs_unpacked_local_class(bindings, ann) {
                            let span = text_range_to_span(site);
                            if td_readonly_fields.contains_key(&span) {
                                let _ = local_vars.insert(kwargs.name.to_string(), span);
                            }
                        }
                    }
                }
                check_readonly_stmts(bindings, td_readonly_fields, &local_vars, &func.body, out);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Module-level bare and attribute assignment collection
// ---------------------------------------------------------------------------

/// Collect module-level bare assignments (`name = expr`).
///
/// Used by the checker to detect re-assignments to `Final`-annotated variables.
pub(super) fn collect_file_final_names(stmts: &[Stmt]) -> std::collections::HashSet<String> {
    // These statements are a whole module's body — its own imports decide what
    // the qualifier binds to there, so the table is built from them.
    let bindings = BindingTable::from_module(stmts);
    let mut names = std::collections::HashSet::new();
    for stmt in stmts {
        let Stmt::AnnAssign(ann) = stmt else { continue };
        let Expr::Name(n) = ann.target.as_ref() else {
            continue;
        };
        if annotation_is_final(&bindings, &ann.annotation) {
            let _ = names.insert(n.id.to_string());
        }
    }
    names
}

/// Collect the set of imported names that are declared `Final` in a sibling module.
///
/// For `from X import Y`, checks if `Y` is `Final` in `X.py`.
/// For `from X import *`, adds all `Final` names from `X.py`.
/// Only resolves simple (non-dotted) module names that map to local `.py` files.
pub(super) fn collect_imported_final_names(
    stmts: &[Stmt],
    module_path: &str,
) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    let Some(module_dir) = std::path::Path::new(module_path).parent() else {
        return out;
    };
    for stmt in stmts {
        let Stmt::ImportFrom(import_from) = stmt else {
            continue;
        };
        let Some(module_name) = import_from.module.as_ref() else {
            continue;
        };
        let module_str = module_name.to_string();
        // Only handle simple (undotted) local module names.
        if module_str.contains('.') {
            continue;
        }
        let sibling_path = module_dir.join(format!("{module_str}.py"));
        let Some(sibling_path_str) = sibling_path.to_str() else {
            continue;
        };
        let Ok(sibling) = basilisk_parser::parse_file(sibling_path_str) else {
            continue;
        };
        let sibling_finals = collect_file_final_names(&sibling.ast.body);
        let is_star = import_from.names.iter().any(|a| a.name.as_str() == "*");
        if is_star {
            out.extend(sibling_finals);
        } else {
            for alias in &import_from.names {
                let name = alias.name.as_str();
                if sibling_finals.contains(name) {
                    let _ = out.insert(name.to_owned());
                }
            }
        }
    }
    out
}
