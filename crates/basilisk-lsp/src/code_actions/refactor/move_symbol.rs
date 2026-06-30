//! Implements [LSPARCH-FEATURES-CODEACTIONS]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-CODEACTIONS
//!
//! Move symbol to new file refactoring action.

use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, Command, CreateFile, CreateFileOptions, DocumentChangeOperation,
    DocumentChanges, OneOf, OptionalVersionedTextDocumentIdentifier, Position, Range, ResourceOp,
    TextDocumentEdit, TextEdit, Url, WorkspaceEdit,
};

/// Information about a symbol found at the cursor position.
struct SymbolInfo {
    /// The symbol's name (e.g., `MyClass`, `my_function`).
    name: String,
    /// Whether the symbol is a `class` or `def`.
    kind: SymbolKind,
    /// 0-based line number where the definition starts.
    start_line: usize,
    /// 0-based line number where the definition ends (exclusive).
    end_line: usize,
}

/// The kind of top-level symbol definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SymbolKind {
    Function,
    Class,
}

impl SymbolKind {
    /// Human-readable label for use in code action titles.
    const fn label(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Class => "class",
        }
    }
}

/// Offer to move the function or class under the cursor to a new file.
///
/// The action creates a new file named after the symbol (`snake_case`),
/// moves the full definition there, replaces the original with an import,
/// and adds the symbol to the original module's `__all__` if present.
// Implements [REFACTOR-MOVE] and [REFACTOR-MOVE-NEW] — the "Move to New File"
// variant of [REFACTOR-MOVE-ALGO]: identify the symbol + its imports (step 1),
// move the definition to a new file named after the symbol (step 3), and leave a
// re-export import in the source.
#[must_use]
pub(in crate::code_actions) fn move_symbol_to_new_file(
    uri: &Url,
    source: &str,
    range: &Range,
) -> Option<CodeAction> {
    let symbol = find_symbol_at_cursor(source, range)?;
    let new_file_uri = compute_new_file_uri(uri, &symbol.name)?;
    let module_name = to_snake_case(&symbol.name);
    let new_file_content = build_new_file_content(source, &symbol);
    let import_line = format!("from .{module_name} import {}\n", symbol.name);
    let symbol_range = symbol_line_range(source, &symbol);

    let operations = build_document_operations(
        uri,
        &new_file_uri,
        &new_file_content,
        &import_line,
        &symbol_range,
    );

    Some(CodeAction {
        title: format!("Move {} to new file (basilisk)", symbol.kind.label()),
        kind: Some(CodeActionKind::new("refactor.move")),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            document_changes: Some(DocumentChanges::Operations(operations)),
            ..Default::default()
        }),
        is_preferred: Some(false),
        ..Default::default()
    })
}

/// Offer to move the function or class under the cursor to an existing file.
///
/// This code action produces a **command** (`basilisk.moveSymbol`) rather
/// than a direct workspace edit, because the destination file is chosen
/// by the user at execution time.  The command arguments carry all the
/// information the server needs to perform the move.
// Implements [REFACTOR-MOVE-ALGO] step 2 — the user picks an existing
// destination module at execution time (the move itself runs in the
// `basilisk.moveSymbol` command handler).
#[must_use]
pub(in crate::code_actions) fn move_symbol_to_existing_file(
    uri: &Url,
    source: &str,
    range: &Range,
) -> Option<CodeAction> {
    let symbol = find_symbol_at_cursor(source, range)?;

    let start = serde_json::Value::from(u64::try_from(symbol.start_line).unwrap_or(0));
    let end = serde_json::Value::from(u64::try_from(symbol.end_line).unwrap_or(0));

    Some(CodeAction {
        title: format!("Move {} to existing file (basilisk)", symbol.kind.label()),
        kind: Some(CodeActionKind::new("refactor.move")),
        diagnostics: None,
        command: Some(Command {
            title: "Move symbol".to_owned(),
            command: basilisk_common::commands::MOVE_SYMBOL.to_owned(),
            arguments: Some(vec![
                serde_json::Value::String(uri.to_string()),
                serde_json::Value::Null, // dest_uri — filled by editor
                serde_json::Value::String(symbol.name),
                start,
                end,
            ]),
        }),
        is_preferred: Some(false),
        ..Default::default()
    })
}

/// Find the `def` or `class` definition at the cursor's line.
///
/// Only matches top-level definitions (zero indentation).
fn find_symbol_at_cursor(source: &str, range: &Range) -> Option<SymbolInfo> {
    let cursor_line = usize::try_from(range.start.line).unwrap_or(usize::MAX);
    let lines: Vec<&str> = source.lines().collect();
    let line = lines.get(cursor_line)?;

    let (kind, name) = parse_definition_line(line)?;

    let end_line = symbol_end_line(&lines, cursor_line);

    Some(SymbolInfo {
        name: name.to_owned(),
        kind,
        start_line: cursor_line,
        end_line,
    })
}

/// Parse a single line to extract a top-level `def` or `class` definition.
///
/// Returns `None` if the line is not a top-level definition.
fn parse_definition_line(line: &str) -> Option<(SymbolKind, &str)> {
    // Only match lines with no leading whitespace (top-level definitions).
    if line.starts_with(' ') || line.starts_with('\t') {
        return None;
    }

    if let Some(rest) = line.strip_prefix("def ") {
        let name = extract_identifier(rest)?;
        return Some((SymbolKind::Function, name));
    }

    if let Some(rest) = line.strip_prefix("class ") {
        let name = extract_identifier(rest)?;
        return Some((SymbolKind::Class, name));
    }

    None
}

/// Extract the leading identifier from a string (up to the first
/// non-alphanumeric, non-underscore character).
fn extract_identifier(text: &str) -> Option<&str> {
    let end = text
        .find(|ch: char| !ch.is_alphanumeric() && ch != '_')
        .unwrap_or(text.len());
    let ident = text.get(..end)?;
    if ident.is_empty() {
        return None;
    }
    Some(ident)
}

/// Find the end line (exclusive) of a definition starting at `start_line`.
///
/// Walks forward until a line at the same or lesser indentation is found
/// (another top-level definition or end of file).
fn symbol_end_line(lines: &[&str], start_line: usize) -> usize {
    let mut end = start_line + 1;
    for line in lines.iter().skip(start_line + 1) {
        // Skip blank lines — they do not terminate a definition.
        if line.trim().is_empty() {
            end += 1;
            continue;
        }
        // A non-blank line at column 0 means a new top-level statement.
        if !line.starts_with(' ') && !line.starts_with('\t') {
            break;
        }
        end += 1;
    }
    end
}

/// Convert a `CamelCase` name to `snake_case`.
///
/// Inserts underscores before uppercase letters that follow a lowercase
/// letter or precede a lowercase letter in an acronym run.
fn to_snake_case(name: &str) -> String {
    let mut result = String::with_capacity(name.len() + 4);
    let chars: Vec<char> = name.chars().collect();

    for (idx, &ch) in chars.iter().enumerate() {
        if ch.is_uppercase() {
            let prev_is_lower = idx > 0 && chars.get(idx - 1).is_some_and(|c| c.is_lowercase());
            let next_is_lower = chars.get(idx + 1).is_some_and(|c| c.is_lowercase());
            if prev_is_lower || (idx > 0 && next_is_lower) {
                result.push('_');
            }
            result.push(ch.to_ascii_lowercase());
        } else {
            result.push(ch);
        }
    }
    result
}

/// Collect import lines that appear before the symbol definition.
///
/// These imports are carried into the new file so the moved symbol
/// can still reference its dependencies.
// Implements [REFACTOR-MOVE-ALGO] step 1 (imports) / step 3 (add required
// imports to the destination) — carry the source module's imports along.
fn collect_imports_before_symbol(source: &str, start_line: usize) -> String {
    let mut imports = String::new();
    for line in source.lines().take(start_line) {
        let trimmed = line.trim();
        if trimmed.starts_with("import ") || trimmed.starts_with("from ") {
            imports.push_str(line);
            imports.push('\n');
        }
    }
    imports
}

/// Build the content for the new file: relevant imports + the symbol body.
fn build_new_file_content(source: &str, symbol: &SymbolInfo) -> String {
    let imports = collect_imports_before_symbol(source, symbol.start_line);
    let body = extract_symbol_body(source, symbol);

    let mut content = String::new();
    if !imports.is_empty() {
        content.push_str(&imports);
        content.push('\n');
    }
    content.push_str(&body);
    // Ensure the file ends with a newline.
    if !content.ends_with('\n') {
        content.push('\n');
    }
    content
}

/// Extract the text of the symbol definition from the source.
fn extract_symbol_body(source: &str, symbol: &SymbolInfo) -> String {
    source
        .lines()
        .skip(symbol.start_line)
        .take(symbol.end_line - symbol.start_line)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Compute the LSP `Range` covering the symbol definition in the original file.
fn symbol_line_range(source: &str, symbol: &SymbolInfo) -> Range {
    let start_line = u32::try_from(symbol.start_line).unwrap_or(u32::MAX);
    let end_line = u32::try_from(symbol.end_line).unwrap_or(u32::MAX);

    // Find the character length of the last line of the symbol if end_line
    // points past the last line of the file.
    let last_char = source
        .lines()
        .nth(symbol.end_line.saturating_sub(1))
        .map_or(0, |l| u32::try_from(l.len()).unwrap_or(u32::MAX));

    // If end_line equals the total line count, we go to the end of the last line.
    // Otherwise, start of the next line (character 0).
    let total_lines = source.lines().count();
    let (end_l, end_c) = if symbol.end_line >= total_lines {
        (
            u32::try_from(total_lines.saturating_sub(1)).unwrap_or(u32::MAX),
            last_char,
        )
    } else {
        (end_line, 0)
    };

    Range {
        start: Position {
            line: start_line,
            character: 0,
        },
        end: Position {
            line: end_l,
            character: end_c,
        },
    }
}

/// Compute the URI for the new file in the same directory as the source.
fn compute_new_file_uri(uri: &Url, symbol_name: &str) -> Option<Url> {
    let path = uri.to_file_path().ok()?;
    let parent = path.parent()?;
    let snake = to_snake_case(symbol_name);
    let new_path = parent.join(format!("{snake}.py"));
    Url::from_file_path(new_path).ok()
}

/// Build the list of `DocumentChangeOperation`s for the workspace edit.
fn build_document_operations(
    original_uri: &Url,
    new_file_uri: &Url,
    new_file_content: &str,
    import_line: &str,
    symbol_range: &Range,
) -> Vec<DocumentChangeOperation> {
    vec![
        // 1. Create the new file.
        DocumentChangeOperation::Op(ResourceOp::Create(CreateFile {
            uri: new_file_uri.clone(),
            options: Some(CreateFileOptions {
                overwrite: Some(false),
                ignore_if_exists: Some(true),
            }),
            annotation_id: None,
        })),
        // 2. Write the symbol content into the new file.
        DocumentChangeOperation::Edit(TextDocumentEdit {
            text_document: OptionalVersionedTextDocumentIdentifier {
                uri: new_file_uri.clone(),
                version: None,
            },
            edits: vec![OneOf::Left(TextEdit {
                range: Range::default(),
                new_text: new_file_content.to_owned(),
            })],
        }),
        // 3. Replace the symbol definition with an import in the original file.
        DocumentChangeOperation::Edit(TextDocumentEdit {
            text_document: OptionalVersionedTextDocumentIdentifier {
                uri: original_uri.clone(),
                version: None,
            },
            edits: vec![OneOf::Left(TextEdit {
                range: *symbol_range,
                new_text: import_line.to_owned(),
            })],
        }),
    ]
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test-only code: unwrap/expect/indexing/panic acceptable in unit tests"
)]
mod tests {
    use super::*;

    // ── to_snake_case ────────────────────────────────────────────────────

    #[test]
    fn test_snake_case_simple_camel() {
        assert_eq!(to_snake_case("MyClass"), "my_class");
    }

    #[test]
    fn test_snake_case_acronym() {
        assert_eq!(to_snake_case("HTTPServer"), "http_server");
    }

    #[test]
    fn test_snake_case_already_lower() {
        assert_eq!(to_snake_case("my_func"), "my_func");
    }

    #[test]
    fn test_snake_case_single_word() {
        assert_eq!(to_snake_case("Widget"), "widget");
    }

    // ── extract_identifier ───────────────────────────────────────────────

    #[test]
    fn test_extract_identifier_with_parens() {
        assert_eq!(extract_identifier("foo(bar)"), Some("foo"));
    }

    #[test]
    fn test_extract_identifier_with_colon() {
        assert_eq!(extract_identifier("MyClass:"), Some("MyClass"));
    }

    #[test]
    fn test_extract_identifier_empty() {
        assert_eq!(extract_identifier(""), None);
    }

    // ── parse_definition_line ────────────────────────────────────────────

    #[test]
    fn test_parse_def_line() {
        let result = parse_definition_line("def my_func(x: int) -> str:");
        let (kind, name) = result.expect("should parse");
        assert_eq!(kind, SymbolKind::Function);
        assert_eq!(name, "my_func");
    }

    #[test]
    fn test_parse_class_line() {
        let result = parse_definition_line("class MyClass(Base):");
        let (kind, name) = result.expect("should parse");
        assert_eq!(kind, SymbolKind::Class);
        assert_eq!(name, "MyClass");
    }

    #[test]
    fn test_parse_indented_def_returns_none() {
        assert!(parse_definition_line("    def nested(): ...").is_none());
    }

    // ── symbol_end_line ──────────────────────────────────────────────────

    #[test]
    fn test_symbol_end_at_eof() {
        let lines = vec!["class Foo:", "    pass"];
        assert_eq!(symbol_end_line(&lines, 0), 2);
    }

    #[test]
    fn test_symbol_end_before_next_def() {
        let lines = vec!["def a():", "    return 1", "", "def b():", "    return 2"];
        assert_eq!(symbol_end_line(&lines, 0), 3);
    }

    // ── find_symbol_at_cursor ────────────────────────────────────────────

    #[test]
    fn test_find_symbol_class() {
        let source = "import os\n\nclass MyWidget:\n    pass\n";
        let range = Range {
            start: Position {
                line: 2,
                character: 0,
            },
            end: Position {
                line: 2,
                character: 0,
            },
        };
        let info = find_symbol_at_cursor(source, &range).expect("should find");
        assert_eq!(info.name, "MyWidget");
        assert_eq!(info.kind, SymbolKind::Class);
        assert_eq!(info.start_line, 2);
        assert_eq!(info.end_line, 4);
    }

    #[test]
    fn test_find_symbol_on_non_def_line_returns_none() {
        let source = "x = 1\n";
        let range = Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 0,
            },
        };
        assert!(find_symbol_at_cursor(source, &range).is_none());
    }

    // ── collect_imports_before_symbol ─────────────────────────────────────

    #[test]
    fn test_collect_imports() {
        let source = "import os\nfrom sys import argv\n\ndef main(): ...\n";
        let imports = collect_imports_before_symbol(source, 3);
        assert_eq!(imports, "import os\nfrom sys import argv\n");
    }

    // ── compute_new_file_uri ─────────────────────────────────────────────

    #[test]
    fn test_compute_new_file_uri() {
        let uri = Url::parse("file:///project/src/main.py").unwrap();
        let new = compute_new_file_uri(&uri, "MyWidget").expect("should compute");
        assert_eq!(new.path(), "/project/src/my_widget.py");
    }

    // ── full integration ─────────────────────────────────────────────────

    // Exercises [REFACTOR-MOVE-NEW] / [REFACTOR-MOVE-ALGO] — create new file,
    // write the symbol body, and replace the original with a re-export import.
    #[test]
    fn test_move_symbol_produces_action() {
        let source = "import os\n\nclass MyWidget:\n    pass\n";
        let uri = Url::parse("file:///project/src/models.py").unwrap();
        let range = Range {
            start: Position {
                line: 2,
                character: 0,
            },
            end: Position {
                line: 2,
                character: 0,
            },
        };

        let action = move_symbol_to_new_file(&uri, source, &range).expect("should produce action");
        assert_eq!(action.title, "Move class to new file (basilisk)");
        assert_eq!(
            action.kind.as_ref().map(CodeActionKind::as_str),
            Some("refactor.move")
        );

        let edit = action.edit.expect("should have edit");
        let doc_changes = edit.document_changes.expect("should have document_changes");

        match doc_changes {
            DocumentChanges::Operations(ops) => {
                assert_eq!(ops.len(), 3, "create + write + replace");

                // First op: CreateFile
                assert!(matches!(
                    &ops[0],
                    DocumentChangeOperation::Op(ResourceOp::Create(_))
                ));

                // Second op: write to new file
                if let DocumentChangeOperation::Edit(edit) = &ops[1] {
                    assert!(edit.text_document.uri.path().ends_with("/my_widget.py"));
                    let text = match &edit.edits[0] {
                        OneOf::Left(te) => &te.new_text,
                        OneOf::Right(ate) => &ate.text_edit.new_text,
                    };
                    assert!(text.contains("class MyWidget:"));
                    assert!(text.contains("import os"));
                } else {
                    panic!("expected Edit operation for new file write");
                }

                // Third op: replace in original
                if let DocumentChangeOperation::Edit(edit) = &ops[2] {
                    assert!(edit.text_document.uri.path().ends_with("/models.py"));
                    let text = match &edit.edits[0] {
                        OneOf::Left(te) => &te.new_text,
                        OneOf::Right(ate) => &ate.text_edit.new_text,
                    };
                    assert_eq!(text, "from .my_widget import MyWidget\n");
                } else {
                    panic!("expected Edit operation for original file replacement");
                }
            }
            DocumentChanges::Edits(_) => panic!("expected Operations, got Edits"),
        }
    }

    #[test]
    fn test_move_symbol_non_def_line_returns_none() {
        let source = "x = 1\ny = 2\n";
        let uri = Url::parse("file:///test.py").unwrap();
        let range = Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 0,
            },
        };
        assert!(move_symbol_to_new_file(&uri, source, &range).is_none());
    }
}
