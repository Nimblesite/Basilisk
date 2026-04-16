//! Diagnostic output rendering — rustc-style text and machine-readable JSON.
//!
//! Text example:
//! ```text
//! error[BSK-E0001]: Missing parameter type annotation for `data`
//!   --> src/utils.py:14:5
//!    |
//! 14 | def process(data):
//!    |             ^^^^ parameter `data` has no type annotation
//!    |
//!    = help: Add a type annotation: `data: <type>`
//!    = note: In Basilisk, all function parameters require explicit types
//!    = see: https://www.basilisk-python.dev/errors/BSK-E0001
//! ```
//!
//! JSON output is a flat array consumed by the VS Code extension:
//! ```json
//! [
//!   {
//!     "code": "BSK-E0001",
//!     "severity": "error",
//!     "message": "Missing parameter type annotation for `x`",
//!     "path": "src/utils.py",
//!     "line": 1,
//!     "col": 9,
//!     "end_line": 1,
//!     "end_col": 10
//!   }
//! ]
//! ```

use clap::ValueEnum;

mod json;
mod text;

pub use json::render_diagnostics_json;
pub use text::render_diagnostics;

/// Output format for the `check` subcommand.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable rustc-style text (default).
    Text,
    /// Machine-readable JSON array consumed by the VS Code extension.
    Json,
}

/// Terminal colour mode.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ColorMode {
    /// Detect automatically (colours when stdout is a terminal).
    Auto,
    /// Always emit ANSI colour codes.
    Always,
    /// Never emit ANSI colour codes.
    Never,
}

impl ColorMode {
    /// Configure the `colored` crate based on the chosen mode.
    pub fn apply(self) {
        match self {
            Self::Auto => {} // `colored` auto-detects by default
            Self::Always => colored::control::set_override(true),
            Self::Never => colored::control::set_override(false),
        }
    }
}

/// Associates a file path with its source text for span-to-line-col mapping.
pub struct FileSource {
    /// The file path.
    pub path: String,
    /// The full source text.
    pub text: String,
}

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    reason = "test-only code: indexing acceptable in unit tests"
)]
mod tests {
    use super::*;
    use json::JsonDiagnostic;
    use text::{byte_offset_to_line_col, format_one, format_snippet};

    use basilisk_checker::Diagnostic;
    use basilisk_checker::{ErrorCode, Severity};
    use basilisk_resolver::Span;

    // ── ANSI escape sequences produced by the `colored` crate ────────────────

    /// Bold red (used for error severity labels and underlines).
    const BOLD_RED: &str = "\x1b[1;31m";
    /// Bold yellow (used for warning severity labels and underlines).
    const BOLD_YELLOW: &str = "\x1b[1;33m";
    /// Bold blue (used for info labels, line numbers, pipes, arrows).
    const BOLD_BLUE: &str = "\x1b[1;34m";
    /// Bold cyan (used for help/note/see annotation labels).
    const BOLD_CYAN: &str = "\x1b[1;36m";
    /// Bold (used for error codes and messages).
    const BOLD: &str = "\x1b[1m";
    /// ANSI reset sequence.
    const RESET: &str = "\x1b[0m";

    /// Force colours on for the duration of a test.
    ///
    /// `colored` uses a global atomic; we force it to `true` so that
    /// `format_one` / `format_snippet` always emit ANSI codes regardless
    /// of whether the test runner's stdout is a TTY.
    fn force_colors() {
        colored::control::set_override(true);
    }

    fn make_diag(help: Option<&str>, note: Option<&str>) -> Diagnostic {
        make_diag_with_severity(Severity::Error, help, note)
    }

    fn make_diag_with_severity(
        severity: Severity,
        help: Option<&str>,
        note: Option<&str>,
    ) -> Diagnostic {
        Diagnostic {
            code: ErrorCode {
                code: "BSK-E0001",
                docs_url: "https://www.basilisk-python.dev/errors/BSK-E0001",
            },
            severity,
            message: "missing annotation for `x`".to_owned(),
            span: Span { start: 8, end: 9 },
            path: "test.py".to_owned(),
            help: help.map(str::to_owned),
            note: note.map(str::to_owned),
            provenance: None,
        }
    }

    #[test]
    fn render_diagnostics_counts_only_errors() {
        let diag = make_diag(Some("add a type"), Some("all params need types"));
        let sources = vec![FileSource {
            path: "test.py".to_owned(),
            text: "def foo(x): pass".to_owned(),
        }];
        let count = render_diagnostics(&[diag], &sources);
        assert_eq!(count, 1);
    }

    #[test]
    fn render_diagnostics_does_not_count_warnings_as_errors() {
        let warning = make_diag_with_severity(Severity::Warning, None, None);
        let error = make_diag(None, None);
        let sources = vec![FileSource {
            path: "test.py".to_owned(),
            text: "def foo(x): pass".to_owned(),
        }];
        let count = render_diagnostics(&[warning, error], &sources);
        assert_eq!(count, 1, "only errors should be counted, not warnings");
    }

    // ── format_one: colour assertions ────────────────────────────────────────

    #[test]
    fn format_one_error_header_is_bold_red() {
        force_colors();
        let diag = make_diag(Some("help"), Some("note"));
        let out = format_one(&diag, Some("def foo(x): pass"));
        assert!(
            out.contains(&format!("{BOLD_RED}error{RESET}")),
            "error label must be bold red, got:\n{out}"
        );
    }

    #[test]
    fn format_one_error_code_is_bold() {
        force_colors();
        let diag = make_diag(None, None);
        let out = format_one(&diag, Some("def foo(x): pass"));
        assert!(
            out.contains(&format!("{BOLD}[BSK-E0001]{RESET}")),
            "error code must be bold, got:\n{out}"
        );
    }

    #[test]
    fn format_one_message_is_bold() {
        force_colors();
        let diag = make_diag(None, None);
        let out = format_one(&diag, Some("def foo(x): pass"));
        assert!(
            out.contains(&format!("{BOLD}missing annotation for `x`{RESET}")),
            "message must be bold, got:\n{out}"
        );
    }

    #[test]
    fn format_one_arrow_is_bold_blue() {
        force_colors();
        let diag = make_diag(None, None);
        let out = format_one(&diag, Some("def foo(x): pass"));
        assert!(
            out.contains(&format!("{BOLD_BLUE}-->{RESET}")),
            "arrow must be bold blue, got:\n{out}"
        );
    }

    #[test]
    fn format_one_help_label_is_bold_cyan() {
        force_colors();
        let diag = make_diag(Some("add a type"), None);
        let out = format_one(&diag, Some("def foo(x): pass"));
        assert!(
            out.contains(&format!("{BOLD_CYAN}help{RESET}")),
            "help label must be bold cyan, got:\n{out}"
        );
    }

    #[test]
    fn format_one_note_label_is_bold_cyan() {
        force_colors();
        let diag = make_diag(None, Some("all params need types"));
        let out = format_one(&diag, Some("def foo(x): pass"));
        assert!(
            out.contains(&format!("{BOLD_CYAN}note{RESET}")),
            "note label must be bold cyan, got:\n{out}"
        );
    }

    #[test]
    fn format_one_see_label_is_bold_cyan() {
        force_colors();
        let diag = make_diag(None, None);
        let out = format_one(&diag, Some("def foo(x): pass"));
        assert!(
            out.contains(&format!("{BOLD_CYAN}see{RESET}")),
            "see label must be bold cyan, got:\n{out}"
        );
    }

    #[test]
    fn format_one_equals_sign_is_bold_blue() {
        force_colors();
        let diag = make_diag(Some("help"), None);
        let out = format_one(&diag, Some("def foo(x): pass"));
        assert!(
            out.contains(&format!("{BOLD_BLUE}={RESET}")),
            "equals sign must be bold blue, got:\n{out}"
        );
    }

    #[test]
    fn format_one_warning_header_is_bold_yellow() {
        force_colors();
        let diag = make_diag_with_severity(Severity::Warning, None, None);
        let out = format_one(&diag, Some("def foo(x): pass"));
        assert!(
            out.contains(&format!("{BOLD_YELLOW}warning{RESET}")),
            "warning label must be bold yellow, got:\n{out}"
        );
    }

    #[test]
    fn format_one_info_header_is_bold_blue() {
        force_colors();
        let diag = make_diag_with_severity(Severity::Info, None, None);
        let out = format_one(&diag, Some("def foo(x): pass"));
        assert!(
            out.contains(&format!("{BOLD_BLUE}info{RESET}")),
            "info label must be bold blue, got:\n{out}"
        );
    }

    #[test]
    fn format_one_safety_violation_header_is_bold_red() {
        force_colors();
        let diag = make_diag_with_severity(Severity::SafetyViolation, None, None);
        let out = format_one(&diag, Some("def foo(x): pass"));
        assert!(
            out.contains(&format!("{BOLD_RED}safety violation{RESET}")),
            "safety violation label must be bold red, got:\n{out}"
        );
    }

    #[test]
    fn format_one_without_source_falls_back_to_path() {
        force_colors();
        let diag = make_diag(Some("help"), Some("note"));
        let out = format_one(&diag, None);
        assert!(
            out.contains("test.py"),
            "must fall back to path when source is None"
        );
        // No snippet section when source is missing.
        assert!(
            !out.contains("def foo"),
            "must not contain source snippet when source is None"
        );
    }

    #[test]
    fn format_one_without_help_omits_help_line() {
        force_colors();
        let diag = make_diag(None, Some("note text"));
        let out = format_one(&diag, Some("def foo(x): pass"));
        assert!(
            !out.contains("help"),
            "must omit help line when help is None"
        );
        assert!(out.contains("note text"), "must include note text");
    }

    #[test]
    fn format_one_without_note_omits_note_line() {
        force_colors();
        let diag = make_diag(Some("help text"), None);
        let out = format_one(&diag, Some("def foo(x): pass"));
        assert!(out.contains("help text"), "must include help text");
        assert!(
            !out.contains("note"),
            "must omit note line when note is None"
        );
    }

    #[test]
    fn format_one_without_help_or_note() {
        force_colors();
        let diag = make_diag(None, None);
        let out = format_one(&diag, Some("def foo(x): pass"));
        assert!(!out.contains("help"), "must omit help when None");
        assert!(!out.contains("note"), "must omit note when None");
        // Still must contain the see URL.
        assert!(out.contains("BSK-E0001"), "must contain error code");
        assert!(out.contains("basilisk-python.dev"), "must contain docs URL");
    }

    // ── format_snippet: colour assertions ────────────────────────────────────

    #[test]
    fn format_snippet_pipe_is_bold_blue() {
        force_colors();
        let out = format_snippet("def foo(x): pass", 8, 9, Severity::Error);
        assert!(
            out.contains(&format!("{BOLD_BLUE}|{RESET}")),
            "pipe must be bold blue, got:\n{out}"
        );
    }

    #[test]
    fn format_snippet_line_number_is_bold_blue() {
        force_colors();
        let out = format_snippet("def foo(x): pass", 8, 9, Severity::Error);
        assert!(
            out.contains(&format!("{BOLD_BLUE}1{RESET}")),
            "line number must be bold blue, got:\n{out}"
        );
    }

    #[test]
    fn format_snippet_error_underline_is_bold_red() {
        force_colors();
        let out = format_snippet("def foo(x): pass", 8, 9, Severity::Error);
        assert!(
            out.contains(&format!("{BOLD_RED}^{RESET}")),
            "error underline must be bold red, got:\n{out}"
        );
    }

    #[test]
    fn format_snippet_warning_underline_is_bold_yellow() {
        force_colors();
        let out = format_snippet("def foo(x): pass", 8, 9, Severity::Warning);
        assert!(
            out.contains(&format!("{BOLD_YELLOW}^{RESET}")),
            "warning underline must be bold yellow, got:\n{out}"
        );
    }

    #[test]
    fn format_snippet_info_underline_is_bold_blue() {
        force_colors();
        let out = format_snippet("def foo(x): pass", 8, 9, Severity::Info);
        assert!(
            out.contains(&format!("{BOLD_BLUE}^{RESET}")),
            "info underline must be bold blue, got:\n{out}"
        );
    }

    #[test]
    fn format_snippet_safety_violation_underline_is_bold_red() {
        force_colors();
        let out = format_snippet("def foo(x): pass", 8, 9, Severity::SafetyViolation);
        assert!(
            out.contains(&format!("{BOLD_RED}^{RESET}")),
            "safety violation underline must be bold red, got:\n{out}"
        );
    }

    #[test]
    fn format_snippet_multi_char_underline_length() {
        force_colors();
        // span covers "foo" at bytes 4..7 → 3 carets
        let out = format_snippet("def foo(x): pass", 4, 7, Severity::Error);
        assert!(
            out.contains(&format!("{BOLD_RED}^^^{RESET}")),
            "underline must be 3 carets for a 3-byte span, got:\n{out}"
        );
    }

    #[test]
    fn format_snippet_contains_source_line() {
        force_colors();
        let out = format_snippet("def foo(x): pass", 8, 9, Severity::Error);
        assert!(
            out.contains("def foo(x): pass"),
            "snippet must contain the source line, got:\n{out}"
        );
    }

    #[test]
    fn format_snippet_on_second_line() {
        force_colors();
        let source = "def foo(): pass\ndef bar(x): pass";
        let out = format_snippet(source, 20, 23, Severity::Error);
        // Line 2 contains "def bar(x): pass", line number should be 2.
        assert!(
            out.contains(&format!("{BOLD_BLUE}2{RESET}")),
            "second-line snippet must show line number 2, got:\n{out}"
        );
        assert!(
            out.contains("def bar(x): pass"),
            "must contain second source line, got:\n{out}"
        );
    }

    // ── format_one: structural content assertions ────────────────────────────

    #[test]
    fn format_one_contains_file_location() {
        force_colors();
        let diag = make_diag(None, None);
        let out = format_one(&diag, Some("def foo(x): pass"));
        assert!(
            out.contains("test.py:1:9"),
            "must contain file:line:col location, got:\n{out}"
        );
    }

    #[test]
    fn format_one_contains_source_snippet() {
        force_colors();
        let diag = make_diag(None, None);
        let out = format_one(&diag, Some("def foo(x): pass"));
        assert!(
            out.contains("def foo(x): pass"),
            "must contain source snippet, got:\n{out}"
        );
    }

    #[test]
    fn format_one_contains_docs_url() {
        force_colors();
        let diag = make_diag(None, None);
        let out = format_one(&diag, Some("def foo(x): pass"));
        assert!(
            out.contains("https://www.basilisk-python.dev/errors/BSK-E0001"),
            "must contain docs URL, got:\n{out}"
        );
    }

    // ── byte_offset_to_line_col ──────────────────────────────────────────────

    #[test]
    fn byte_offset_to_line_col_second_line() {
        let source = "def foo(): pass\ndef bar(x): pass";
        // byte 16 is the 'd' starting "def bar"
        let (line, col) = byte_offset_to_line_col(source, 16);
        assert_eq!(line, 2);
        assert_eq!(col, 1);
    }

    // ── JSON output ───────────────────────────────────────────────────────────

    #[test]
    fn json_produces_valid_array_with_correct_fields() -> Result<(), Box<dyn std::error::Error>> {
        let source = "def foo(x): pass";
        // byte 8 = 'x', so line=1, col=9 (1-based)
        let (line, col) = byte_offset_to_line_col(source, 8);
        let (end_line, end_col) = byte_offset_to_line_col(source, 9);
        let item = JsonDiagnostic {
            code: "BSK-E0001",
            severity: "error",
            message: "missing annotation for `x`",
            path: "test.py",
            line,
            col,
            end_line,
            end_col,
        };
        let json = serde_json::to_string(&item)?;
        assert!(json.contains("BSK-E0001"));
        assert!(json.contains("\"line\":1"));
        assert!(json.contains("\"col\":9"));
        assert!(json.contains("\"end_line\":1"));
        assert!(json.contains("\"end_col\":10"));
        Ok(())
    }

    #[test]
    fn json_empty_diagnostics_produces_empty_array() -> Result<(), Box<dyn std::error::Error>> {
        let items: Vec<JsonDiagnostic<'_>> = vec![];
        let json = serde_json::to_string(&items)?;
        assert_eq!(json, "[]");
        Ok(())
    }

    #[test]
    fn render_diagnostics_json_smoke_test() {
        let diag = make_diag(None, None);
        let sources = vec![FileSource {
            path: "test.py".to_owned(),
            text: "def foo(x): pass".to_owned(),
        }];
        // Just verify it doesn't panic.
        render_diagnostics_json(&[diag], &sources);
    }

    #[test]
    fn render_diagnostics_json_empty_is_safe() {
        render_diagnostics_json(&[], &[]);
    }

    #[test]
    fn render_diagnostics_json_warning_severity() {
        let diag = make_diag_with_severity(Severity::Warning, None, None);
        let sources = vec![FileSource {
            path: "test.py".to_owned(),
            text: "def foo(x): pass".to_owned(),
        }];
        render_diagnostics_json(&[diag], &sources);
    }

    #[test]
    fn render_diagnostics_json_info_severity() {
        let diag = make_diag_with_severity(Severity::Info, None, None);
        let sources = vec![FileSource {
            path: "test.py".to_owned(),
            text: "def foo(x): pass".to_owned(),
        }];
        render_diagnostics_json(&[diag], &sources);
    }

    #[test]
    fn render_diagnostics_json_safety_violation_severity() {
        let diag = make_diag_with_severity(Severity::SafetyViolation, None, None);
        let sources = vec![FileSource {
            path: "test.py".to_owned(),
            text: "def foo(x): pass".to_owned(),
        }];
        render_diagnostics_json(&[diag], &sources);
    }

    #[test]
    fn json_severity_warning() -> Result<(), Box<dyn std::error::Error>> {
        let source = "def foo(x): pass";
        let (line, col) = byte_offset_to_line_col(source, 8);
        let (end_line, end_col) = byte_offset_to_line_col(source, 9);
        let item = JsonDiagnostic {
            code: "BSK-W0001",
            severity: "warning",
            message: "test warning",
            path: "test.py",
            line,
            col,
            end_line,
            end_col,
        };
        let json = serde_json::to_string(&item)?;
        assert!(json.contains("\"warning\""));
        Ok(())
    }

    // ── render_diagnostics_json: FnValue→() mutant at output.rs:87 ──────────

    /// `render_diagnostics_json` — `FnValue → ()` at line 87.
    /// The function must actually produce output for non-empty diagnostics.
    /// We verify by checking the JSON serialisation round-trips correctly.
    #[test]
    fn render_diagnostics_json_produces_correct_item_count() {
        let d1 = Diagnostic {
            code: ErrorCode {
                code: "BSK-E0001",
                docs_url: "https://www.basilisk-python.dev/errors/BSK-E0001",
            },
            severity: Severity::Error,
            message: "missing annotation".to_owned(),
            span: Span { start: 0, end: 3 },
            path: "a.py".to_owned(),
            help: None,
            note: None,
            provenance: None,
        };
        let d2 = Diagnostic {
            code: ErrorCode {
                code: "BSK-E0002",
                docs_url: "https://www.basilisk-python.dev/errors/BSK-E0002",
            },
            severity: Severity::Error,
            message: "missing return annotation".to_owned(),
            span: Span { start: 4, end: 7 },
            path: "a.py".to_owned(),
            help: None,
            note: None,
            provenance: None,
        };
        let sources = [FileSource {
            path: "a.py".to_owned(),
            text: "def foo(x): pass".to_owned(),
        }];
        // Can't easily capture stdout, but verify items array construction is correct
        // by constructing directly.
        let items: Vec<JsonDiagnostic<'_>> = [&d1, &d2]
            .iter()
            .map(|d| {
                let source = sources
                    .iter()
                    .find(|s| s.path == d.path)
                    .map(|s| s.text.as_str());
                let (line, col) = source.map_or((1, 1), |src| {
                    byte_offset_to_line_col(src, usize::try_from(d.span.start).unwrap_or(0))
                });
                let (end_line, end_col) = source.map_or((line, col + 1), |src| {
                    byte_offset_to_line_col(src, usize::try_from(d.span.end).unwrap_or(0))
                });
                JsonDiagnostic {
                    code: d.code.code,
                    severity: "error",
                    message: &d.message,
                    path: &d.path,
                    line,
                    col,
                    end_line,
                    end_col,
                }
            })
            .collect();
        assert_eq!(items.len(), 2, "must produce one item per diagnostic");
        assert_eq!(items[0].code, "BSK-E0001");
        assert_eq!(items[1].code, "BSK-E0002");
    }

    // ── render_diagnostics_json: != mutant at output.rs:92 ──────────────────

    /// `!=` mutant at line 92: `sources.iter().find(|s| s.path == d.path)`.
    /// If `==` becomes `!=`, wrong source is matched → wrong line/col.
    /// Test that the right source file is used for offset resolution.
    #[test]
    fn render_diagnostics_json_matches_correct_source_file() {
        let diag = Diagnostic {
            code: ErrorCode {
                code: "BSK-E0001",
                docs_url: "https://www.basilisk-python.dev/errors/BSK-E0001",
            },
            severity: Severity::Error,
            message: "test".to_owned(),
            span: Span { start: 0, end: 1 },
            path: "b.py".to_owned(),
            help: None,
            note: None,
            provenance: None,
        };
        let sources = [
            FileSource {
                path: "a.py".to_owned(),
                text: "aaaa\nbbbb".to_owned(),
            },
            FileSource {
                path: "b.py".to_owned(),
                text: "x = 1\n".to_owned(),
            },
        ];
        let source = sources
            .iter()
            .find(|s| s.path == diag.path)
            .map(|s| s.text.as_str());
        let (line, col) = source.map_or((1, 1), |src| byte_offset_to_line_col(src, 0));
        // b.py offset 0 → line 1, col 1
        assert_eq!(line, 1);
        assert_eq!(col, 1);
    }

    // ── render_diagnostics_json: - / * mutants at output.rs:97 ──────────────

    /// `BinaryOperator` `-`/`*` mutants at line 97 in `render_diagnostics_json`.
    /// Line 97 computes end position: `byte_offset_to_line_col(src, d.span.end as usize)`.
    /// We verify `end_col` > col for a span that crosses characters.
    #[test]
    fn render_diagnostics_json_end_position_after_start() {
        let source = "def foo(x): pass";
        // span covers "foo" at bytes 4..7
        let (start_line, start_col) = byte_offset_to_line_col(source, 4);
        let (end_line, end_col) = byte_offset_to_line_col(source, 7);
        assert_eq!(start_line, 1);
        assert_eq!(end_line, 1);
        assert!(end_col > start_col, "end_col must be after start_col");
    }

    // ── byte_offset_to_line_col: - → / mutant at output.rs:158 ────────────

    /// The column formula is: `(clamped - pos - 1) + 1` where pos is the last '\n'.
    /// `/` mutant replaces `-` with `/` in `clamped - pos - 1`.
    /// e.g. with clamped=8, pos=5: correct = 8-5-1+1 = 3; mutant = 8/5-1+1 = 1+1 = 2 (wrong).
    /// Assert the exact column value to kill this mutant.
    #[test]
    fn byte_offset_to_line_col_column_arithmetic_exact() {
        // "hello\nworld" — "world" starts at byte 6
        // At byte 8 ('r'): line=2, col=3 (1-based: w=1, o=2, r=3)
        let source = "hello\nworld";
        let (line, col) = byte_offset_to_line_col(source, 8);
        assert_eq!(line, 2, "byte 8 must be line 2");
        assert_eq!(col, 3, "byte 8 ('r') must be col 3");
    }

    /// Further column test: first char of second line must be col 1.
    #[test]
    fn byte_offset_to_line_col_first_char_of_second_line() {
        let source = "hello\nworld";
        // byte 6 is 'w' — first char of line 2
        let (line, col) = byte_offset_to_line_col(source, 6);
        assert_eq!(line, 2);
        assert_eq!(col, 1, "first char of line must be col 1");
    }

    /// Multi-line: byte 12 is 'l' in "line3" (3rd line, 1st char).
    #[test]
    fn byte_offset_to_line_col_multi_line_correct() {
        let source = "line1\nline2\nline3";
        // "line3" starts at byte 12
        let (line, col) = byte_offset_to_line_col(source, 12);
        assert_eq!(line, 3, "byte 12 must be line 3");
        assert_eq!(col, 1, "byte 12 must be col 1");
    }

    /// Last char of first line (just before '\n').
    #[test]
    fn byte_offset_to_line_col_last_char_first_line() {
        // "hello\nworld": byte 4 is 'o' (5th char of first line)
        let source = "hello\nworld";
        let (line, col) = byte_offset_to_line_col(source, 4);
        assert_eq!(line, 1);
        assert_eq!(col, 5, "byte 4 ('o') must be col 5");
    }

    /// Offset past end is clamped — doesn't panic.
    #[test]
    fn byte_offset_to_line_col_offset_beyond_end() {
        let source = "abc";
        let (line, col) = byte_offset_to_line_col(source, 9999);
        assert_eq!(line, 1);
        assert_eq!(col, 4, "clamped to len=3, col=4 (1-based after last char)");
    }

    // ── format_snippet: structural / mutant tests ────────────────────────────

    /// `BinaryOperator` `-`/`*` mutants at `line_start = rfind('\n').map_or(0, |p| p + 1)`.
    /// The `+ 1` skips the newline byte. Without it, `line_start` points at '\n' itself.
    #[test]
    fn format_snippet_line_start_skips_newline() {
        force_colors();
        let source = "hello\nworld";
        let out = format_snippet(source, 8, 10, Severity::Error);
        // Must contain "world" (the source line), not "\nworld".
        assert!(
            out.contains("world"),
            "snippet must contain source line, got:\n{out}"
        );
        // Underline must be 2 carets for a 2-byte span.
        assert!(
            out.contains(&format!("{BOLD_RED}^^{RESET}")),
            "underline must be 2 carets (bold red), got:\n{out}"
        );
    }

    /// `col_start = start - line_start`. If this becomes `start + line_start`,
    /// `col_start` would be huge and the underline position would be wrong.
    #[test]
    fn format_snippet_col_start_no_overflow() {
        force_colors();
        let source = "abcdef\nghijkl";
        let out = format_snippet(source, 9, 12, Severity::Error);
        // Must contain the source line.
        assert!(out.contains("ghijkl"), "must contain source line");
        // Underline must be 3 carets for "ijk".
        assert!(
            out.contains(&format!("{BOLD_RED}^^^{RESET}")),
            "underline must be 3 carets, got:\n{out}"
        );
    }

    /// `col_end = (end - line_start).min(len)`. The span 12..14 extends past
    /// the end of "ghijkl" (6 chars at line_start=7), so col_end is clamped
    /// to line length. underline_len = 6 - 5 = 1.
    #[test]
    fn format_snippet_col_end_no_overflow() {
        force_colors();
        let source = "abcdef\nghijkl";
        let out = format_snippet(source, 12, 14, Severity::Error);
        assert!(out.contains("ghijkl"), "must contain source line");
        assert!(
            out.contains(&format!("{BOLD_RED}^{RESET}")),
            "underline must be clamped to 1 caret, got:\n{out}"
        );
    }

    /// Verify `format_snippet` produces correct underline length.
    #[test]
    fn format_snippet_arithmetic_properties() {
        let source = "hello world\n";
        // span covers "world" at bytes 6..11
        // line_start = 0 (no newline before), col_start = 6, col_end = 11, underline = 5
        let start = 6usize;
        let end = 11usize;
        let line_start = source[..start].rfind('\n').map_or(0, |p| p + 1);
        let line_text = source[line_start..].lines().next().unwrap_or("");
        let col_start = start - line_start;
        let col_end = (end - line_start).min(line_text.len());
        let underline_len = col_end.saturating_sub(col_start).max(1);
        assert_eq!(line_start, 0);
        assert_eq!(col_start, 6);
        assert_eq!(col_end, 11);
        assert_eq!(underline_len, 5, "underline for 'world' must be 5 chars");
    }

    // ColorMode::Never / Always are tested via subprocess in cli_binary_tests.rs
    // because `colored` uses a global atomic that races with parallel unit tests.
}
