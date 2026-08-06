//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Final Readonly visitor functions.

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use crate::scope::{ClassInfo, ReadOnlyViolationInfo, ReadOnlyViolationKind};

use crate::canonical::BindingTable;

use super::annotations::annotation_is_final;
use super::core::{source_slice_range, text_range_to_span};

pub(super) fn collect_final_string_constants<'a>(
    bindings: &BindingTable,
    stmts: &'a [Stmt],
) -> std::collections::HashMap<&'a str, &'a str> {
    let mut map = std::collections::HashMap::new();
    for stmt in stmts {
        let Stmt::AnnAssign(ann) = stmt else { continue };
        let Expr::Name(n) = ann.target.as_ref() else {
            continue;
        };
        if !annotation_is_final(bindings, &ann.annotation) {
            continue;
        }
        // RHS must be a string literal.
        let Some(val) = ann.value.as_deref() else {
            continue;
        };
        let Expr::StringLiteral(s) = val else {
            continue;
        };
        let _ = map.insert(n.id.as_str(), s.value.to_str());
    }
    map
}

/// Build a map from `TypedDict` class name to its `ReadOnly` field names.
pub(super) fn build_typeddict_readonly_map<'a>(
    classes: &'a [ClassInfo],
    source: &'a str,
) -> std::collections::HashMap<&'a str, std::collections::HashSet<&'a str>> {
    use std::collections::{HashMap, HashSet};
    let class_map = crate::scope::class_by_name(classes);
    // Use the effective (post-inheritance) field set so a subclass that does NOT
    // redeclare an inherited `ReadOnly` field still treats it as read-only
    // (`class Album2(NamedDict): year: int` keeps `name: ReadOnly[str]`), while a
    // subclass that redeclares it as mutable drops the read-only status (the
    // most-derived declaration wins).
    let map: HashMap<&str, HashSet<&str>> = classes
        .iter()
        .filter(|cls| crate::scope::is_transitive_typeddict(cls.name.as_str(), &class_map))
        .filter_map(|cls| {
            let fields: HashSet<&str> =
                super::typeddict_schema::effective_fields(cls, &class_map, source)
                    .into_iter()
                    .filter(|f| f.readonly)
                    .map(|f| f.name)
                    .collect();
            if fields.is_empty() {
                None
            } else {
                Some((cls.name.as_str(), fields))
            }
        })
        .collect();
    map
}

/// Build a borrowed map from variable names to their declared `TypedDict`.
///
/// The resolver only needs these names while walking the AST. Borrowing them
/// avoids allocating two strings per annotated variable in large `TypedDict`
/// modules; only names that become diagnostics are copied into the result.
fn build_var_type_map<'a>(
    stmts: &'a [Stmt],
    td_readonly_fields: &std::collections::HashMap<&'a str, std::collections::HashSet<&'a str>>,
) -> std::collections::HashMap<&'a str, &'a str> {
    let mut map = std::collections::HashMap::new();
    for stmt in stmts {
        let Stmt::AnnAssign(ann) = stmt else { continue };
        let Expr::Name(var_name) = ann.target.as_ref() else {
            continue;
        };
        let Expr::Name(type_name) = ann.annotation.as_ref() else {
            continue;
        };
        if let Some((&key, _)) = td_readonly_fields.get_key_value(type_name.id.as_str()) {
            let _ = map.insert(var_name.id.as_str(), key);
        }
    }
    map
}

/// Build a map from variable name to its declared `TypedDict` type name.
pub(super) fn collect_readonly_violations(
    stmts: &[Stmt],
    classes: &[ClassInfo],
    source: &str,
) -> Vec<ReadOnlyViolationInfo> {
    let td_readonly_fields = build_typeddict_readonly_map(classes, source);
    if td_readonly_fields.is_empty() {
        return Vec::new();
    }
    let var_type = build_var_type_map(stmts, &td_readonly_fields);
    let mut out = Vec::new();
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
                    let Some(&class_name) = var_type.get(var_name.id.as_str()) else {
                        continue;
                    };
                    let Some(fields) = td_readonly_fields.get(class_name) else {
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
            _ => {}
        }
    }
    out
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
        let sibling_finals = collect_file_final_names(&sibling.ast.body, &sibling.source);
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

