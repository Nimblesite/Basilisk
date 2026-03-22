//! Basilisk CLI entry point.
//!
//! Usage:
//! ```
//! basilisk check [paths...]
//! basilisk check [paths...] --output json
//! ```

use std::collections::HashSet;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use tracing::{error, info, warn};

use crate::output::{render_diagnostics, render_diagnostics_json, FileSource, OutputFormat};

mod adopt;
mod fix;
mod output;

/// Basilisk — strict-by-default Python type checker.
///
/// No escape hatches. Every parameter typed. Every return declared.
#[derive(Parser)]
#[command(name = "basilisk", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Transport protocol for the LSP server.
#[derive(Clone, Debug, clap::ValueEnum)]
enum Transport {
    /// JSON-RPC over standard input/output (default).
    Stdio,
    /// JSON-RPC over WebSocket.
    Ws,
}

#[derive(Subcommand)]
enum Command {
    /// Type check one or more files or directories.
    Check {
        /// Paths to check. Directories are traversed recursively for `.py` files.
        #[arg(default_value = ".")]
        paths: Vec<String>,
        /// Output format: text (default, human-readable) or json (machine-readable).
        #[arg(long, default_value = "text")]
        output: OutputFormat,
    },
    /// Apply autofixes to one or more files or directories.
    Fix {
        /// Paths to fix. Directories are traversed recursively for `.py` files.
        #[arg(default_value = ".")]
        paths: Vec<String>,
        /// Include unsafe (heuristic) fixes alongside safe fixes.
        #[arg(long)]
        r#unsafe: bool,
        /// Comma-separated list of rule codes to fix (e.g. BSK-E0001,BSK-E0003).
        /// If omitted, all safe rules are applied. Use `--rules all` for all rules.
        #[arg(long, value_delimiter = ',')]
        rules: Vec<String>,
    },
    /// Adopt files — demote remaining errors to warnings for gradual migration.
    Adopt {
        /// Paths to adopt. Directories are traversed recursively for `.py` files.
        #[arg(default_value = ".")]
        paths: Vec<String>,
        /// Show adoption status instead of adopting.
        #[arg(long)]
        status: bool,
    },
    /// Un-adopt files — restore full strictness.
    Unadopt {
        /// Paths to un-adopt. Directories are traversed recursively for `.py` files.
        #[arg(default_value = ".")]
        paths: Vec<String>,
    },
    /// Start the Basilisk Language Server.
    Lsp {
        /// Transport protocol: stdio (default) or ws (WebSocket).
        #[arg(long, default_value = "stdio")]
        transport: Transport,
        /// Port for WebSocket transport (ignored for stdio).
        #[arg(long, default_value_t = 8765)]
        port: u16,
    },
}

fn main() -> ExitCode {
    // Initialize tracing. Controlled via BASILISK_LOG env var (defaults to info).
    // Examples: BASILISK_LOG=debug, BASILISK_LOG=basilisk_lsp::debug=trace
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("BASILISK_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    let exit_code: u8 = match cli.command {
        Command::Check { paths, output } => run_check(&paths, output),
        Command::Fix {
            paths,
            r#unsafe: include_unsafe,
            rules,
        } => fix::run_fix(&paths, include_unsafe, &rules),
        Command::Adopt { paths, status } => {
            if status {
                adopt::run_adopt_status(&paths)
            } else {
                adopt::run_adopt(&paths)
            }
        }
        Command::Unadopt { paths } => adopt::run_unadopt(&paths),
        Command::Lsp { transport, port } => match transport {
            Transport::Stdio => match basilisk_lsp::run_server() {
                Ok(()) => 0,
                Err(err) => {
                    error!(%err, "failed to start LSP server (stdio)");
                    1
                }
            },
            Transport::Ws => match basilisk_lsp::run_server_ws_blocking(port) {
                Ok(()) => 0,
                Err(err) => {
                    error!(%err, "failed to start LSP server (ws)");
                    1
                }
            },
        },
    };

    ExitCode::from(exit_code)
}

/// Run the check subcommand.
///
/// Exit codes:
/// - `0` — clean, no errors
/// - `1` — type errors found
/// - `3` — internal error
fn run_check(paths: &[String], format: OutputFormat) -> u8 {
    match collect_and_check(paths) {
        Ok((diagnostics, sources)) => match format {
            OutputFormat::Json => {
                render_diagnostics_json(&diagnostics, &sources);
                let error_count = diagnostics
                    .iter()
                    .filter(|d| d.severity == basilisk_checker::Severity::Error)
                    .count();
                u8::from(error_count > 0)
            }
            OutputFormat::Text => {
                let error_count = render_diagnostics(&diagnostics, &sources);
                let total = diagnostics.len();
                if total == 0 {
                    println!("All checked. No issues found.");
                    0
                } else {
                    println!(
                        "Found {} diagnostic{} ({} error{}).",
                        total,
                        pluralise(total),
                        error_count,
                        pluralise(error_count),
                    );
                    u8::from(error_count > 0)
                }
            }
        },
        Err(err) => {
            error!(%err, "internal error");
            3
        }
    }
}

fn collect_and_check(
    paths: &[String],
) -> Result<(Vec<basilisk_checker::Diagnostic>, Vec<FileSource>), String> {
    // Load config from the first path's directory (or cwd).
    let config_root = paths
        .first()
        .map(std::path::Path::new)
        .and_then(|p| {
            if p.is_dir() {
                Some(p.to_path_buf())
            } else {
                p.parent().map(std::path::Path::to_path_buf)
            }
        })
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let config = basilisk_config::load_basilisk_config(&config_root);

    let excluded: HashSet<&str> = config.exclude.iter().map(String::as_str).collect();
    info!(
        excluded_dirs = ?config.exclude,
        "loaded config from {}",
        config_root.display()
    );

    let python_files = collect_python_files(paths, &excluded)?;

    let mut all_diagnostics = Vec::new();
    let mut sources = Vec::new();

    for path in python_files {
        match process_file(&path) {
            Ok((diags, source)) => {
                all_diagnostics.extend(diags);
                sources.push(FileSource { path, text: source });
            }
            Err(err) => {
                warn!(path, %err, "error processing file");
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

/// Return `"s"` for counts != 1, empty string otherwise.
pub(crate) fn pluralise(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}

pub(crate) fn collect_python_files(
    paths: &[String],
    excluded: &HashSet<&str>,
) -> Result<Vec<String>, String> {
    let mut files = Vec::new();

    for root in paths {
        let meta = match std::fs::metadata(root) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(format!("cannot access {root}: {e}"));
            }
            Err(e) => {
                warn!(root, %e, "cannot access path");
                continue;
            }
        };

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
                .filter_entry(|e| {
                    if !e.file_type().is_dir() {
                        return true;
                    }
                    // Never exclude the root entry (depth 0) — the user
                    // explicitly asked to check this path (often `.`).
                    if e.depth() == 0 {
                        return true;
                    }
                    let name = e.file_name().to_string_lossy();
                    // Hidden directories are always excluded.
                    if name.starts_with('.') {
                        return false;
                    }
                    !excluded.contains(name.as_ref())
                })
                .filter_map(Result::ok)
                .filter(|e| e.file_type().is_file())
                .filter(|e| {
                    e.path()
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("py"))
                })
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

    /// Default excludes for test helpers.
    fn test_excludes() -> HashSet<&'static str> {
        basilisk_config::DEFAULT_EXCLUDES.iter().copied().collect()
    }

    // ── collect_python_files ──────────────────────────────────────────────────

    #[test]
    fn collect_python_files_returns_err_for_nonexistent_path() {
        let result = collect_python_files(&["/no/such/path/ever.py".to_owned()], &test_excludes());
        assert!(result.is_err(), "nonexistent path must return Err");
    }

    #[test]
    fn collect_python_files_skips_non_py_file() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir();
        let txt = dir.join("basilisk_test_skip.txt");
        std::fs::write(&txt, b"hello")?;
        let path = txt.to_string_lossy().into_owned();
        let files = collect_python_files(&[path], &test_excludes())?;
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
        let result = collect_and_check(&[path]);
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
        let files = collect_python_files(&[path], &test_excludes())?;
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
        let files = collect_python_files(&[path], &test_excludes())?;
        let _ = std::fs::remove_dir_all(&base);
        assert_eq!(
            files.len(),
            1,
            "directory walk must find exactly one .py file"
        );
        Ok(())
    }

    // ── collect_and_check: produces diagnostics for bad code ──────────────────

    #[test]
    fn collect_and_check_returns_diagnostics_for_bad_code() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = std::env::temp_dir();
        let py = dir.join("basilisk_test_bad_code.py");
        std::fs::write(&py, b"def foo(x):\n    pass\n")?;
        let path = py.to_string_lossy().into_owned();
        let (diags, _) = collect_and_check(&[path])?;
        let _ = std::fs::remove_file(&py);
        assert!(
            !diags.is_empty(),
            "unannotated function must produce diagnostics"
        );
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
        assert!(
            diags.is_empty(),
            "fully annotated code must produce no diagnostics"
        );
        Ok(())
    }

    // ── run_check: return value tests (lines 63, 65, 81) ────────────────────
    //
    // run_check returns:
    //   0  — no errors (Json: error_count == 0; Text: total == 0 OR error_count == 0)
    //   1  — errors found
    //   3  — internal error
    //
    // Mutants:
    //   line 63  == → !=  : Json path, error filter
    //   line 65  > → ==/</>= : Json path, i32::from(error_count > 0)
    //   line 81  > → >=   : Text path, i32::from(error_count > 0)

    /// `run_check` Json path: bad code must return 1.
    /// Kills `!=` at line 63 (which would invert the severity filter)
    /// and `== / < / >=` at line 65 (which would change the return value).
    #[test]
    fn run_check_json_bad_code_returns_one() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir();
        let py = dir.join("basilisk_test_rc_json_bad.py");
        std::fs::write(&py, b"def foo(x) -> None:\n    pass\n")?;
        let path = py.to_string_lossy().into_owned();
        let code = run_check(&[path], OutputFormat::Json);
        let _ = std::fs::remove_file(&py);
        assert_eq!(code, 1, "bad code must make run_check return 1 (Json)");
        Ok(())
    }

    /// `run_check` Json path: clean code must return 0.
    /// Kills `==` mutant at line 65 (which would return 1 for clean code).
    #[test]
    fn run_check_json_clean_code_returns_zero() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir();
        let py = dir.join("basilisk_test_rc_json_clean.py");
        std::fs::write(&py, b"def greet(name: str) -> str:\n    return name\n")?;
        let path = py.to_string_lossy().into_owned();
        let code = run_check(&[path], OutputFormat::Json);
        let _ = std::fs::remove_file(&py);
        assert_eq!(code, 0, "clean code must make run_check return 0 (Json)");
        Ok(())
    }

    /// `run_check` Text path: bad code must return 1.
    /// Kills `>=` mutant at line 81 (which always returns 1 since usize >= 0).
    #[test]
    fn run_check_text_bad_code_returns_one() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir();
        let py = dir.join("basilisk_test_rc_text_bad.py");
        std::fs::write(&py, b"def foo(x) -> None:\n    pass\n")?;
        let path = py.to_string_lossy().into_owned();
        let code = run_check(&[path], OutputFormat::Text);
        let _ = std::fs::remove_file(&py);
        assert_eq!(code, 1, "bad code must make run_check return 1 (Text)");
        Ok(())
    }

    /// `run_check` Text path: clean code must return 0.
    /// Kills `>=` mutant at line 81: if `> 0` became `>= 0`, clean code (count=0) would return 1.
    #[test]
    fn run_check_text_clean_code_returns_zero() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir();
        let py = dir.join("basilisk_test_rc_text_clean.py");
        std::fs::write(&py, b"def greet(name: str) -> str:\n    return name\n")?;
        let path = py.to_string_lossy().into_owned();
        let code = run_check(&[path], OutputFormat::Text);
        let _ = std::fs::remove_file(&py);
        assert_eq!(code, 0, "clean code must make run_check return 0 (Text)");
        Ok(())
    }

    /// `run_check` internal error path: nonexistent path must return 3.
    #[test]
    fn run_check_nonexistent_path_returns_three() {
        let code = run_check(&["/no/such/path.py".to_owned()], OutputFormat::Text);
        assert_eq!(code, 3, "nonexistent path must make run_check return 3");
    }

    // ── collect_python_files: MatchArmGuard mutant at main.rs:129 ────────────

    /// `collect_python_files` — `MatchArmGuard → true` at line 129.
    /// The guard `e.kind() == ErrorKind::NotFound` distinguishes "not found" from
    /// other I/O errors. If the guard is replaced with `true`, ALL I/O errors
    /// would return Err instead of continuing. We test that the `NotFound` path
    /// specifically returns Err (not Ok with empty list).
    #[test]
    fn collect_python_files_not_found_returns_err() {
        let result = collect_python_files(
            &["/absolutely/does/not/exist/file.py".to_owned()],
            &test_excludes(),
        );
        assert!(result.is_err(), "NotFound path must return Err, not Ok");
    }

    /// `run_check` Text path: warnings-only code must return 0 (no errors).
    #[test]
    fn run_check_text_warnings_only_returns_zero() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir();
        let py = dir.join("basilisk_test_rc_text_warn.py");
        // Demote the missing-param-annotation error to a warning via inline override.
        std::fs::write(
            &py,
            b"def foo(x) -> None:  # type: warning[BSK-E0001]\n    pass\n",
        )?;
        let path = py.to_string_lossy().into_owned();
        let code = run_check(&[path], OutputFormat::Text);
        let _ = std::fs::remove_file(&py);
        assert_eq!(
            code, 0,
            "warnings-only code must make run_check return 0 (Text)"
        );
        Ok(())
    }

    /// `run_check` Json path: warnings-only code must return 0 (no errors).
    #[test]
    fn run_check_json_warnings_only_returns_zero() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir();
        let py = dir.join("basilisk_test_rc_json_warn.py");
        std::fs::write(
            &py,
            b"def foo(x) -> None:  # type: warning[BSK-E0001]\n    pass\n",
        )?;
        let path = py.to_string_lossy().into_owned();
        let code = run_check(&[path], OutputFormat::Json);
        let _ = std::fs::remove_file(&py);
        assert_eq!(
            code, 0,
            "warnings-only code must make run_check return 0 (Json)"
        );
        Ok(())
    }

    /// Complement: a path that exists but is not .py returns Ok with empty list.
    /// This kills the `true` guard mutant: if all errors → Err, this would fail.
    #[test]
    fn collect_python_files_non_py_existing_file_returns_ok_empty(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir();
        let txt = dir.join("basilisk_test_guard_complement.txt");
        std::fs::write(&txt, b"hello")?;
        let path = txt.to_string_lossy().into_owned();
        let result = collect_python_files(&[path], &test_excludes());
        let _ = std::fs::remove_file(&txt);
        assert!(result.is_ok(), "existing non-py file must return Ok");
        assert!(result?.is_empty(), "non-py file must produce empty list");
        Ok(())
    }

    // ── directory exclusion ─────────────────────────────────────────────────

    #[test]
    fn collect_python_files_skips_excluded_directories() -> Result<(), Box<dyn std::error::Error>> {
        let base = std::env::temp_dir().join("basilisk_test_exclude_dirs");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base)?;

        // File in root — should be found.
        std::fs::write(base.join("app.py"), b"x = 1")?;

        // Files in default-excluded directories — should be skipped.
        for excluded in &["__pycache__", "venv", "site-packages", "node_modules"] {
            let sub = base.join(excluded);
            std::fs::create_dir_all(&sub)?;
            std::fs::write(sub.join("hidden.py"), b"x = 1")?;
        }

        // File in a hidden directory — should be skipped.
        let hidden = base.join(".hidden");
        std::fs::create_dir_all(&hidden)?;
        std::fs::write(hidden.join("secret.py"), b"x = 1")?;

        let path = base.to_string_lossy().into_owned();
        let files = collect_python_files(&[path], &test_excludes())?;
        let _ = std::fs::remove_dir_all(&base);

        assert_eq!(
            files.len(),
            1,
            "only root app.py should be found, got: {files:?}"
        );
        Ok(())
    }

    #[test]
    fn collect_python_files_respects_custom_excludes() -> Result<(), Box<dyn std::error::Error>> {
        let base = std::env::temp_dir().join("basilisk_test_custom_exclude");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base)?;

        std::fs::write(base.join("app.py"), b"x = 1")?;
        let sub = base.join("vendor");
        std::fs::create_dir_all(&sub)?;
        std::fs::write(sub.join("lib.py"), b"x = 1")?;

        // Custom exclude: only "vendor", not the defaults.
        let custom: HashSet<&str> = ["vendor"].into_iter().collect();
        let path = base.to_string_lossy().into_owned();
        let files = collect_python_files(&[path], &custom)?;
        let _ = std::fs::remove_dir_all(&base);

        assert_eq!(
            files.len(),
            1,
            "vendor should be excluded, only app.py found"
        );
        Ok(())
    }

    /// Regression: `basilisk check .` found zero files because the root
    /// entry `.` starts with `.` and was rejected by the hidden-dir filter.
    /// The same bug hits any user-supplied root whose name starts with `.`
    /// (e.g. `.myproject`).
    #[test]
    fn collect_python_files_hidden_root_dir_still_walked() -> Result<(), Box<dyn std::error::Error>>
    {
        let base = std::env::temp_dir().join("basilisk_test_hidden_root");
        let _ = std::fs::remove_dir_all(&base);

        // Root directory whose name starts with `.` — simulates the `.`
        // case (or any hidden-named project root).
        let hidden = base.join(".myproject");
        std::fs::create_dir_all(&hidden)?;
        std::fs::write(hidden.join("app.py"), b"x = 1")?;
        let sub = hidden.join("pkg");
        std::fs::create_dir_all(&sub)?;
        std::fs::write(sub.join("mod.py"), b"y = 2")?;

        let path = hidden.to_string_lossy().into_owned();
        let files = collect_python_files(&[path], &test_excludes())?;
        let _ = std::fs::remove_dir_all(&base);

        assert_eq!(
            files.len(),
            2,
            "user-supplied root starting with '.' must still be walked, got: {files:?}"
        );
        Ok(())
    }
}
