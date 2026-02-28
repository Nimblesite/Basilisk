//! Basilisk CLI entry point.
//!
//! Usage:
//! ```
//! basilisk check [paths...]
//! ```

use std::process;

use clap::{Parser, Subcommand};

use crate::output::{render_diagnostics, FileSource};

mod output;

/// Basilisk — strict-by-default Python type analyzer.
///
/// TypeScript for Python. Every parameter typed. Every return declared.
#[derive(Parser)]
#[command(name = "basilisk", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Type check one or more files or directories.
    Check {
        /// Paths to check. Directories are traversed recursively for `.py` files.
        #[arg(default_value = ".")]
        paths: Vec<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Command::Check { paths } => run_check(&paths),
    };

    process::exit(exit_code);
}

/// Run the check subcommand.
///
/// Exit codes:
/// - `0` — clean, no errors
/// - `1` — type errors found
/// - `3` — internal error
fn run_check(paths: &[String]) -> i32 {
    match collect_and_check(paths) {
        Ok((diagnostics, sources)) => {
            let error_count = render_diagnostics(&diagnostics, &sources);
            let total = diagnostics.len();
            if total == 0 {
                println!("All checked. No issues found.");
                0
            } else {
                println!(
                    "Found {} diagnostic{} ({} error{}).",
                    total,
                    if total == 1 { "" } else { "s" },
                    error_count,
                    if error_count == 1 { "" } else { "s" },
                );
                if error_count > 0 {
                    1
                } else {
                    0
                }
            }
        }
        Err(err) => {
            eprintln!("basilisk: internal error: {err}");
            3
        }
    }
}

fn collect_and_check(
    paths: &[String],
) -> Result<(Vec<basilisk_checker::Diagnostic>, Vec<FileSource>), String> {
    let python_files = collect_python_files(paths)?;

    let mut all_diagnostics = Vec::new();
    let mut sources = Vec::new();

    for path in python_files {
        match process_file(&path) {
            Ok((diags, source)) => {
                all_diagnostics.extend(diags);
                sources.push(FileSource { path, text: source });
            }
            Err(err) => {
                eprintln!("basilisk: error processing {path}: {err}");
            }
        }
    }

    Ok((all_diagnostics, sources))
}

fn process_file(path: &str) -> Result<(Vec<basilisk_checker::Diagnostic>, String), String> {
    let parsed = basilisk_parser::parse_file(path).map_err(|e| e.to_string())?;
    let source = parsed.source.clone();
    let resolved = basilisk_resolver::resolve(&parsed).map_err(|e| e.to_string())?;
    let diags = basilisk_checker::check(&resolved);
    Ok((diags, source))
}

fn collect_python_files(paths: &[String]) -> Result<Vec<String>, String> {
    let mut files = Vec::new();

    for root in paths {
        let meta = std::fs::metadata(root).map_err(|e| format!("cannot access {root}: {e}"))?;

        if meta.is_file() {
            if root.ends_with(".py") {
                files.push(root.clone());
            }
        } else {
            for entry in walkdir::WalkDir::new(root)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "py"))
            {
                files.push(entry.path().to_string_lossy().into_owned());
            }
        }
    }

    Ok(files)
}
