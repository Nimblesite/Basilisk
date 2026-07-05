//! Implements [LSPARCH-FEATURES-FORMAT] and [LSPFMT-ENGINE].
//! See docs/specs/LSP-FORMATTING-SPEC.md#LSPFMT-ENGINE
//!
//! Document and range formatting via the **embedded** Ruff formatter.
//!
//! The `ruff_python_formatter` crate is linked into the binary at the same
//! pinned rev as the parser, so formatting works with no `ruff` executable
//! installed anywhere — no subprocess, no PATH lookup, no silent no-op
//! ([LSPFMT-DECISION], #254). Output is a pure passthrough of Ruff's
//! formatter ([LSPFMT-HONESTY]); style options come from the project's
//! `[tool.ruff]` / `[tool.ruff.format]` config ([LSPFMT-CONFIG]).

use ruff_formatter::{IndentStyle, LineWidth};
use ruff_python_ast::PySourceType;
use ruff_python_formatter::{MagicTrailingComma, PyFormatOptions, QuoteStyle};
use ruff_text_size::{TextRange, TextSize};
use tower_lsp::lsp_types::{Range, TextEdit};

use crate::config::FormatStyle;

/// Version of the embedded Ruff formatter, derived at compile time from the
/// pinned dependency rev by `build.rs` ([LSPFMT-PROVENANCE]). Surfaced in
/// `basilisk --version`, LSP `serverInfo.version`, and the log line emitted
/// on each format.
pub const EMBEDDED_RUFF_FORMATTER_VERSION: &str = env!("BASILISK_RUFF_FORMATTER_VERSION");

/// Translate the project's `[tool.ruff.format]` style into Ruff's options.
///
/// Unset fields keep Ruff's own defaults so output stays byte-identical to
/// what the user's `ruff format` would produce ([LSPFMT-ENGINE]).
fn py_format_options(style: &FormatStyle) -> PyFormatOptions {
    let mut options = PyFormatOptions::from_source_type(PySourceType::Python);
    if let Some(width) = style.line_length.and_then(|w| LineWidth::try_from(w).ok()) {
        options = options.with_line_width(width);
    }
    match style.quote_style.as_deref() {
        Some("single") => options = options.with_quote_style(QuoteStyle::Single),
        Some("preserve") => options = options.with_quote_style(QuoteStyle::Preserve),
        _ => {}
    }
    if style.indent_style.as_deref() == Some("tab") {
        options = options.with_indent_style(IndentStyle::Tab);
    }
    if style.skip_magic_trailing_comma {
        options = options.with_magic_trailing_comma(MagicTrailingComma::Ignore);
    }
    options
}

/// Format a whole Python document in-process with the embedded Ruff formatter.
///
/// Returns a single `TextEdit` replacing the entire document, or `None` when
/// the document is already formatted or does not parse.
#[must_use]
pub fn format_document(source: &str, style: &FormatStyle) -> Option<Vec<TextEdit>> {
    let formatted = ruff_python_formatter::format_module_source(source, py_format_options(style))
        .ok()?
        .into_code();
    if formatted == source {
        return None;
    }
    // [LSPFMT-PROVENANCE]: no silent magic — say which engine produced the bytes.
    tracing::info!(
        engine = "ruff",
        version = EMBEDDED_RUFF_FORMATTER_VERSION,
        "formatted with embedded Ruff formatter"
    );
    Some(vec![TextEdit {
        range: crate::code_actions::full_document_range(source),
        new_text: formatted,
    }])
}

/// Format a selection ("Format Selection") with the embedded Ruff formatter.
///
/// Ruff widens the requested range to whole logical lines; the returned edit
/// covers exactly the widened source range. Returns `None` when the selection
/// is already formatted or the source does not parse. [LSPFMT-CAPABILITIES]
#[must_use]
pub fn format_selection(source: &str, range: Range, style: &FormatStyle) -> Option<Vec<TextEdit>> {
    let start = crate::util::position_to_byte_offset(source, range.start);
    let end = crate::util::position_to_byte_offset(source, range.end);
    let text_range = TextRange::new(
        TextSize::try_from(start).ok()?,
        TextSize::try_from(end).ok()?,
    );

    let printed =
        ruff_python_formatter::format_range(source, text_range, py_format_options(style)).ok()?;
    let replaced = printed.source_range();
    let replaced_start = usize::from(replaced.start());
    let replaced_end = usize::from(replaced.end());
    if source.get(replaced_start..replaced_end) == Some(printed.as_code()) {
        return None;
    }
    tracing::info!(
        engine = "ruff",
        version = EMBEDDED_RUFF_FORMATTER_VERSION,
        "formatted selection with embedded Ruff formatter"
    );
    Some(vec![TextEdit {
        range: Range {
            start: crate::util::byte_offset_to_position(source, replaced_start),
            end: crate::util::byte_offset_to_position(source, replaced_end),
        },
        new_text: printed.into_code(),
    }])
}
