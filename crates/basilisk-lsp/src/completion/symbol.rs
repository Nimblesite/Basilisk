//! Symbol completion provider.
//!
//! Produces completion items for functions, classes, module-level variables,
//! imports, and Python builtins.

use std::collections::HashSet;

use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind};

/// Build the full symbol completion list for `prefix`.
pub(super) fn symbol_completions(
    resolved: &basilisk_resolver::ResolvedModule,
    prefix: &str,
) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    let mut seen = HashSet::new();

    add_function_completions(resolved, prefix, &mut items, &mut seen);
    add_class_completions(resolved, prefix, &mut items, &mut seen);
    add_variable_completions(resolved, prefix, &mut items, &mut seen);
    add_import_completions(resolved, prefix, &mut items, &mut seen);
    add_builtin_completions(&mut items, &mut seen, prefix);
    items
}

fn add_function_completions(
    resolved: &basilisk_resolver::ResolvedModule,
    prefix: &str,
    items: &mut Vec<CompletionItem>,
    seen: &mut HashSet<String>,
) {
    for func in &resolved.functions {
        if func.class_name.is_some() {
            continue;
        }
        if !prefix.is_empty() && !func.name.starts_with(prefix) {
            continue;
        }
        if seen.insert(func.name.clone()) {
            let mut detail = String::from("(");
            for (idx, param) in func.parameters.iter().enumerate() {
                if idx > 0 {
                    detail.push_str(", ");
                }
                detail.push_str(&param.name);
            }
            detail.push(')');
            let data = serde_json::json!({
                "kind": "function",
                "name": func.name
            });
            items.push(CompletionItem {
                label: func.name.clone(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some(detail),
                data: Some(serde_json::to_value(data).unwrap_or_default()),
                documentation: None,
                ..Default::default()
            });
        }
    }
}

fn add_class_completions(
    resolved: &basilisk_resolver::ResolvedModule,
    prefix: &str,
    items: &mut Vec<CompletionItem>,
    seen: &mut HashSet<String>,
) {
    for class in &resolved.classes {
        if !prefix.is_empty() && !class.name.starts_with(prefix) {
            continue;
        }
        if seen.insert(class.name.clone()) {
            let data = serde_json::json!({
                "kind": "class",
                "name": class.name
            });
            items.push(CompletionItem {
                label: class.name.clone(),
                kind: Some(CompletionItemKind::CLASS),
                detail: Some("class".to_owned()),
                data: Some(serde_json::to_value(data).unwrap_or_default()),
                documentation: None,
                ..Default::default()
            });
        }
    }
}

fn add_variable_completions(
    resolved: &basilisk_resolver::ResolvedModule,
    prefix: &str,
    items: &mut Vec<CompletionItem>,
    seen: &mut HashSet<String>,
) {
    for var in &resolved.module_vars {
        if !prefix.is_empty() && !var.name.starts_with(prefix) {
            continue;
        }
        if seen.insert(var.name.clone()) {
            let data = serde_json::json!({
                "kind": "variable",
                "name": var.name
            });
            items.push(CompletionItem {
                label: var.name.clone(),
                kind: Some(CompletionItemKind::VARIABLE),
                detail: Some("variable".to_owned()),
                data: Some(serde_json::to_value(data).unwrap_or_default()),
                ..Default::default()
            });
        }
    }
}

/// Add completion items for all imported names.
pub(super) fn add_import_completions(
    resolved: &basilisk_resolver::ResolvedModule,
    prefix: &str,
    items: &mut Vec<CompletionItem>,
    seen: &mut HashSet<String>,
) {
    for imp in &resolved.imports {
        match imp.kind {
            basilisk_resolver::scope::ImportKind::Plain => {
                let name = imp.module.split('.').next().unwrap_or(&imp.module);
                if !prefix.is_empty() && !name.starts_with(prefix) {
                    continue;
                }
                if seen.insert(name.to_owned()) {
                    items.push(CompletionItem {
                        label: name.to_owned(),
                        kind: Some(CompletionItemKind::MODULE),
                        detail: Some(format!("import {}", imp.module)),
                        ..Default::default()
                    });
                }
            }
            basilisk_resolver::scope::ImportKind::From => {
                for name in &imp.names {
                    if !prefix.is_empty() && !name.starts_with(prefix) {
                        continue;
                    }
                    if seen.insert(name.clone()) {
                        items.push(CompletionItem {
                            label: name.clone(),
                            kind: Some(CompletionItemKind::VARIABLE),
                            detail: Some(format!("from {} import", imp.module)),
                            ..Default::default()
                        });
                    }
                }
            }
            basilisk_resolver::scope::ImportKind::Star => {}
        }
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "contains large const arrays of Python builtins that cannot be split"
)]
/// Add Python builtin functions, constants, and exceptions to the completion list.
pub(super) fn add_builtin_completions(
    items: &mut Vec<CompletionItem>,
    seen: &mut HashSet<String>,
    prefix: &str,
) {
    const BUILTIN_FUNCTIONS: &[&str] = &[
        "abs",
        "all",
        "any",
        "bin",
        "bool",
        "breakpoint",
        "bytearray",
        "bytes",
        "callable",
        "chr",
        "classmethod",
        "compile",
        "complex",
        "delattr",
        "dict",
        "dir",
        "divmod",
        "enumerate",
        "eval",
        "exec",
        "filter",
        "float",
        "format",
        "frozenset",
        "getattr",
        "globals",
        "hasattr",
        "hash",
        "help",
        "hex",
        "id",
        "input",
        "int",
        "isinstance",
        "issubclass",
        "iter",
        "len",
        "list",
        "locals",
        "map",
        "max",
        "memoryview",
        "min",
        "next",
        "object",
        "oct",
        "open",
        "ord",
        "pow",
        "print",
        "property",
        "range",
        "repr",
        "reversed",
        "round",
        "set",
        "setattr",
        "slice",
        "sorted",
        "staticmethod",
        "str",
        "sum",
        "super",
        "tuple",
        "type",
        "vars",
        "zip",
    ];
    const BUILTIN_CONSTANTS: &[&str] = &["True", "False", "None", "NotImplemented", "Ellipsis"];
    const BUILTIN_EXCEPTIONS: &[&str] = &[
        "Exception",
        "BaseException",
        "ValueError",
        "TypeError",
        "KeyError",
        "IndexError",
        "AttributeError",
        "ImportError",
        "OSError",
        "RuntimeError",
        "StopIteration",
        "ArithmeticError",
        "LookupError",
        "SyntaxError",
        "NameError",
        "FileNotFoundError",
        "NotImplementedError",
        "OverflowError",
        "ZeroDivisionError",
        "RecursionError",
        "PermissionError",
        "TimeoutError",
    ];

    add_builtin_group(
        items,
        seen,
        prefix,
        BUILTIN_FUNCTIONS,
        CompletionItemKind::FUNCTION,
        "built-in",
    );
    add_builtin_group(
        items,
        seen,
        prefix,
        BUILTIN_CONSTANTS,
        CompletionItemKind::CONSTANT,
        "built-in",
    );
    add_builtin_group(
        items,
        seen,
        prefix,
        BUILTIN_EXCEPTIONS,
        CompletionItemKind::CLASS,
        "built-in exception",
    );
}

fn add_builtin_group(
    items: &mut Vec<CompletionItem>,
    seen: &mut HashSet<String>,
    prefix: &str,
    names: &[&str],
    kind: CompletionItemKind,
    detail: &str,
) {
    for &name in names {
        if !prefix.is_empty() && !name.starts_with(prefix) {
            continue;
        }
        if seen.insert(name.to_owned()) {
            items.push(CompletionItem {
                label: name.to_owned(),
                kind: Some(kind),
                detail: Some(detail.to_owned()),
                ..Default::default()
            });
        }
    }
}
