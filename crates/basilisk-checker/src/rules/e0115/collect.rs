//! Collection helpers for BSK-E0115.
//!
//! Functions that scan an AST or source text to build maps of deprecated
//! definitions and variable types.

use std::collections::HashMap;

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::Span;

use super::decorators::{is_deprecated_decorator, text_range_to_span};
use super::types::VarType;

/// Info about a deprecated entity.
#[derive(Debug, Clone)]
pub(super) struct DeprecatedInfo {
    /// The kind of entity: "class", "function", "method", "overload", "property", "property setter".
    pub(super) kind: String,
    /// The deprecation message from the decorator argument.
    pub(super) message: Option<String>,
    /// The defining span (for deduplication).
    pub(super) def_span: Span,
}

/// Collect deprecated function/class definitions from a list of statements.
pub(super) fn collect_deprecated_definitions(
    stmts: &[Stmt],
    out: &mut HashMap<String, DeprecatedInfo>,
    class_name: Option<&str>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDef(func) => {
                let has_overload = func.decorator_list.iter().any(|d| {
                    matches!(&d.expression, Expr::Name(n) if n.id.as_str() == "overload")
                        || matches!(&d.expression, Expr::Attribute(a) if a.attr.as_str() == "overload")
                });

                for dec in &func.decorator_list {
                    if let Some(message) = is_deprecated_decorator(&dec.expression) {
                        let kind = if has_overload {
                            "overload".to_owned()
                        } else if class_name.is_some() {
                            let has_property = func.decorator_list.iter().any(|d| {
                                matches!(&d.expression, Expr::Name(n) if n.id.as_str() == "property")
                            });
                            let has_setter = func.decorator_list.iter().any(|d| {
                                if let Expr::Attribute(a) = &d.expression {
                                    a.attr.as_str() == "setter"
                                } else {
                                    false
                                }
                            });
                            if has_setter {
                                "property setter".to_owned()
                            } else if has_property {
                                "property".to_owned()
                            } else {
                                "method".to_owned()
                            }
                        } else {
                            "function".to_owned()
                        };

                        let name = if let Some(cls) = class_name {
                            format!("{cls}.{}", func.name)
                        } else {
                            func.name.to_string()
                        };

                        let _ = out.insert(
                            name,
                            DeprecatedInfo {
                                kind,
                                message,
                                def_span: text_range_to_span(func.range()),
                            },
                        );
                        break;
                    }
                }

                // Recurse into method bodies for nested definitions.
                collect_deprecated_definitions(&func.body, out, class_name);
            }
            Stmt::ClassDef(cls) => {
                for dec in &cls.decorator_list {
                    if let Some(message) = is_deprecated_decorator(&dec.expression) {
                        let _ = out.insert(
                            cls.name.to_string(),
                            DeprecatedInfo {
                                kind: "class".to_owned(),
                                message,
                                def_span: text_range_to_span(cls.range()),
                            },
                        );
                        break;
                    }
                }
                // Recurse into class body for methods.
                collect_deprecated_definitions(&cls.body, out, Some(cls.name.as_str()));
            }
            _ => {}
        }
    }
}

/// Collect deprecated names imported from sibling modules.
///
/// Also populates `from_import_deprecated` with `(local_name, import_span)` pairs so
/// that a diagnostic can be emitted at the import site itself (PEP 702 requirement).
pub(super) fn collect_imported_deprecated(
    stmts: &[Stmt],
    module_path: &str,
    out: &mut HashMap<String, DeprecatedInfo>,
    module_aliases: &mut HashMap<String, String>,
    from_import_deprecated: &mut Vec<(String, Span)>,
) {
    let Some(module_dir) = std::path::Path::new(module_path).parent() else {
        return;
    };

    for stmt in stmts {
        match stmt {
            Stmt::ImportFrom(import_from) => {
                let Some(module_name) = import_from.module.as_ref() else {
                    continue;
                };
                let module_str = module_name.to_string();
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

                // Collect deprecated definitions from the sibling.
                let mut sibling_deprecated: HashMap<String, DeprecatedInfo> = HashMap::new();
                collect_deprecated_definitions(&sibling.ast.body, &mut sibling_deprecated, None);

                for alias in &import_from.names {
                    let name = alias.name.as_str();
                    if let Some(info) = sibling_deprecated.get(name) {
                        let local_name = alias
                            .asname
                            .as_ref()
                            .map_or_else(|| name.to_owned(), std::string::ToString::to_string);
                        // Record the import site span so we can emit a diagnostic there.
                        let import_span = text_range_to_span(import_from.range());
                        from_import_deprecated.push((local_name.clone(), import_span));
                        let _ = out.insert(local_name, info.clone());
                    }
                }
            }
            Stmt::Import(import_stmt) => {
                for alias in &import_stmt.names {
                    let module_str = alias.name.to_string();
                    if let Some(asname) = alias.asname.as_ref() {
                        let _ = module_aliases.insert(asname.to_string(), module_str);
                    } else {
                        let _ = module_aliases.insert(module_str.clone(), module_str);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Collect deprecated members from imported module classes.
///
/// Returns a map: `module_alias` -> member_key -> `DeprecatedInfo`.
/// Member keys look like `"norwegian_blue"` (top-level) or `"Spam.__add__"` (class member).
pub(super) fn collect_imported_deprecated_members(
    stmts: &[Stmt],
    module_path: &str,
) -> HashMap<String, HashMap<String, DeprecatedInfo>> {
    let mut result: HashMap<String, HashMap<String, DeprecatedInfo>> = HashMap::new();
    let Some(module_dir) = std::path::Path::new(module_path).parent() else {
        return result;
    };

    for stmt in stmts {
        if let Stmt::Import(import_stmt) = stmt {
            for alias in &import_stmt.names {
                let module_str = alias.name.to_string();
                if module_str.contains('.') {
                    continue;
                }
                let alias_name = alias
                    .asname
                    .as_ref()
                    .map_or_else(|| module_str.clone(), std::string::ToString::to_string);
                let sibling_path = module_dir.join(format!("{module_str}.py"));
                let Some(sibling_path_str) = sibling_path.to_str() else {
                    continue;
                };
                let Ok(sibling) = basilisk_parser::parse_file(sibling_path_str) else {
                    continue;
                };
                let mut sibling_deprecated: HashMap<String, DeprecatedInfo> = HashMap::new();
                collect_deprecated_definitions(&sibling.ast.body, &mut sibling_deprecated, None);
                if !sibling_deprecated.is_empty() {
                    let _ = result.insert(alias_name, sibling_deprecated);
                }
            }
        }
    }
    result
}

/// Build a map from variable name to inferred class type by scanning simple assignments.
///
/// Handles:
/// - `spam = library.Spam()` -> spam: `VarType { module_alias: "library", class_name: "Spam" }`
/// - `invocable = Invocable()` -> invocable: `VarType { module_alias: "", class_name: "Invocable" }`
pub(super) fn collect_var_types(stmts: &[Stmt]) -> HashMap<String, VarType> {
    let mut var_types: HashMap<String, VarType> = HashMap::new();
    collect_var_types_from_stmts(stmts, &mut var_types);
    var_types
}

fn collect_var_types_from_stmts(stmts: &[Stmt], var_types: &mut HashMap<String, VarType>) {
    for stmt in stmts {
        match stmt {
            Stmt::Assign(assign) => {
                if let Some(var_type) = infer_call_type(&assign.value) {
                    for target in &assign.targets {
                        if let Expr::Name(name) = target {
                            let _ = var_types.insert(name.id.to_string(), var_type.clone());
                        }
                    }
                }
            }
            Stmt::FunctionDef(func) => {
                collect_var_types_from_stmts(&func.body, var_types);
            }
            Stmt::ClassDef(cls) => {
                collect_var_types_from_stmts(&cls.body, var_types);
            }
            _ => {}
        }
    }
}

/// Infer the class type from a constructor call expression.
fn infer_call_type(expr: &Expr) -> Option<VarType> {
    if let Expr::Call(call) = expr {
        match call.func.as_ref() {
            Expr::Name(name) => {
                return Some(VarType {
                    module_alias: String::new(),
                    class_name: name.id.to_string(),
                });
            }
            Expr::Attribute(attr) => {
                if let Expr::Name(obj) = attr.value.as_ref() {
                    return Some(VarType {
                        module_alias: obj.id.to_string(),
                        class_name: attr.attr.to_string(),
                    });
                }
            }
            _ => {}
        }
    }
    None
}
