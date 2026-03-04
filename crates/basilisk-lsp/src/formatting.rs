//! Document formatting handler via Ruff delegation.
//!
//! Delegates to `ruff format` for Python document formatting, returning
//! a single `TextEdit` that replaces the entire document content.

use std::io::Write;
use std::process::{Command, Stdio};

use tower_lsp::lsp_types::{Position, Range, TextEdit};

/// Format a Python document by delegating to `ruff format`.
///
/// Spawns `ruff format --stdin-filename <file_path> -` and pipes the source
/// through stdin. Returns a single `TextEdit` replacing the entire document,
/// or `None` if the output is unchanged or Ruff is unavailable.
#[must_use]
pub fn format_document(source: &str, file_path: &str) -> Option<Vec<TextEdit>> {
    let mut child = Command::new("ruff")
        .args(["format", "--stdin-filename", file_path, "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    // Write source to stdin, then drop to close the pipe.
    child
        .stdin
        .as_mut()?
        .write_all(source.as_bytes())
        .ok()?;

    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }

    let formatted = String::from_utf8(output.stdout).ok()?;

    // No change needed.
    if formatted == source {
        return None;
    }

    // Compute the range spanning the entire original document.
    let line_count = source.lines().count();
    let last_line = if line_count == 0 { 0 } else { line_count - 1 };
    let last_col = source.lines().last().map_or(0, str::len);

    let last_line_u32 = u32::try_from(last_line).unwrap_or(u32::MAX);
    let last_col_u32 = u32::try_from(last_col).unwrap_or(u32::MAX);

    let range = Range {
        start: Position::new(0, 0),
        end: Position::new(last_line_u32, last_col_u32),
    };

    Some(vec![TextEdit {
        range,
        new_text: formatted,
    }])
}
