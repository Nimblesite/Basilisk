//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Final Readonly visitor functions.

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use crate::scope::{ClassInfo, ReadOnlyViolationInfo, ReadOnlyViolationKind};

use crate::canonical::BindingTable;

use super::annotations::annotation_is_final;
use super::core::text_range_to_span;

// ##########################################################################
// # DELETED BODIES — `build_typeddict_readonly_map` and                     #
// # `build_var_type_map`. DO NOT RESTORE THEM AND DO NOT RETURN AN EMPTY    #
// # MAP.                                                                    #
// #                                                                         #
// #   Some((cls.name.as_str(), fields))                                     #
// #   td_readonly_fields.get_key_value(type_name.id.as_str())               #
// #                                                                         #
// # A `ReadOnly` VIOLATION DECIDED BY MATCHING AN ANNOTATION'S SPELLING     #
// # AGAINST A CLASS'S SPELLING. The `TypedDict` chain and the effective     #
// # field set were both computed on the definition-site class graph, and    #
// # then the class was reduced to `ClassInfo::name` so an annotation's      #
// # `Expr::Name` could be looked up by its characters. So:                  #
// #                                                                         #
// #   * `Alias = Album; a: Alias` never matches, and every `a["x"] = ...`   #
// #     assignment to a read-only field goes unreported;                    #
// #   * `import other; a: other.Album` is not an `Expr::Name` at all and    #
// #     falls out silently;                                                 #
// #   * an ordinary class and a `TypedDict` sharing one name in the same    #
// #     module collapse onto one entry, so a plain `a["x"] = 1` is          #
// #     REPORTED against a class that has no read-only anything.            #
// #                                                                         #
// # The annotation and the target are both `Expr` nodes with real offsets   #
// # at the point they are read here. The rebuild resolves each through the  #
// # module's binding table to the `class` statement it denotes and keys the #
// # map on `ClassInfo::name_span`. `collect_readonly_violations` is kept as #
// # the map of what has to be rebuilt.                                      #
// ##########################################################################

/// DELETED — panics; see the banner above.
pub(super) fn build_typeddict_readonly_map<'a>(
    _classes: &'a [ClassInfo],
    _source: &'a str,
) -> std::collections::HashMap<&'a str, std::collections::HashSet<&'a str>> {
    panic!(
        "basilisk-resolver: `build_typeddict_readonly_map` was DELETED because it resolved \
         the `TypedDict` chain on the definition-site class graph and then keyed the \
         read-only field sets by CLASS NAME, so an annotation was matched to a class by \
         its characters. It panics because the real implementation — the annotation \
         expression resolved through the module's binding table, keyed on \
         `ClassInfo::name_span` — DOES NOT EXIST YET. Do not restore the name key and do \
         not return an empty map in its place."
    )
}

/// DELETED — panics; see the banner above.
fn build_var_type_map<'a>(
    _stmts: &'a [Stmt],
    _td_readonly_fields: &std::collections::HashMap<&'a str, std::collections::HashSet<&'a str>>,
) -> std::collections::HashMap<&'a str, &'a str> {
    panic!(
        "basilisk-resolver: `build_var_type_map` was DELETED because it decided which \
         `TypedDict` a variable was annotated with by looking the ANNOTATION'S SPELLING up \
         in a map keyed by CLASS SPELLING, so an aliased or dotted annotation was invisible \
         and a same-named ordinary class inherited another class's read-only fields. It \
         panics because the real implementation — both expressions resolved through the \
         binding table at their own offsets — DOES NOT EXIST YET. Do not restore the name \
         lookup and do not return an empty map in its place."
    )
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
