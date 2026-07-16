//! Implements [LSPARCH-FEATURES-COMPLETION]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-COMPLETION
//!
//! Completion (`IntelliSense`) handler.
//!
//! Provides symbol completions, dot completions, import completions,
//! keyword argument completions, and Python builtin completions.
//!
//! Supports lazy-loading of documentation via `completionItem/resolve`.

use tower_lsp::lsp_types::{CompletionItem, Documentation, MarkupContent, MarkupKind};

mod dot;
mod kwarg;
pub(crate) mod prefix;
mod symbol;

// ── Public API ───────────────────────────────────────────────────────────────

/// Compute completion items for a byte offset in the source.
#[must_use]
pub fn complete(
    resolved: &basilisk_resolver::ResolvedModule,
    text: &str,
    byte_offset: usize,
) -> Vec<CompletionItem> {
    let pfx = prefix::extract_prefix(text, byte_offset);
    if prefix::is_dot_completion(text, byte_offset) {
        dot::dot_completions(resolved, text, byte_offset)
    } else if let Some(kwarg_items) = kwarg::kwarg_completions(resolved, text, byte_offset, &pfx) {
        kwarg_items
    } else {
        symbol::symbol_completions(resolved, &pfx)
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
            #[expect(
                clippy::as_conversions,
                reason = "u32 line_number to usize for comparison with enumerate index"
            )]
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
    let (kind, name) = {
        let Some(data) = &item.data else {
            return item;
        };
        let kind = data
            .get("kind")
            .and_then(|k| k.as_str())
            .unwrap_or("")
            .to_owned();
        let name = data
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_owned();
        (kind, name)
    };

    if name.is_empty() {
        return item;
    }

    let Some(resolved) = try_resolve(text, path) else {
        return item;
    };

    match kind.as_str() {
        "function" | "method" => resolve_function(item, &resolved, &name),
        "class" => resolve_class(item, &resolved, &name),
        "variable" | "attribute" => resolve_variable(item, &resolved, &name),
        _ => item,
    }
}

fn resolve_function(
    item: CompletionItem,
    resolved: &basilisk_resolver::ResolvedModule,
    name: &str,
) -> CompletionItem {
    let Some(func) = resolved.functions.iter().find(|f| f.name == name) else {
        return item;
    };
    let mut result = item;
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
    if result.documentation.is_none() {
        if let Some(ref docstring) = func.docstring {
            result.documentation = Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: docstring.clone(),
            }));
        }
    }
    result
}

fn resolve_class(
    item: CompletionItem,
    resolved: &basilisk_resolver::ResolvedModule,
    name: &str,
) -> CompletionItem {
    let Some(class) = resolved.classes.iter().find(|c| c.name == name) else {
        return item;
    };
    let mut result = item;
    if result.detail.is_none() {
        let detail = if class.bases.is_empty() {
            "class".to_owned()
        } else {
            format!("class({})", class.bases.join(", "))
        };
        result.detail = Some(detail);
    }
    if result.documentation.is_none() {
        if let Some(ref docstring) = class.docstring {
            result.documentation = Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: docstring.clone(),
            }));
        }
    }
    result
}

fn resolve_variable(
    item: CompletionItem,
    resolved: &basilisk_resolver::ResolvedModule,
    name: &str,
) -> CompletionItem {
    if resolved.module_vars.iter().any(|v| v.name == name) {
        let mut result = item;
        if result.detail.is_none() {
            result.detail = Some("variable".to_owned());
        }
        return result;
    }
    item
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
        let byte_offset = code.len();
        let resolved = resolve_patched(code, 8);
        let items = complete(&resolved, code, byte_offset);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"name"), "missing 'name': {labels:?}");
        assert!(labels.contains(&"breed"), "missing 'breed': {labels:?}");
        assert!(labels.contains(&"bark"), "missing 'bark': {labels:?}");
        assert!(labels.contains(&"fetch"), "missing 'fetch': {labels:?}");
    }

    #[test]
    fn test_dot_completion_self_in_second_class() {
        // Regression for [LSPARCH-FEATURES-COMPLETION]: `self.` inside a class
        // defined AFTER another class must complete the ENCLOSING class's
        // members, not the first class in the file. Found by the
        // [VSIX-REALWORLD] rich corpus: `self.` inside Console (declared after
        // ConsoleOptions in rich/console.py) offered ConsoleOptions attributes.
        let code = "class Options:\n    width: int\n    def copy(self) -> str:\n        return \"copy\"\n\nclass Console:\n    file: str\n    def print_line(self) -> str:\n        return \"line\"\n    def log(self) -> str:\n        return self.";
        let byte_offset = code.len();
        let resolved = resolve_patched(code, 10);
        let items = complete(&resolved, code, byte_offset);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"file"), "missing 'file': {labels:?}");
        assert!(
            labels.contains(&"print_line"),
            "missing 'print_line': {labels:?}"
        );
        assert!(labels.contains(&"log"), "missing 'log': {labels:?}");
        assert!(
            !labels.contains(&"width") && !labels.contains(&"copy"),
            "leaked members of the FIRST class instead of the enclosing one: {labels:?}"
        );
    }

    #[test]
    fn test_dot_completion_self() {
        let code = "class Cat:\n    color: str\n    age: int\n    def meow(self) -> str:\n        return \"meow\"\n    def describe(self) -> str:\n        return self.";
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
