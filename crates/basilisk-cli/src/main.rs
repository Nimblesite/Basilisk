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
                i32::from(error_count > 0)
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
            if std::path::Path::new(root)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("py"))
            {
                files.push(root.clone());
            }
        } else {
            for entry in walkdir::WalkDir::new(root)
                .follow_links(false)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "py"))
            {
                files.push(entry.path().to_string_lossy().into_owned());
            }
        }
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── collect_python_files ──────────────────────────────────────────────────

    #[test]
    fn collect_python_files_returns_err_for_nonexistent_path() {
        let result = collect_python_files(&["/no/such/path/ever.py".to_owned()]);
        assert!(result.is_err(), "nonexistent path must return Err");
    }

    #[test]
    fn collect_python_files_skips_non_py_file() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir();
        let txt = dir.join("basilisk_test_skip.txt");
        std::fs::write(&txt, b"hello")?;
        let path = txt.to_string_lossy().into_owned();
        let files = collect_python_files(&[path])?;
        assert!(files.is_empty(), "non-.py file must be skipped");
        let _ = std::fs::remove_file(&txt);
        Ok(())
    }

    // ── collect_and_check: process_file error branch ─────────────────────────

    #[test]
    #[cfg(unix)]
    fn collect_and_check_handles_unreadable_py_file() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir();
        let py = dir.join("basilisk_test_locked.py");
        std::fs::write(&py, b"def foo(): pass")?;
        std::fs::set_permissions(&py, std::fs::Permissions::from_mode(0o000))?;

        let path = py.to_string_lossy().into_owned();
        // collect_and_check prints a warning to stderr and returns Ok with no diags.
        let result = collect_and_check(&[path]);
        // Restore permissions before asserting so cleanup can run even on failure.
        std::fs::set_permissions(&py, std::fs::Permissions::from_mode(0o644))?;
        let _ = std::fs::remove_file(&py);

        let (diags, _) = result?;
        assert!(
            diags.is_empty(),
            "unreadable file produces no diagnostics, got: {diags:#?}"
        );
        Ok(())
    }

    // ── collect_python_files: .py file is included ────────────────────────────

    #[test]
    fn collect_python_files_includes_py_file() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir();
        let py = dir.join("basilisk_test_include.py");
        std::fs::write(&py, b"x = 1")?;
        let path = py.to_string_lossy().into_owned();
        let files = collect_python_files(&[path])?;
        let _ = std::fs::remove_file(&py);
        assert_eq!(files.len(), 1, ".py file must be included");
        Ok(())
    }

    // ── collect_python_files: directory traversal ─────────────────────────────

    #[test]
    fn collect_python_files_walks_directory() -> Result<(), Box<dyn std::error::Error>> {
        let base = std::env::temp_dir().join("basilisk_test_walk_dir");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base)?;
        std::fs::write(base.join("a.py"), b"x = 1")?;
        std::fs::write(base.join("b.txt"), b"ignored")?;
        let path = base.to_string_lossy().into_owned();
        let files = collect_python_files(&[path])?;
        let _ = std::fs::remove_dir_all(&base);
        assert_eq!(files.len(), 1, "directory walk must find exactly one .py file");
        Ok(())
    }

    // ── collect_and_check: produces diagnostics for bad code ──────────────────

    #[test]
    fn collect_and_check_returns_diagnostics_for_bad_code(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir();
        let py = dir.join("basilisk_test_bad_code.py");
        std::fs::write(&py, b"def foo(x):\n    pass\n")?;
        let path = py.to_string_lossy().into_owned();
        let (diags, _) = collect_and_check(&[path])?;
        let _ = std::fs::remove_file(&py);
        assert!(!diags.is_empty(), "unannotated function must produce diagnostics");
        Ok(())
    }

    // ── collect_and_check: clean code produces no diagnostics ─────────────────

    #[test]
    fn collect_and_check_returns_no_diagnostics_for_clean_code(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir();
        let py = dir.join("basilisk_test_clean_code.py");
        std::fs::write(&py, b"def greet(name: str) -> str:\n    return name\n")?;
        let path = py.to_string_lossy().into_owned();
        let (diags, _) = collect_and_check(&[path])?;
        let _ = std::fs::remove_file(&py);
        assert!(diags.is_empty(), "fully annotated code must produce no diagnostics");
        Ok(())
    }
}
