//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Final Readonly visitor functions.

use ruff_python_ast::{Expr, Stmt, StmtFunctionDef};
use ruff_text_size::Ranged;

use crate::scope::{ClassInfo, ReadOnlyViolationInfo, ReadOnlyViolationKind};

use super::annotations::{ann_text_is_final, annotation_contains_readonly_expr};
use super::class_info_ext::expr_simple_name;
use super::core::{source_slice_range, text_range_to_span};

pub(super) fn collect_final_string_constants<'a>(
    stmts: &'a [Stmt],
    source: &'a str,
) -> std::collections::HashMap<&'a str, &'a str> {
    let mut map = std::collections::HashMap::new();
    for stmt in stmts {
        let Stmt::AnnAssign(ann) = stmt else { continue };
        let Expr::Name(n) = ann.target.as_ref() else {
            continue;
        };
        let range = ann.annotation.range();
        let Some(ann_text) = source_slice_range(source, range) else {
            continue;
        };
        if !ann_text_is_final(ann_text) {
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

// ---------------------------------------------------------------------------
// Shared utilities
// ---------------------------------------------------------------------------

/// Collects the field names wrapped in `ReadOnly[...]` from a functional
/// `TypedDict(...)` fields dict expression.
pub(super) fn functional_typeddict_readonly_fields(
    dict_expr: &Expr,
) -> std::collections::HashSet<&str> {
    let Expr::Dict(dict) = dict_expr else {
        return std::collections::HashSet::new();
    };
    dict.items
        .iter()
        .filter_map(|item| {
            let key_expr = item.key.as_ref()?;
            let Expr::StringLiteral(key) = key_expr else {
                return None;
            };
            if annotation_contains_readonly_expr(&item.value) {
                Some(key.value.to_str())
            } else {
                None
            }
        })
        .collect()
}

/// Scan function body for `kwargs["key"] = val` where key is a `ReadOnly` field.
pub(super) fn check_kwargs_readonly_violations(
    func: &StmtFunctionDef,
    td_readonly_fields: &std::collections::HashMap<&str, std::collections::HashSet<&str>>,
    out: &mut Vec<ReadOnlyViolationInfo>,
) {
    let Some(kwarg) = &func.parameters.kwarg else {
        return;
    };
    let Some(ann_expr) = kwarg.annotation.as_deref() else {
        return;
    };
    // Match Unpack[TypedDictName]
    let Expr::Subscript(sub) = ann_expr else {
        return;
    };
    if !matches!(sub.value.as_ref(), Expr::Name(n) if n.id == "Unpack") {
        return;
    }
    let Some(td_name) = expr_simple_name(&sub.slice) else {
        return;
    };
    let Some(readonly_fields) = td_readonly_fields.get(td_name.as_str()) else {
        return;
    };
    let kwarg_name = kwarg.name.as_str();
    for stmt in &func.body {
        let Stmt::Assign(assign) = stmt else {
            continue;
        };
        for target in &assign.targets {
            let Expr::Subscript(tsub) = target else {
                continue;
            };
            let Some(var_name) = expr_simple_name(&tsub.value) else {
                continue;
            };
            if var_name != kwarg_name {
                continue;
            }
            let Expr::StringLiteral(key_str) = tsub.slice.as_ref() else {
                continue;
            };
            let key = key_str.value.to_str();
            if readonly_fields.contains(key) {
                out.push(ReadOnlyViolationInfo {
                    var_name,
                    field_name: Some(key.to_owned()),
                    kind: ReadOnlyViolationKind::SubscriptAssign,
                    span: text_range_to_span(assign.range()),
                });
            }
        }
    }
}

/// Build a map from `TypedDict` class name to its `ReadOnly` field names.
pub(super) fn build_typeddict_readonly_map<'a>(
    stmts: &'a [Stmt],
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
    let mut map: HashMap<&str, HashSet<&str>> = classes
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
    // Functional form: `Name = TypedDict("Name", {"field": ReadOnly[...]})`
    for stmt in stmts {
        let Stmt::Assign(assign) = stmt else { continue };
        let Some(Expr::Name(lhs_name)) = assign.targets.first() else {
            continue;
        };
        let Expr::Call(call) = assign.value.as_ref() else {
            continue;
        };
        if !matches!(call.func.as_ref(), Expr::Name(n) if n.id == "TypedDict") {
            continue;
        }
        if let Some(second_arg) = call.arguments.args.get(1) {
            let fields = functional_typeddict_readonly_fields(second_arg);
            if !fields.is_empty() {
                let _ = map.insert(lhs_name.id.as_str(), fields);
            }
        }
    }
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
    let td_readonly_fields = build_typeddict_readonly_map(stmts, classes, source);
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
            Stmt::FunctionDef(func) => {
                check_kwargs_readonly_violations(func, &td_readonly_fields, &mut out);
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
pub(super) fn collect_file_final_names(
    stmts: &[Stmt],
    source: &str,
) -> std::collections::HashSet<String> {
    let mut names = std::collections::HashSet::new();
    for stmt in stmts {
        let Stmt::AnnAssign(ann) = stmt else { continue };
        let Expr::Name(n) = ann.target.as_ref() else {
            continue;
        };
        let range = ann.annotation.range();
        let Some(ann_text) = source_slice_range(source, range) else {
            continue;
        };
        if ann_text_is_final(ann_text) {
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

/// `true` when a decorator names `final` / `typing.final`.
fn decorator_is_final(dec: &ruff_python_ast::Decorator) -> bool {
    match &dec.expression {
        Expr::Name(n) => n.id.as_str() == "final",
        Expr::Attribute(a) => a.attr.as_str() == "final",
        _ => false,
    }
}

/// The `@final` method names of every class defined in `body`. A method counts
/// as `@final` when *any* of its definitions (e.g. the first overload of a stub)
/// carries `@final`.
fn collect_file_final_methods(
    body: &[Stmt],
) -> std::collections::HashMap<String, std::collections::HashSet<String>> {
    let mut out: std::collections::HashMap<String, std::collections::HashSet<String>> =
        std::collections::HashMap::new();
    for stmt in body {
        let Stmt::ClassDef(cls) = stmt else {
            continue;
        };
        let finals: std::collections::HashSet<String> = cls
            .body
            .iter()
            .filter_map(|s| match s {
                Stmt::FunctionDef(func) if func.decorator_list.iter().any(decorator_is_final) => {
                    Some(func.name.to_string())
                }
                _ => None,
            })
            .collect();
        if !finals.is_empty() {
            let _ = out.insert(cls.name.to_string(), finals);
        }
    }
    out
}

/// Map each imported class to its `@final` method names, read from a sibling
/// module (`.pyi` preferred, then `.py`). Mirrors [`collect_imported_final_names`]
/// but records per-class method sets so cross-module `@final`-override checks
/// (`qualifiers_final_decorator`) can see base methods declared `@final` in an imported stub.
pub(super) fn collect_imported_final_methods(
    stmts: &[Stmt],
    module_path: &str,
) -> std::collections::HashMap<String, std::collections::HashSet<String>> {
    let mut out: std::collections::HashMap<String, std::collections::HashSet<String>> =
        std::collections::HashMap::new();
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
        if module_str.contains('.') {
            continue;
        }
        let sibling = ["pyi", "py"].iter().find_map(|ext| {
            let path = module_dir.join(format!("{module_str}.{ext}"));
            path.to_str()
                .and_then(|s| basilisk_parser::parse_file(s).ok())
        });
        let Some(sibling) = sibling else {
            continue;
        };
        let class_finals = collect_file_final_methods(&sibling.ast.body);
        let is_star = import_from.names.iter().any(|a| a.name.as_str() == "*");
        for (class_name, methods) in class_finals {
            let imported = is_star
                || import_from
                    .names
                    .iter()
                    .any(|a| a.name.as_str() == class_name);
            if imported {
                out.entry(class_name).or_default().extend(methods);
            }
        }
    }
    out
}
