//! Implements [CHKARCH-CLI]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI
//! Basilisk CLI entry point.
//!
//! Usage:
//! ```
//! basilisk check [paths...]
//! basilisk check [paths...] --output json
//! basilisk analyze [paths...]
//! basilisk format [paths...] [--check]
//! ```

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use colored::Colorize as _;
use shipwright::{dispatch, BuildInfo, VersionSpec};
use shipwright_manifest::{ExecutableKind, Language};
use tracing::error;

use crate::output::{
    render_diagnostics, render_diagnostics_json, ColorMode, JsonFailure, OutputFormat,
};
use crate::pipeline::{collect_and_check, pluralise, DiagnosticScope, PipelineError};

mod adopt;
mod cache_check;
mod fix;
mod format;
mod import_search;
mod mcp;
mod output;
mod pipeline;
mod stubs;
mod typeshed_cli;

#[cfg(test)]
use stubs::{cache_stub, find_package_source, run as run_stubs, StubAction, StubGenModeArg};

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

/// The paths/`--output`/`--color`/`--cache*` surface shared verbatim by
/// `check` and `analyze` — the two commands run the identical pipeline and
/// differ only in diagnostic scope ([CHKARCH-COMMANDS]).
#[derive(clap::Args)]
struct CheckArgs {
    /// Paths to check. Directories are traversed recursively for `.py`
    /// files. Defaults to the configured `[tool.basilisk] include` roots,
    /// else the current directory.
    paths: Vec<String>,
    /// Output format: text (default, human-readable) or json (machine-readable).
    #[arg(long, default_value = "text")]
    output: OutputFormat,
    /// When to use terminal colours: auto (default), always, or never.
    #[arg(long, default_value = "auto")]
    color: ColorMode,
    /// Enable the opt-in result cache for this run, whatever
    /// `[tool.basilisk] cache` says: unchanged files are served from a
    /// persistent cache. A hit is returned only when the file, every file
    /// it reads, the config, and the checker version are unchanged.
    #[arg(long)]
    cache: bool,
    /// Disable the result cache for this run, whatever `[tool.basilisk] cache`
    /// says. Wins over `--cache` when both are given ([CHKCACHE-CONFIG]).
    #[arg(long)]
    no_cache: bool,
    /// Override the cache directory for this run (default: the project's
    /// `[tool.basilisk] cache-dir`, else `<project>/.basilisk/cache/check`).
    #[arg(long, value_name = "DIR")]
    cache_dir: Option<std::path::PathBuf>,
    /// Print cache hit/miss counts to stderr after checking.
    #[arg(long)]
    cache_stats: bool,
}

// Implements [CHKARCH-CLI-COMMANDS]: the `check`/`analyze` core commands
// ([CHKARCH-COMMANDS]) plus `format`/`fix`/`adopt`/`unadopt`/`lsp`/`stubs`.
// See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI-COMMANDS
#[derive(Subcommand)]
enum Command {
    /// Type check one or more files or directories — the PEP typing spec,
    /// always. Emits only `pep`-tagged rules ([CHKARCH-COMMANDS]).
    Check {
        #[command(flatten)]
        args: CheckArgs,
    },
    /// Run the opt-in analysis layer — every rule *not* tagged `pep`, fired
    /// only when configuration selects it ([CHKARCH-COMMANDS]).
    Analyze {
        #[command(flatten)]
        args: CheckArgs,
    },
    /// Format Python files with the embedded Ruff formatter — the same
    /// engine and style configuration as LSP formatting ([LSPFMT-CLIENTS]).
    Format {
        /// Paths to format. Directories are traversed recursively for `.py` files.
        #[arg(default_value = ".")]
        paths: Vec<String>,
        /// Report files that would change without rewriting them.
        #[arg(long)]
        check: bool,
    },
    /// Apply autofixes to one or more files or directories.
    Fix {
        /// Paths to fix. Directories are traversed recursively for `.py`
        /// files. Defaults to the configured `[tool.basilisk] include` roots,
        /// else the current directory.
        paths: Vec<String>,
        /// Include unsafe (heuristic) fixes alongside safe fixes.
        #[arg(long)]
        r#unsafe: bool,
        /// Comma-separated list of rule codes to fix (e.g. BSK-0001,BSK-0003).
        /// If omitted, all safe rules are applied. Use `--rules all` for all rules.
        #[arg(long, value_delimiter = ',')]
        rules: Vec<String>,
    },
    /// Adopt current error debt — demote firing error codes to folder-level
    /// warning entries for gradual migration ([AUTOFIX-ADOPTION]).
    Adopt {
        /// Paths to adopt. Directories are traversed recursively for `.py` files.
        #[arg(default_value = ".")]
        paths: Vec<String>,
        /// Show adoption status instead of adopting.
        #[arg(long)]
        status: bool,
    },
    /// Un-adopt — delete the folder-level warning entries, restoring the
    /// ancestor severity ([AUTOFIX-ADOPTION]).
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
    /// Serve read-only Basilisk status tools over Model Context Protocol stdio.
    Mcp {
        /// Workspace whose project configuration selects the typeshed source.
        #[arg(long, default_value = ".", value_name = "DIR")]
        workspace: std::path::PathBuf,
    },
    /// Manage the verified typeshed store. Downloading happens ONLY here (and
    /// via the editor's Download buttons) — checking never downloads
    /// ([STUBRES-TYPESHED-DOWNLOAD]).
    Typeshed {
        #[command(subcommand)]
        action: typeshed_cli::TypeshedAction,
    },
    /// Manage type stubs for untyped packages.
    Stubs {
        #[command(subcommand)]
        action: stubs::StubAction,
    },
    /// Generate a package stub using Pyright's compatibility spelling.
    #[command(name = "createstub", long_flag = "createstub")]
    CreateStub(stubs::CreateStubArgs),
}

/// Handle `--version` / `--version --json` via the Shipwright contract emitter.
///
/// Returns `true` when a version flag was handled and `main` should exit 0.
/// Build-time metadata is supplied by `build.rs`.
fn handle_version(args: &[String]) -> bool {
    let spec = VersionSpec {
        name: "basilisk",
        version: env!("CARGO_PKG_VERSION"),
        kind: ExecutableKind::Lsp,
        language: Language::Rust,
        product: Some("basilisk"),
        capabilities: &["cli", "lsp", "mcp", "dap", "profiler", "test-explorer"],
        build: BuildInfo {
            git_sha: option_env!("SHIPWRIGHT_GIT_SHA"),
            git_dirty: option_env!("SHIPWRIGHT_GIT_DIRTY").map(|s| s == "true"),
            build_time: option_env!("SHIPWRIGHT_BUILD_TIME"),
            target: option_env!("SHIPWRIGHT_TARGET"),
            toolchain: option_env!("SHIPWRIGHT_TOOLCHAIN"),
        },
    };
    match dispatch(args, &mut std::io::stdout(), &spec) {
        Ok(handled) => {
            // [LSPFMT-PROVENANCE]: the human-readable `--version` also lists
            // the embedded formatter engine. The `--json` payload stays a
            // pure Shipwright contract, so machine consumers are unaffected.
            if handled && !args.iter().any(|a| a == "--json") {
                let _ = std::io::Write::write_all(
                    &mut std::io::stdout(),
                    format!(
                        "Ruff formatter: {}\n",
                        basilisk_lsp::formatting::EMBEDDED_RUFF_FORMATTER_VERSION
                    )
                    .as_bytes(),
                );
            }
            handled
        }
        Err(err) => {
            let _ = std::io::Write::write_all(
                &mut std::io::stderr(),
                format!("basilisk: --version emission failed: {err}\n").as_bytes(),
            );
            // Don't swallow the error silently; surface to the user but
            // don't continue normal execution either.
            true
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if handle_version(&args) {
        return ExitCode::SUCCESS;
    }

    // Initialize tracing. Controlled via BASILISK_LOG env var (defaults to info).
    // Examples: BASILISK_LOG=debug, BASILISK_LOG=basilisk_lsp::debug=trace
    //
    // Only colourise when stderr is an interactive terminal. When the binary
    // runs as a subprocess (e.g. the LSP launched by the VS Code extension)
    // stderr is a pipe, and raw ANSI escapes would otherwise render as garbage
    // in the editor's output channel (issue #23).
    let tracing = tracing_subscriber::fmt()
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stderr()))
        .with_writer(std::io::stderr);
    if std::env::var_os("BASILISK_LOG").is_some() {
        tracing
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_env("BASILISK_LOG")
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
            )
            .init();
    } else {
        // The default path needs only warnings/errors. Avoid constructing and
        // parsing an EnvFilter on every short-lived CLI check.
        tracing.with_max_level(tracing::Level::WARN).init();
    }

    let cli = Cli::parse();

    // Command dispatch runs on an analysis-sized stack: `check`/`analyze`/
    // `fix`/`adopt` walk the AST recursively and overflow the default
    // main-thread stack (~8 MiB on macOS/Linux, ~1 MiB on Windows) on deeply
    // chained expressions in generated code. Implements [LSPARCH-ARCH-STACK]
    // (GitHub #278).
    let exit_code =
        match basilisk_lsp::runtime::run_with_analysis_stack("basilisk-cli", move || {
            run_command(cli.command)
        }) {
            Ok(code) => code,
            Err(err) => {
                error!(%err, "analysis thread failed");
                // 3 = internal failure ([CHKARCH-CLI-EXITCODES]). This path is
                // the analysis thread failing to run at all, which is never a
                // finding about the user's code — reporting 1 here would tell a
                // CI consumer "error diagnostics were found" when none were.
                3
            }
        };
    ExitCode::from(exit_code)
}

/// Dispatch the parsed subcommand. Returns the process exit code.
fn run_command(command: Command) -> u8 {
    match command {
        // [CHKARCH-COMMANDS]: identical pipeline, different edge filter.
        Command::Check { args } => run_scoped_check(&args, DiagnosticScope::Check),
        Command::Analyze { args } => run_scoped_check(&args, DiagnosticScope::Analyze),
        Command::Format { paths, check } => format::run_format(&paths, check),
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
        Command::Mcp { workspace } => match mcp::run(&workspace) {
            Ok(()) => 0,
            Err(err) => {
                error!(%err, "MCP server failed");
                1
            }
        },
        Command::Typeshed { action } => typeshed_cli::run(action),
        Command::Stubs { action } => stubs::run(action),
        Command::CreateStub(args) => stubs::run_create_stub(args),
    }
}

/// Run the `check`/`analyze` pipeline and render its outcome.
///
/// Implements [CHKARCH-CLI-EXITCODES]. Exit codes:
/// - `0` — clean, no errors
/// - `1` — error diagnostics found
/// - `2` — invalid configuration (a `pep` rule resolved to `disabled`,
///   [CHKARCH-CONFIG-MODEL])
/// - `3` — internal error
fn run_scoped_check(args: &CheckArgs, scope: DiagnosticScope) -> u8 {
    args.color.apply();
    let cache = cache_check::CacheOptions {
        enabled: cache_check::CacheOverride::from_flags(args.cache, args.no_cache),
        dir: args.cache_dir.clone(),
        stats: args.cache_stats,
    };
    let mut stats = cache_check::CacheStats::default();
    let result = collect_and_check(&args.paths, &cache, &mut stats, scope);
    if cache.stats {
        stats.report();
    }
    match result {
        // Implements [CHKARCH-CLI-OUTPUT]: the human-readable text default and
        // machine-readable JSON. The spec's `sarif`/`junit` formats are not
        // implemented (see report).
        Ok(outcome) => {
            let diagnostic_exit = render_outcome(&outcome, args.output);
            for failure in &outcome.failures {
                error!(path = %failure.path, error = %failure.message, "error processing file");
            }
            if outcome.failures.is_empty() {
                diagnostic_exit
            } else {
                3
            }
        }
        Err(PipelineError::Config(message)) => {
            error!(%message, "configuration error");
            2
        }
        Err(PipelineError::Internal(message)) => {
            error!(%message, "internal error");
            3
        }
    }
}

/// Tell the user which rules their configuration selected that this command
/// never evaluated, and where to see them.
///
/// Implements [CHKARCH-CLI-SCOPE-NOTICE] (GitHub #334). `check` drops every
/// analyze-scope diagnostic at the edge ([CHKARCH-COMMANDS]), so without this
/// line a project that grades eight rule tags `error` reads "All checked. No
/// issues found." while none of those rules ever ran. Text only: the JSON
/// contract machine consumers parse is unchanged.
fn print_scope_notice(unrun_selected_rules: usize) {
    if unrun_selected_rules == 0 {
        return;
    }
    println!(
        "{}",
        format!(
            "Note: your configuration selects {unrun_selected_rules} rule{} that `check` \
             never runs — they are not PEP typing-spec rules. Run `basilisk analyze` to \
             evaluate them.",
            pluralise(unrun_selected_rules),
        )
        .yellow()
    );
}

/// Render diagnostics in the requested format; `1` when errors exist, else `0`.
fn render_outcome(outcome: &pipeline::CheckOutcome, format: OutputFormat) -> u8 {
    match format {
        OutputFormat::Json => {
            let failures: Vec<JsonFailure<'_>> = outcome
                .failures
                .iter()
                .map(|failure| JsonFailure {
                    path: &failure.path,
                    message: &failure.message,
                })
                .collect();
            render_diagnostics_json(&outcome.diagnostics, &outcome.sources, &failures);
            let error_count = outcome
                .diagnostics
                .iter()
                .filter(|d| d.severity == basilisk_checker::Severity::Error)
                .count();
            u8::from(error_count > 0)
        }
        OutputFormat::Text => {
            let error_count = render_diagnostics(&outcome.diagnostics, &outcome.sources);
            let total = outcome.diagnostics.len();
            let exit_code = if total == 0 && outcome.failures.is_empty() {
                println!("{}", "All checked. No issues found.".green().bold());
                0
            } else if total == 0 {
                0
            } else {
                let summary = format!(
                    "Found {} diagnostic{} ({} error{}).",
                    total,
                    pluralise(total),
                    error_count,
                    pluralise(error_count),
                );
                if error_count > 0 {
                    println!("{}", summary.red().bold());
                } else {
                    println!("{}", summary.yellow().bold());
                }
                u8::from(error_count > 0)
            };
            print_scope_notice(outcome.unrun_selected_rules);
            exit_code
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test-only CLI contract assertions fail loudly with explicit messages"
)]
mod tests {
    use super::*;

    /// Unique temp dir for tests that need an isolated project root.
    fn unique_project_dir(prefix: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static CTR: AtomicU64 = AtomicU64::new(0);
        let n = CTR.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("{prefix}_{}_{n}", std::process::id()))
    }

    /// `CheckArgs` for a plain, uncached run over `paths`.
    fn plain_args(paths: Vec<String>, output: OutputFormat) -> CheckArgs {
        CheckArgs {
            paths,
            output,
            color: ColorMode::Never,
            cache: false,
            no_cache: false,
            cache_dir: None,
            cache_stats: false,
        }
    }

    /// [STUBRES-TYPESHED-OFFLINE]: the retired one-run waiver flags are gone —
    /// there is no cache to skip and no verification to switch off, so a
    /// command using them must fail to parse rather than silently no-op.
    #[test]
    fn check_cli_rejects_retired_typeshed_waiver_flags() {
        for flag in ["--no-typeshed-cache", "--no-typeshed-verification"] {
            assert!(
                Cli::try_parse_from(["basilisk", "check", "example.py", flag]).is_err(),
                "retired flag must be rejected: {flag}"
            );
        }
    }

    /// [STUBRES-TYPESHED-DOWNLOAD]: the download surface parses — latest by
    /// default, or one exact `--commit`.
    #[test]
    fn typeshed_download_cli_parses_latest_and_exact_forms() {
        let latest = Cli::try_parse_from(["basilisk", "typeshed", "download"])
            .expect("download latest must parse");
        let Command::Typeshed {
            action: typeshed_cli::TypeshedAction::Download { commit, .. },
        } = latest.command
        else {
            panic!("expected typeshed download");
        };
        assert!(commit.is_none(), "no --commit means latest");

        let exact = Cli::try_parse_from([
            "basilisk",
            "typeshed",
            "download",
            "--commit",
            "83c2518a9e6abbda0c44592c3483de459198f887",
        ])
        .expect("download --commit must parse");
        let Command::Typeshed {
            action: typeshed_cli::TypeshedAction::Download { commit, .. },
        } = exact.command
        else {
            panic!("expected typeshed download");
        };
        assert_eq!(
            commit.as_deref(),
            Some("83c2518a9e6abbda0c44592c3483de459198f887")
        );
    }

    /// An isolated project that opts the annotation house rule in, holding
    /// one file that violates it. Returns the dir and the file path.
    fn house_rule_project(prefix: &str) -> Result<(std::path::PathBuf, String), std::io::Error> {
        let dir = unique_project_dir(prefix);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(
            dir.join("pyproject.toml"),
            b"[tool.basilisk.rules]\n\"BSK-0001\" = \"error\"\n",
        )?;
        let py = dir.join("bad.py");
        std::fs::write(&py, b"def foo(x) -> None:\n    pass\n")?;
        Ok((dir, py.to_string_lossy().into_owned()))
    }

    // ── run_scoped_check exit codes ([CHKARCH-CLI-EXITCODES]) ──────────────

    /// [CHKARCH-COMMANDS]: an analyze-scope error (configured house rule)
    /// makes `analyze` exit 1 — in both output formats.
    #[test]
    fn analyze_bad_code_returns_one() -> Result<(), Box<dyn std::error::Error>> {
        let (dir, path) = house_rule_project("basilisk_test_rc_analyze_bad")?;
        let json = run_scoped_check(
            &plain_args(vec![path.clone()], OutputFormat::Json),
            DiagnosticScope::Analyze,
        );
        let text = run_scoped_check(
            &plain_args(vec![path], OutputFormat::Text),
            DiagnosticScope::Analyze,
        );
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(json, 1, "analyze-scope errors must exit 1 (Json)");
        assert_eq!(text, 1, "analyze-scope errors must exit 1 (Text)");
        Ok(())
    }

    /// [CHKARCH-COMMANDS]: `check` never sees house diagnostics, even when
    /// configuration selects them — the same file exits 0 under check.
    #[test]
    fn check_ignores_configured_house_rules() -> Result<(), Box<dyn std::error::Error>> {
        let (dir, path) = house_rule_project("basilisk_test_rc_check_scope")?;
        let code = run_scoped_check(
            &plain_args(vec![path], OutputFormat::Json),
            DiagnosticScope::Check,
        );
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(code, 0, "check must not exit 1 on analyze-scope debt");
        Ok(())
    }

    /// A pep-scope error (`return "x"` from `-> int`) makes `check` exit 1
    /// in both formats. [CHKARCH-COMMANDS]
    #[test]
    fn check_pep_error_returns_one() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_project_dir("basilisk_test_rc_check_pep");
        std::fs::create_dir_all(&dir)?;
        let py = dir.join("bad.py");
        std::fs::write(&py, b"def foo() -> int:\n    return \"x\"\n")?;
        let path = py.to_string_lossy().into_owned();
        let json = run_scoped_check(
            &plain_args(vec![path.clone()], OutputFormat::Json),
            DiagnosticScope::Check,
        );
        let text = run_scoped_check(
            &plain_args(vec![path], OutputFormat::Text),
            DiagnosticScope::Check,
        );
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(json, 1, "pep errors must make check exit 1 (Json)");
        assert_eq!(text, 1, "pep errors must make check exit 1 (Text)");
        Ok(())
    }

    /// Clean code exits 0 under both commands and formats.
    #[test]
    fn clean_code_returns_zero() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir();
        let py = dir.join("basilisk_test_rc_clean.py");
        std::fs::write(&py, b"def greet(name: str) -> str:\n    return name\n")?;
        let path = py.to_string_lossy().into_owned();
        for scope in [DiagnosticScope::Check, DiagnosticScope::Analyze] {
            for output in [OutputFormat::Text, OutputFormat::Json] {
                assert_eq!(
                    run_scoped_check(&plain_args(vec![path.clone()], output), scope),
                    0,
                    "clean code must exit 0 ({scope:?}, {output:?})"
                );
            }
        }
        let _ = std::fs::remove_file(&py);
        Ok(())
    }

    /// [CHKARCH-CONFIG-MODEL] / [CHKARCH-CLI-EXITCODES]: a config that
    /// resolves a pep rule to `disabled` is a configuration error — exit 2,
    /// for both commands, before any checking.
    #[test]
    fn pep_disable_config_returns_two() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_project_dir("basilisk_test_rc_pep_disable");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(
            dir.join("pyproject.toml"),
            b"[tool.basilisk.rules]\n\"imports_unresolved\" = \"disabled\"\n",
        )?;
        let py = dir.join("m.py");
        std::fs::write(&py, b"x: int = 1\n")?;
        let path = py.to_string_lossy().into_owned();
        for scope in [DiagnosticScope::Check, DiagnosticScope::Analyze] {
            assert_eq!(
                run_scoped_check(&plain_args(vec![path.clone()], OutputFormat::Json), scope),
                2,
                "pep-disable config must exit 2 ({scope:?})"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// Internal error path: nonexistent path must return 3.
    #[test]
    fn nonexistent_path_returns_three() {
        let code = run_scoped_check(
            &plain_args(vec!["/no/such/path.py".to_owned()], OutputFormat::Text),
            DiagnosticScope::Check,
        );
        assert_eq!(code, 3, "nonexistent path must exit 3");
    }

    /// Warnings-only code must return 0 (no errors) in both formats:
    /// an inline `# type: warning[...]` demotion of a pep error.
    #[test]
    fn warnings_only_returns_zero() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir();
        let py = dir.join("basilisk_test_rc_warn.py");
        std::fs::write(
            &py,
            b"import basilisk_no_such_module_xyz  # type: warning[imports_unresolved]\n",
        )?;
        let path = py.to_string_lossy().into_owned();
        for output in [OutputFormat::Text, OutputFormat::Json] {
            assert_eq!(
                run_scoped_check(
                    &plain_args(vec![path.clone()], output),
                    DiagnosticScope::Check
                ),
                0,
                "warnings-only code must exit 0 ({output:?})"
            );
        }
        let _ = std::fs::remove_file(&py);
        Ok(())
    }

    // ── stubs subcommand ─────────────────────────────────────────────────
    //
    // The `basilisk stubs` subsystem (run_stubs, cache_stub,
    // find_package_source) is exercised in-process here. Driving it directly
    // — rather than through a spawned binary — keeps its coverage independent
    // of subprocess profile merging, which is unreliable across platforms.
    // Implements [STUBRES-AUTOGEN] on the CLI surface.

    /// `find_package_source` returns `None` for a package that cannot be
    /// imported (the querying subprocess exits non-zero).
    #[test]
    fn find_package_source_returns_none_for_unknown_package() {
        let result = find_package_source(
            "basilisk_definitely_not_installed_pkg",
            std::path::Path::new("python3"),
        );
        assert!(result.is_none(), "unknown package must resolve to None");
    }

    /// `find_package_source` resolves an installed stdlib **package** to its
    /// `__init__.py` — exercising the success path (subprocess ok, dir parse,
    /// `__init__.py` exists). `json` is a package in every supported `CPython`.
    #[test]
    fn find_package_source_resolves_stdlib_package() {
        let result = find_package_source("json", std::path::Path::new("python3"));
        // Skip silently only if no USABLE interpreter is on PATH; otherwise the
        // success branch must resolve `json/__init__.py`. `output().is_ok()`
        // alone is not enough: the Windows Store `python3` execution alias
        // spawns successfully but only prints an install hint and exits
        // non-zero, so the guard must require the interpreter to actually run.
        let usable = std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_ok_and(|out| out.status.success());
        if usable {
            assert!(
                result.is_some_and(|p| p.ends_with("__init__.py")),
                "the `json` stdlib package must resolve to its __init__.py"
            );
        }
    }

    /// Package names are data, never Python source: a crafted name must not run
    /// code, while a valid dotted module must resolve to that module's own file.
    #[test]
    fn find_package_source_rejects_injection_and_resolves_dotted_module(
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
        {
            return Ok(());
        }

        let sentinel = unique_project_dir("basilisk_stub_source_injection").with_extension("txt");
        let _ = std::fs::remove_file(&sentinel);
        let sentinel_literal = format!("{:?}", sentinel.to_string_lossy());
        let malicious = format!("os; open({sentinel_literal}, 'w').write('executed') #");

        assert!(
            find_package_source(&malicious, std::path::Path::new("python3")).is_none(),
            "an invalid module name must be rejected"
        );
        let injection_ran = sentinel.exists();
        let _ = std::fs::remove_file(&sentinel);
        assert!(
            !injection_ran,
            "the package name must never execute as Python code"
        );

        let source = find_package_source("xml.etree.ElementTree", std::path::Path::new("python3"))
            .ok_or("dotted stdlib module did not resolve")?;
        assert_eq!(
            source.file_name(),
            Some(std::ffi::OsStr::new("ElementTree.py")),
            "a dotted module must resolve to its own source, not the top-level package"
        );
        Ok(())
    }

    /// `cache_stub` writes the stub and returns `true` on success.
    #[test]
    fn cache_stub_writes_and_returns_true() -> Result<(), Box<dyn std::error::Error>> {
        use basilisk_stubs::generate::{GeneratedStub, StubGenMode};
        let dir = unique_project_dir("basilisk_cli_cache_stub_ok");
        std::fs::create_dir_all(&dir)?;
        let stub = GeneratedStub {
            module_name: "widget".to_owned(),
            pyi_content: "def f() -> int: ...\n".to_owned(),
            mode: StubGenMode::Hybrid,
        };
        let ok = cache_stub(&dir, "widget", &stub);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(ok, "cache_stub must succeed writing to a writable dir");
        Ok(())
    }

    /// `cache_stub` returns `false` when the cache directory cannot be created
    /// because a regular file sits where a parent directory is required.
    #[test]
    fn cache_stub_returns_false_when_dir_uncreatable() -> Result<(), Box<dyn std::error::Error>> {
        use basilisk_stubs::generate::{GeneratedStub, StubGenMode};
        let base = unique_project_dir("basilisk_cli_cache_stub_fail");
        std::fs::create_dir_all(&base)?;
        // A regular file where a directory component is required downstream.
        let blocker = base.join("blocker");
        std::fs::write(&blocker, b"not a dir")?;
        let stub = GeneratedStub {
            module_name: "widget".to_owned(),
            pyi_content: "x: int\n".to_owned(),
            mode: StubGenMode::Ast,
        };
        // cache_dir nested under the regular file → `create_dir_all` must fail.
        let ok = cache_stub(&blocker.join("nested"), "widget", &stub);
        let _ = std::fs::remove_dir_all(&base);
        assert!(
            !ok,
            "cache_stub must return false when the cache dir is uncreatable"
        );
        Ok(())
    }

    /// `run_stubs(Status)` always reports without error (exit 0), whether or
    /// not any stubs are cached. Exercises the `Status` dispatch arm.
    #[test]
    fn run_stubs_status_returns_zero() {
        assert_eq!(
            run_stubs(StubAction::Status),
            0,
            "stubs status must return 0"
        );
    }

    /// `run_stubs(Generate { .. })` dispatches to generation; with no packages
    /// it returns 1. Exercises the `Generate` dispatch arm end to end.
    #[test]
    fn run_stubs_generate_dispatch_no_packages_returns_one() {
        let action = StubAction::Generate {
            packages: Vec::new(),
            all: false,
            mode: StubGenModeArg::Ast,
            python: "python3".to_owned(),
        };
        assert_eq!(
            run_stubs(action),
            1,
            "generate with no packages must return 1"
        );
    }

    // ── run_command dispatch ─────────────────────────────────────────────
    //
    // `run_command` is the parsed-subcommand dispatcher `main` delegates to on
    // the analysis stack. Driving each arm in-process — rather than only
    // through the spawned binary — keeps the dispatch covered independently
    // of subprocess profile merging. The `Lsp` arm is excluded on purpose: it
    // blocks on a running server.

    /// A temp project holding one clean, fully-annotated module. Returns the
    /// directory (to clean up) and the module's path.
    fn clean_project(
        prefix: &str,
    ) -> Result<(std::path::PathBuf, String), Box<dyn std::error::Error>> {
        let dir = unique_project_dir(prefix);
        std::fs::create_dir_all(&dir)?;
        // Anchor the project root at `dir` with a `pyproject.toml` marker.
        // Without one, `find_project_root` (which recognises only
        // `pyproject.toml`/`uv.lock`) walks past the temp dir and falls back
        // to the process cwd.
        std::fs::write(
            dir.join("pyproject.toml"),
            b"[project]\nname = \"fixture\"\nversion = \"0.0.0\"\n",
        )?;
        let py = dir.join("m.py");
        std::fs::write(&py, b"def greet(name: str) -> str:\n    return name\n")?;
        let path = py.to_string_lossy().into_owned();
        Ok((dir, path))
    }

    /// `run_command(Check)` (text) on clean code returns 0 and applies colour
    /// mode; `run_command(Analyze)` mirrors it ([CHKARCH-COMMANDS]).
    #[test]
    fn run_command_check_and_analyze_text_return_zero() -> Result<(), Box<dyn std::error::Error>> {
        let (dir, py) = clean_project("rc_check_text")?;
        let check = run_command(Command::Check {
            args: plain_args(vec![py.clone()], OutputFormat::Text),
        });
        let analyze = run_command(Command::Analyze {
            args: plain_args(vec![py], OutputFormat::Text),
        });
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(check, 0, "clean check (text) must return 0");
        assert_eq!(analyze, 0, "clean analyze (text) must return 0");
        Ok(())
    }

    /// `run_command(Check)` (json) on clean code returns 0.
    #[test]
    fn run_command_check_json_returns_zero() -> Result<(), Box<dyn std::error::Error>> {
        let (dir, py) = clean_project("rc_check_json")?;
        let code = run_command(Command::Check {
            args: CheckArgs {
                paths: vec![py],
                output: OutputFormat::Json,
                color: ColorMode::Always,
                cache: false,
                no_cache: false,
                cache_dir: None,
                cache_stats: false,
            },
        });
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(code, 0, "clean check (json) must return 0");
        Ok(())
    }

    /// `run_command(Check)` with the opt-in cache + stats exercises the cache
    /// context build, the cached check path, and the stats report.
    #[test]
    fn run_command_check_with_cache_and_stats_returns_zero(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (dir, py) = clean_project("rc_check_cache")?;
        let cache_dir = dir.join("cache");
        let code = run_command(Command::Check {
            args: CheckArgs {
                paths: vec![py],
                output: OutputFormat::Text,
                color: ColorMode::Auto,
                cache: true,
                no_cache: false,
                cache_dir: Some(cache_dir),
                cache_stats: true,
            },
        });
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(code, 0, "cached clean check must return 0");
        Ok(())
    }

    /// `run_command(Fix)` on clean code returns 0 (nothing to fix).
    #[test]
    fn run_command_fix_returns_zero() -> Result<(), Box<dyn std::error::Error>> {
        let (dir, py) = clean_project("rc_fix")?;
        let code = run_command(Command::Fix {
            paths: vec![py],
            r#unsafe: false,
            rules: Vec::new(),
        });
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(code, 0, "fixing clean code must return 0");
        Ok(())
    }

    /// `run_command(Adopt)` and `run_command(Adopt { status })` both succeed on
    /// a clean project — exercising both the adopt and status dispatch branch.
    /// [AUTOFIX-ADOPTION]
    #[test]
    fn run_command_adopt_and_status_return_zero() -> Result<(), Box<dyn std::error::Error>> {
        let (dir, py) = clean_project("rc_adopt")?;
        let adopt = run_command(Command::Adopt {
            paths: vec![py.clone()],
            status: false,
        });
        let status = run_command(Command::Adopt {
            paths: vec![py],
            status: true,
        });
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(adopt, 0, "adopting clean code must return 0");
        assert_eq!(status, 0, "adopt --status must return 0");
        Ok(())
    }

    /// `run_command(Unadopt)` on a clean project returns 0. [AUTOFIX-ADOPTION]
    #[test]
    fn run_command_unadopt_returns_zero() -> Result<(), Box<dyn std::error::Error>> {
        let (dir, py) = clean_project("rc_unadopt")?;
        let code = run_command(Command::Unadopt { paths: vec![py] });
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(code, 0, "unadopt on a clean project must return 0");
        Ok(())
    }

    /// `run_command(Stubs { Status })` reports without error.
    #[test]
    fn run_command_stubs_status_returns_zero() {
        assert_eq!(
            run_command(Command::Stubs {
                action: StubAction::Status,
            }),
            0,
            "stubs status via run_command must return 0"
        );
    }
}
