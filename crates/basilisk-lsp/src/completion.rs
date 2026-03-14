//! Completion (`IntelliSense`) handler.
//!
//! Provides symbol completions, dot completions, import completions,
//! keyword argument completions, and Python builtin completions.
//!
//! Supports lazy-loading of documentation via `completionItem/resolve`.

use std::collections::HashSet;

use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, Documentation, MarkupContent, MarkupKind,
};

// ── Public API ───────────────────────────────────────────────────────────────

/// Compute completion items for a byte offset in the source.
#[must_use]
pub fn complete(
    resolved: &basilisk_resolver::ResolvedModule,
    text: &str,
    byte_offset: usize,
) -> Vec<CompletionItem> {
    let prefix = extract_prefix(text, byte_offset);
    if is_dot_completion(text, byte_offset) {
        dot_completions(resolved, text, byte_offset)
    } else if let Some(kwarg_items) = kwarg_completions(resolved, text, byte_offset, &prefix) {
        kwarg_items
    } else {
        symbol_completions(resolved, &prefix)
    }
}

/// Try to parse and resolve a source string, returning `None` on failure.
#[must_use]
pub fn try_resolve(text: &str, path: &str) -> Option<basilisk_resolver::ResolvedModule> {
    let parsed = basilisk_parser::parse_source(text.to_owned(), path.to_owned()).ok()?;
    basilisk_resolver::resolve(&parsed).ok()
}

/// Replace the line at `line_number` (0-based) with `pass` (preserving indentation).
///
/// Keeps the file structurally valid when the cursor line has an incomplete
/// expression like `self.` or `obj.`.
#[must_use]
pub fn patch_cursor_line(text: &str, line_number: u32) -> String {
    text.lines()
        .enumerate()
        .map(|(idx, line)| {
            if idx == line_number as usize {
                let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
                format!("{indent}pass")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Resolve additional data for a completion item (lazy-load documentation/detail).
///
/// This is called when the user selects a completion item, allowing us to
/// provide rich detail information without including it in every completion list.
///
/// The completion item's `data` field should contain a JSON object with:
/// - `kind`: "function", "class", "variable", "attribute", "method"
/// - `name`: the symbol name
/// - For functions/classes: `data.file_path` for the source file
#[must_use]
pub fn resolve_completion_item(item: CompletionItem, text: &str, path: &str) -> CompletionItem {
    let Some(data) = &item.data else {
        return item;
    };

    // Parse the data to determine what kind of symbol this is
    let kind = data.get("kind").and_then(|k| k.as_str()).unwrap_or("");
    let name = data.get("name").and_then(|n| n.as_str()).unwrap_or("");

    if name.is_empty() {
        return item;
    }

    // Try to resolve the module to find the symbol's documentation
    let Some(resolved) = try_resolve(text, path) else {
        return item;
    };

    match kind {
        "function" | "method" => {
            if let Some(func) = resolved.functions.iter().find(|f| f.name == name) {
                let mut result = item;
                // Add signature to detail if not already set
                if result.detail.is_none() {
                    let mut detail = String::from("(");
                    for (idx, param) in func.parameters.iter().enumerate() {
                        if idx > 0 {
                            detail.push_str(", ");
                        }
                        detail.push_str(&param.name);
                    }
                    detail.push(')');
                    result.detail = Some(detail);
                }
                // Add docstring to documentation if not already set
                if result.documentation.is_none() {
                    if let Some(ref docstring) = func.docstring {
                        result.documentation = Some(Documentation::MarkupContent(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: docstring.clone(),
                        }));
                    }
                }
                return result;
            }
        }
        "class" => {
            if let Some(class) = resolved.classes.iter().find(|c| c.name == name) {
                let mut result = item;
                // Add class info to detail if not already set
                if result.detail.is_none() {
                    let detail = if class.bases.is_empty() {
                        "class".to_owned()
                    } else {
                        format!("class({})", class.bases.join(", "))
                    };
                    result.detail = Some(detail);
                }
                // Add docstring to documentation if not already set
                if result.documentation.is_none() {
                    if let Some(ref docstring) = class.docstring {
                        result.documentation = Some(Documentation::MarkupContent(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: docstring.clone(),
                        }));
                    }
                }
                return result;
            }
        }
        "variable" | "attribute" => {
            // For variables, we can at least ensure detail is present
            if resolved.module_vars.iter().any(|v| v.name == name) {
                let mut result = item;
                if result.detail.is_none() {
                    result.detail = Some("variable".to_owned());
                }
                return result;
            }
        }
        _ => {}
    }

    item
}

// ── Prefix / dot detection ───────────────────────────────────────────────────

fn extract_prefix(text: &str, byte_offset: usize) -> String {
    let before = &text[..byte_offset.min(text.len())];
    before
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn is_dot_completion(text: &str, byte_offset: usize) -> bool {
    let before = &text[..byte_offset.min(text.len())];
    let stripped = before.trim_end_matches(|c: char| c.is_alphanumeric() || c == '_');
    stripped.ends_with('.')
}

fn dot_receiver(text: &str, byte_offset: usize) -> Option<String> {
    let before = &text[..byte_offset.min(text.len())];
    let stripped = before.trim_end_matches(|c: char| c.is_alphanumeric() || c == '_');
    let before_dot = stripped.strip_suffix('.')?;
    let name: String = before_dot
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

// ── Dot completions (class attributes + methods) ─────────────────────────────

fn dot_completions(
    resolved: &basilisk_resolver::ResolvedModule,
    text: &str,
    byte_offset: usize,
) -> Vec<CompletionItem> {
    let receiver = dot_receiver(text, byte_offset);
    let prefix = extract_prefix(text, byte_offset);

    if receiver.as_deref() == Some("self") {
        enclosing_class(resolved, byte_offset)
            .map(|c| class_member_items(c, &prefix))
            .unwrap_or_default()
    } else if let Some(ref recv_name) = receiver {
        resolved
            .classes
            .iter()
            .find(|c| &c.name == recv_name)
            .map(|c| class_member_items(c, &prefix))
            .unwrap_or_default()
    } else {
        vec![]
    }
}

fn enclosing_class(
    resolved: &basilisk_resolver::ResolvedModule,
    offset: usize,
) -> Option<&basilisk_resolver::scope::ClassInfo> {
    let func = resolved
        .functions
        .iter()
        .find(|f| f.class_name.is_some() && (f.def_span.start as usize) <= offset)?;
    let class_name = func.class_name.as_ref()?;
    resolved.classes.iter().find(|c| &c.name == class_name)
}

fn class_member_items(
    class: &basilisk_resolver::scope::ClassInfo,
    prefix: &str,
) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    for attr in &class.attributes {
        if !prefix.is_empty() && !attr.name.starts_with(prefix) {
            continue;
        }
        items.push(CompletionItem {
            label: attr.name.clone(),
            kind: Some(CompletionItemKind::FIELD),
            detail: Some(format!("{}.{}", class.name, attr.name)),
            ..Default::default()
        });
    }
    for method_name in &class.method_names {
        if !prefix.is_empty() && !method_name.starts_with(prefix) {
            continue;
        }
        items.push(CompletionItem {
            label: method_name.clone(),
            kind: Some(CompletionItemKind::METHOD),
            detail: Some(format!("{}.{}", class.name, method_name)),
            ..Default::default()
        });
    }
    items
}

// ── Keyword argument completions ──────────────────────────────────────────────

/// If the cursor is inside a function call (between `(` and `)`), return
/// `param_name=` completion items for each parameter of the called function
/// that hasn't already been supplied as a keyword argument.
fn kwarg_completions(
    resolved: &basilisk_resolver::ResolvedModule,
    text: &str,
    byte_offset: usize,
    prefix: &str,
) -> Option<Vec<CompletionItem>> {
    let callee = find_enclosing_call(text, byte_offset)?;
    let func = find_function(resolved, &callee)?;
    let already_provided = already_provided_kwargs(text, byte_offset);

    let items = func
        .parameters
        .iter()
        .filter(|p| !is_self_or_cls(&p.name))
        .filter(|p| !already_provided.contains(&p.name))
        .filter(|p| {
            let label = format!("{}=", p.name);
            prefix.is_empty() || label.starts_with(prefix) || p.name.starts_with(prefix)
        })
        .map(|p| CompletionItem {
            label: format!("{}=", p.name),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some(format!("keyword argument for {callee}")),
            ..Default::default()
        })
        .collect::<Vec<_>>();

    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}

/// Scan backwards from `byte_offset` to find an unmatched `(`, then extract
/// the callee name (the identifier immediately before the `(`).
fn find_enclosing_call(text: &str, byte_offset: usize) -> Option<String> {
    let before = &text[..byte_offset.min(text.len())];
    let mut depth: i32 = 0;
    let mut paren_pos = None;

    for (idx, ch) in before.char_indices().rev() {
        match ch {
            ')' => depth += 1,
            '(' => {
                if depth == 0 {
                    paren_pos = Some(idx);
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
    }

    let paren_idx = paren_pos?;
    let before_paren = &text[..paren_idx];
    let name: String = before_paren
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Look up a `FunctionInfo` by name in the resolved module.
fn find_function<'a>(
    resolved: &'a basilisk_resolver::ResolvedModule,
    name: &str,
) -> Option<&'a basilisk_resolver::scope::FunctionInfo> {
    resolved.functions.iter().find(|f| f.name == name)
}

/// Scan the text between the enclosing `(` and the cursor for keyword
/// arguments that have already been provided (patterns like `name=`).
fn already_provided_kwargs(text: &str, byte_offset: usize) -> HashSet<String> {
    let before = &text[..byte_offset.min(text.len())];
    let mut result = HashSet::new();

    // Find the unmatched `(` position
    let mut depth: i32 = 0;
    let mut paren_pos = None;
    for (idx, ch) in before.char_indices().rev() {
        match ch {
            ')' => depth += 1,
            '(' => {
                if depth == 0 {
                    paren_pos = Some(idx);
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
    }

    let Some(start) = paren_pos else {
        return result;
    };

    // Scan the argument region for `identifier=` patterns (not `==`)
    let args_region = &text[start + 1..byte_offset.min(text.len())];
    let chars: Vec<char> = args_region.chars().collect();
    let len = chars.len();
    let mut idx = 0;

    while idx < len {
        // Skip whitespace
        if chars[idx].is_whitespace() || chars[idx] == ',' {
            idx += 1;
            continue;
        }
        // Try to read an identifier
        if chars[idx].is_alphabetic() || chars[idx] == '_' {
            let start_idx = idx;
            while idx < len && (chars[idx].is_alphanumeric() || chars[idx] == '_') {
                idx += 1;
            }
            let ident: String = chars[start_idx..idx].iter().collect();
            // Check if followed by `=` but not `==`
            if idx < len && chars[idx] == '=' && (idx + 1 >= len || chars[idx + 1] != '=') {
                let _ = result.insert(ident);
            }
        } else {
            idx += 1;
        }
    }

    result
}

/// Returns `true` if the parameter name is `self` or `cls` (method receivers).
fn is_self_or_cls(name: &str) -> bool {
    name == "self" || name == "cls"
}

// ── Symbol completions (functions, classes, vars, imports, builtins) ──────────

fn symbol_completions(
    resolved: &basilisk_resolver::ResolvedModule,
    prefix: &str,
) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    let mut seen = HashSet::new();

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
            // For resolve, we include minimal data - docstrings can be loaded lazily
            let data = serde_json::json!({
                "kind": "function",
                "name": func.name
            });
            items.push(CompletionItem {
                label: func.name.clone(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some(detail),
                data: Some(serde_json::to_value(data).unwrap_or_default()),
                // Skip inline documentation - let resolve_completion_item load it lazily
                documentation: None,
                ..Default::default()
            });
        }
    }

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
                // Skip inline documentation - let resolve_completion_item load it lazily
                documentation: None,
                ..Default::default()
            });
        }
    }

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

    add_import_completions(resolved, prefix, &mut items, &mut seen);
    add_builtin_completions(&mut items, &mut seen, prefix);
    items
}

fn add_import_completions(
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
fn add_builtin_completions(
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

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only code: unwrap acceptable in unit tests"
)]
mod tests {
    use super::*;

    fn resolve_patched(code: &str, line: u32) -> basilisk_resolver::ResolvedModule {
        let patched = patch_cursor_line(code, line);
        let parsed = basilisk_parser::parse_source(patched, "/test.py".to_owned()).unwrap();
        basilisk_resolver::resolve(&parsed).unwrap()
    }

    #[test]
    fn test_dot_completion_dog_class() {
        let code = "class Dog:\n    name: str\n    breed: str\n    def bark(self) -> str:\n        return \"woof\"\n    def fetch(self, item: str) -> str:\n        return item\n\nDog.";
        // cursor at line 8, character 4 (after "Dog.")
        let byte_offset = code.len(); // end of text
        let resolved = resolve_patched(code, 8);
        let items = complete(&resolved, code, byte_offset);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"name"), "missing 'name': {labels:?}");
        assert!(labels.contains(&"breed"), "missing 'breed': {labels:?}");
        assert!(labels.contains(&"bark"), "missing 'bark': {labels:?}");
        assert!(labels.contains(&"fetch"), "missing 'fetch': {labels:?}");
    }

    #[test]
    fn test_dot_completion_self() {
        let code = "class Cat:\n    color: str\n    age: int\n    def meow(self) -> str:\n        return \"meow\"\n    def describe(self) -> str:\n        return self.";
        // cursor at end of text (after "self.")
        let byte_offset = code.len();
        let resolved = resolve_patched(code, 6);
        let items = complete(&resolved, code, byte_offset);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"color"), "missing 'color': {labels:?}");
        assert!(labels.contains(&"age"), "missing 'age': {labels:?}");
        assert!(labels.contains(&"meow"), "missing 'meow': {labels:?}");
        assert!(
            labels.contains(&"describe"),
            "missing 'describe': {labels:?}"
        );
    }
}
