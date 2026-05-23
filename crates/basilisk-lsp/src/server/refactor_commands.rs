//! Implements [LSPARCH-ARCH-MODSTRUCT]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-MODSTRUCT
//!
//! LSP command handlers for refactoring operations.
//!
//! Currently handles `basilisk.moveSymbol` which moves a top-level symbol
//! from one file to another, appending it to the destination and replacing
//! the original with an import statement.

use std::collections::HashMap;

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{Position, Range, TextEdit, Url, WorkspaceEdit};
use tracing::{error, info};

use super::LspServer;

/// Execute `basilisk.moveSymbol`.
///
/// Arguments: `[source_uri, dest_uri, symbol_name, start_line, end_line]`.
///
/// Reads the symbol body from the source file, appends it to the
/// destination file, and replaces the original with a relative import.
pub(super) async fn execute_move_symbol(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let Some(parsed) = parse_move_args(args) else {
        return Ok(Some(serde_json::json!({"error": "invalid arguments"})));
    };

    let source_text = read_document_text(server, &parsed.source_uri).await;
    let dest_text = read_document_text(server, &parsed.dest_uri).await;

    let symbol_body = extract_lines(&source_text, parsed.start_line, parsed.end_line);
    if symbol_body.trim().is_empty() {
        return Ok(Some(serde_json::json!({"error": "empty symbol body"})));
    }

    let import_line = build_import_line(&parsed.dest_uri, &parsed.symbol_name);
    let dest_append = build_dest_append(&dest_text, &symbol_body);

    let edit = build_move_edit(&parsed, &source_text, &import_line, &dest_append);

    apply_and_respond(server, edit, &parsed.symbol_name, &parsed.dest_uri).await
}

/// Apply the workspace edit and build the JSON response.
async fn apply_and_respond(
    server: &LspServer,
    edit: WorkspaceEdit,
    symbol_name: &str,
    dest_uri: &Url,
) -> LspResult<Option<serde_json::Value>> {
    match server.client.apply_edit(edit).await {
        Ok(resp) if resp.applied => {
            info!(symbol = %symbol_name, "moved symbol to {:?}", dest_uri.path());
            Ok(Some(serde_json::json!({"moved": symbol_name})))
        }
        Ok(resp) => {
            let reason = resp.failure_reason.unwrap_or_default();
            error!(reason = %reason, "move symbol edit rejected");
            Ok(Some(serde_json::json!({"error": reason})))
        }
        Err(err) => {
            error!(error = %err, "move symbol apply_edit failed");
            Ok(Some(serde_json::json!({"error": err.to_string()})))
        }
    }
}

// ── Argument parsing ─────────────────────────────────────────────────────────

/// Parsed arguments for the move-symbol command.
struct MoveArgs {
    source_uri: Url,
    dest_uri: Url,
    symbol_name: String,
    start_line: usize,
    end_line: usize,
}

/// Parse the 5 positional arguments from the command invocation.
fn parse_move_args(args: &[serde_json::Value]) -> Option<MoveArgs> {
    let source_str = args.first()?.as_str()?;
    let dest_str = args.get(1)?.as_str()?;
    let symbol_name = args.get(2)?.as_str()?.to_owned();
    let start_line = usize::try_from(args.get(3)?.as_u64()?).ok()?;
    let end_line = usize::try_from(args.get(4)?.as_u64()?).ok()?;

    let source_uri = Url::parse(source_str).ok()?;
    let dest_uri = Url::parse(dest_str).ok()?;

    Some(MoveArgs {
        source_uri,
        dest_uri,
        symbol_name,
        start_line,
        end_line,
    })
}

// ── Document reading ─────────────────────────────────────────────────────────

/// Read the current text of a document from the workspace index.
async fn read_document_text(server: &LspServer, uri: &Url) -> String {
    let guard = server.index.read().await;
    guard
        .as_ref()
        .and_then(|idx| idx.get_text(uri))
        .unwrap_or_default()
}

// ── Text helpers ─────────────────────────────────────────────────────────────

/// Extract lines `[start..end)` from the source text.
fn extract_lines(source: &str, start: usize, end: usize) -> String {
    source
        .lines()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Build a relative import line from the destination file's module name.
fn build_import_line(dest_uri: &Url, symbol_name: &str) -> String {
    let module = dest_uri
        .to_file_path()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "module".to_owned());
    format!("from .{module} import {symbol_name}\n")
}

/// Build the text to append at the end of the destination file.
///
/// Ensures PEP 8 blank-line separation (two blank lines before a top-level
/// definition) and a trailing newline.
fn build_dest_append(dest_text: &str, symbol_body: &str) -> String {
    let separator = if dest_text.is_empty() || dest_text.ends_with("\n\n") {
        String::new()
    } else if dest_text.ends_with('\n') {
        "\n".to_owned()
    } else {
        "\n\n".to_owned()
    };

    let mut result = separator;
    result.push_str(symbol_body);
    if !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// Build the `WorkspaceEdit` that moves the symbol.
fn build_move_edit(
    parsed: &MoveArgs,
    source_text: &str,
    import_line: &str,
    dest_append: &str,
) -> WorkspaceEdit {
    let source_range = line_range(source_text, parsed.start_line, parsed.end_line);
    let dest_end = end_of_file_position();

    let mut changes = HashMap::new();

    let _ = changes.insert(
        parsed.source_uri.clone(),
        vec![TextEdit {
            range: source_range,
            new_text: import_line.to_owned(),
        }],
    );

    let _ = changes.insert(
        parsed.dest_uri.clone(),
        vec![TextEdit {
            range: Range {
                start: dest_end,
                end: dest_end,
            },
            new_text: dest_append.to_owned(),
        }],
    );

    WorkspaceEdit {
        changes: Some(changes),
        ..Default::default()
    }
}

/// Compute the LSP `Range` for lines `[start..end)`.
fn line_range(source: &str, start: usize, end: usize) -> Range {
    let total_lines = source.lines().count();
    let start_u32 = u32::try_from(start).unwrap_or(u32::MAX);

    let (end_l, end_c) = if end >= total_lines {
        let last_char = source
            .lines()
            .last()
            .map_or(0, |l| u32::try_from(l.len()).unwrap_or(u32::MAX));
        (
            u32::try_from(total_lines.saturating_sub(1)).unwrap_or(0),
            last_char,
        )
    } else {
        (u32::try_from(end).unwrap_or(u32::MAX), 0)
    };

    Range {
        start: Position {
            line: start_u32,
            character: 0,
        },
        end: Position {
            line: end_l,
            character: end_c,
        },
    }
}

/// Position representing "end of file" — uses a very large line number.
///
/// The LSP spec says edits at positions past the end of the file are
/// clamped to the end, so this effectively means "append".
fn end_of_file_position() -> Position {
    Position {
        line: u32::MAX,
        character: 0,
    }
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test-only code: expect acceptable in unit tests"
)]
mod tests {
    use super::*;

    #[test]
    fn extract_lines_returns_correct_range() {
        let source = "line0\nline1\nline2\nline3\n";
        assert_eq!(extract_lines(source, 1, 3), "line1\nline2");
    }

    #[test]
    fn extract_lines_empty_range() {
        let source = "line0\nline1\n";
        assert_eq!(extract_lines(source, 1, 1), "");
    }

    #[test]
    fn extract_lines_past_end() {
        let source = "line0\nline1\n";
        assert_eq!(extract_lines(source, 0, 100), "line0\nline1");
    }

    #[test]
    fn build_import_line_extracts_module_stem() {
        let uri = Url::parse("file:///project/src/helpers.py").expect("valid URL");
        let result = build_import_line(&uri, "MyClass");
        assert_eq!(result, "from .helpers import MyClass\n");
    }

    #[test]
    fn build_dest_append_empty_dest() {
        let result = build_dest_append("", "def foo():\n    pass\n");
        assert_eq!(result, "def foo():\n    pass\n");
    }

    #[test]
    fn build_dest_append_single_trailing_newline() {
        let result = build_dest_append("existing\n", "def foo():\n    pass\n");
        assert_eq!(result, "\ndef foo():\n    pass\n");
    }

    #[test]
    fn build_dest_append_double_trailing_newline() {
        let result = build_dest_append("existing\n\n", "def foo():\n    pass\n");
        assert_eq!(result, "def foo():\n    pass\n");
    }

    #[test]
    fn build_dest_append_no_trailing_newline() {
        let result = build_dest_append("existing", "def foo():\n    pass");
        assert_eq!(result, "\n\ndef foo():\n    pass\n");
    }

    #[test]
    fn line_range_within_bounds() {
        let source = "aaa\nbbb\nccc\n";
        let range = line_range(source, 1, 2);
        assert_eq!(range.start.line, 1);
        assert_eq!(range.start.character, 0);
        assert_eq!(range.end.line, 2);
        assert_eq!(range.end.character, 0);
    }

    #[test]
    fn line_range_past_end() {
        let source = "aaa\nbbb";
        let range = line_range(source, 0, 100);
        assert_eq!(range.start.line, 0);
        assert_eq!(range.end.line, 1);
        assert_eq!(range.end.character, 3);
    }

    #[test]
    fn end_of_file_position_uses_max() {
        let pos = end_of_file_position();
        assert_eq!(pos.line, u32::MAX);
        assert_eq!(pos.character, 0);
    }

    #[test]
    fn parse_move_args_valid() {
        let args = vec![
            serde_json::json!("file:///src/a.py"),
            serde_json::json!("file:///src/b.py"),
            serde_json::json!("MyClass"),
            serde_json::json!(5),
            serde_json::json!(10),
        ];
        let parsed = parse_move_args(&args).expect("should parse");
        assert_eq!(parsed.symbol_name, "MyClass");
        assert_eq!(parsed.start_line, 5);
        assert_eq!(parsed.end_line, 10);
    }

    #[test]
    fn parse_move_args_missing_field() {
        let args = vec![serde_json::json!("file:///src/a.py")];
        assert!(parse_move_args(&args).is_none());
    }

    #[test]
    fn parse_move_args_empty() {
        assert!(parse_move_args(&[]).is_none());
    }

    #[test]
    fn build_move_edit_has_both_files() {
        let source_uri = Url::parse("file:///src/a.py").expect("valid URL");
        let dest_uri = Url::parse("file:///src/b.py").expect("valid URL");
        let parsed = MoveArgs {
            source_uri: source_uri.clone(),
            dest_uri: dest_uri.clone(),
            symbol_name: "foo".to_owned(),
            start_line: 0,
            end_line: 1,
        };
        let edit = build_move_edit(
            &parsed,
            "def foo():\n    pass\n",
            "from .b import foo\n",
            "def foo():\n    pass\n",
        );
        let changes = edit.changes.expect("should have changes");
        assert!(changes.contains_key(&source_uri));
        assert!(changes.contains_key(&dest_uri));
    }
}
