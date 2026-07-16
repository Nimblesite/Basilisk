//! Implements [LSPFMT-CLIENTS] and [CHKARCH-CLI-COMMANDS]. See
//! docs/specs/LSP-FORMATTING-SPEC.md#LSPFMT-CLIENTS
//! `basilisk format` subcommand — format Python files in place, or verify
//! them with `--check`, using the embedded Ruff formatter.
//!
//! Same engine, same style source as LSP `textDocument/formatting`
//! ([LSPFMT-ENGINE]): for identical input and configuration the output bytes
//! are identical. No `ruff` executable is ever spawned ([LSPFMT-DECISION]).

use basilisk_lsp::config::{FormatStyle, FormatterEngine};
use basilisk_lsp::formatting::{format_document, EMBEDDED_RUFF_FORMATTER_VERSION};
use tracing::warn;

use crate::pipeline::pluralise;

/// Run the format subcommand.
///
/// Exit codes:
/// - `0` — write mode completed, or check mode found every file formatted
/// - `1` — check mode found unformatted files, or a file failed to parse
/// - `3` — internal error (path collection, config discovery)
pub(crate) fn run_format(paths: &[String], check: bool) -> u8 {
    let config_root = crate::pipeline::first_path_dir(paths);
    let workspace = basilisk_lsp::config::load_config(&config_root);
    if workspace.formatter == FormatterEngine::Disabled {
        // [LSPFMT-CONFIG]: `"none"` disables formatting; mirror the LSP,
        // which stops advertising the formatting capabilities.
        println!("Formatter is disabled (formatter = \"none\"); nothing to do.");
        return 0;
    }
    match collect_and_format(paths, &workspace.format_style, check) {
        Ok(summary) => summarise(&summary, check),
        Err(err) => {
            tracing::error!(%err, "internal error");
            3
        }
    }
}

/// Outcome of formatting one file.
enum FileOutcome {
    /// The file was rewritten (write mode) or would be (check mode).
    Changed,
    /// The file is already formatted.
    Clean,
}

/// Result of a whole format run.
#[derive(Default)]
struct FormatSummary {
    /// Files rewritten (write mode) or needing a rewrite (check mode).
    changed: usize,
    /// Files already formatted.
    unchanged: usize,
    /// Files that could not be read or parsed.
    failures: usize,
}

/// Collect Python files under `paths` and format each one.
///
/// Path collection honours the same `[tool.basilisk]` `exclude` semantics as
/// `check` and `fix` ([CHKARCH-CONFIG-EXCLUDE]).
fn collect_and_format(
    paths: &[String],
    style: &FormatStyle,
    check: bool,
) -> Result<FormatSummary, String> {
    let config_root = crate::pipeline::first_path_dir(paths);
    let config = basilisk_config::load_basilisk_config(&config_root);
    let excluded = crate::pipeline::excluded_dirs_and_log(&config, &config_root);
    let python_files = crate::pipeline::collect_python_files(paths, &excluded)?;

    let mut summary = FormatSummary::default();
    for path in python_files {
        match format_single_file(&path, style, check) {
            Ok(FileOutcome::Changed) => summary.changed += 1,
            Ok(FileOutcome::Clean) => summary.unchanged += 1,
            Err(err) => {
                warn!(path, %err, "cannot format file");
                summary.failures += 1;
            }
        }
    }
    Ok(summary)
}

/// Format one file: rewrite it in write mode, report it in check mode.
fn format_single_file(path: &str, style: &FormatStyle, check: bool) -> Result<FileOutcome, String> {
    let source = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let Some(formatted) = formatted_text(&source, style) else {
        // `format_document` returns `None` for already-formatted AND for
        // unparseable sources; parse to tell them apart. Like `ruff format`,
        // invalid syntax is refused, never rewritten.
        return match basilisk_parser::parse_source(source, path.to_owned()) {
            Ok(_) => Ok(FileOutcome::Clean),
            Err(err) => Err(err.to_string()),
        };
    };
    if check {
        println!("Would reformat: {path}");
        return Ok(FileOutcome::Changed);
    }
    std::fs::write(path, formatted).map_err(|e| e.to_string())?;
    Ok(FileOutcome::Changed)
}

/// The full formatted text, or `None` when the source is already formatted
/// or does not parse ([LSPFMT-ENGINE] pure passthrough).
fn formatted_text(source: &str, style: &FormatStyle) -> Option<String> {
    format_document(source, style)?
        .into_iter()
        .next()
        .map(|edit| edit.new_text)
}

/// Print the run summary and derive the exit code.
///
/// The summary names the engine and version — the CLI face of the
/// provenance contract ([LSPFMT-PROVENANCE]).
fn summarise(summary: &FormatSummary, check: bool) -> u8 {
    let changed = summary.changed;
    let verb = if check {
        format!("{changed} file{} would be reformatted", pluralise(changed))
    } else {
        format!("Reformatted {changed} file{}", pluralise(changed))
    };
    println!(
        "{verb}, {} already formatted (embedded Ruff {EMBEDDED_RUFF_FORMATTER_VERSION}).",
        summary.unchanged
    );
    if summary.failures > 0 {
        println!(
            "{} file{} failed to parse and {} left unchanged.",
            summary.failures,
            pluralise(summary.failures),
            if summary.failures == 1 { "was" } else { "were" }
        );
    }
    u8::from(summary.failures > 0 || (check && changed > 0))
}
