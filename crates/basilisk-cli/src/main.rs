//! Implements [CHKARCH-CLI]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI
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
use colored::Colorize as _;
use shipwright::{dispatch, BuildInfo, VersionSpec};
use shipwright_manifest::{ExecutableKind, Language};
use tracing::{error, info, warn};

use crate::output::{
    render_diagnostics, render_diagnostics_json, ColorMode, FileSource, OutputFormat,
};

mod adopt;
mod cache_check;
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

// Implements [CHKARCH-CLI-COMMANDS]: the `check` core command (with `--watch`
// deferred — see report). `fix`/`adopt`/`unadopt`/`lsp`/`stubs` extend the spec
// list; the spec's `stats`/`migrate`/`init` commands are not implemented.
// See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI-COMMANDS
#[derive(Subcommand)]
enum Command {
    /// Type check one or more files or directories.
    Check {
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
        /// Enable the opt-in result cache: unchanged files are served from a
        /// persistent cache. A hit is returned only when the file, every file
        /// it reads, the config, and the checker version are unchanged.
        #[arg(long)]
        cache: bool,
        /// Override the cache directory (default: `<project>/.basilisk/cache/check`).
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<std::path::PathBuf>,
        /// Print cache hit/miss counts to stderr after checking.
        #[arg(long)]
        cache_stats: bool,
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
    /// Manage type stubs for untyped packages.
    Stubs {
        #[command(subcommand)]
        action: StubAction,
    },
}

/// Stub management sub-commands.
#[derive(Subcommand)]
enum StubAction {
    /// Generate best-effort `.pyi` stubs for untyped packages.
    Generate {
        /// Package names to generate stubs for.
        /// Use `--all` to generate for all untyped imports.
        packages: Vec<String>,
        /// Generate stubs for all untyped imports in the project.
        #[arg(long)]
        all: bool,
        /// Generation mode: runtime (inspect.signature), ast (parse .py), or hybrid (default).
        #[arg(long, default_value = "hybrid")]
        mode: StubGenModeArg,
        /// Path to the Python interpreter.
        #[arg(long, default_value = "python3")]
        python: String,
    },
    /// Show stub coverage status for the project.
    Status,
}

/// CLI-friendly stub generation mode.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum StubGenModeArg {
    Runtime,
    Ast,
    Hybrid,
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
        capabilities: &["cli", "lsp", "dap", "profiler", "test-explorer"],
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
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("BASILISK_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stderr()))
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    // Command dispatch runs on an analysis-sized stack: `check`/`fix`/`adopt`
    // walk the AST recursively and overflow the default main-thread stack
    // (~8 MiB on macOS/Linux, ~1 MiB on Windows) on deeply chained
    // expressions in generated code. Implements [LSPARCH-ARCH-STACK]
    // (GitHub #278).
    let exit_code =
        match basilisk_lsp::runtime::run_with_analysis_stack("basilisk-cli", move || {
            run_command(cli.command)
        }) {
            Ok(code) => code,
            Err(err) => {
                error!(%err, "analysis thread failed");
                1
            }
        };
    ExitCode::from(exit_code)
}

/// Dispatch the parsed subcommand. Returns the process exit code.
fn run_command(command: Command) -> u8 {
    match command {
        Command::Check {
            paths,
            output,
            color,
            cache,
            cache_dir,
            cache_stats,
        } => {
            color.apply();
            let cache_options = cache_check::CacheOptions {
                enabled: cache,
                dir: cache_dir,
                stats: cache_stats,
            };
            run_check(&paths, output, &cache_options)
        }
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
        Command::Stubs { action } => run_stubs(action),
    }
}

/// Run the stubs subcommand.
fn run_stubs(action: StubAction) -> u8 {
    match action {
        StubAction::Generate {
            packages,
            all,
            mode,
            python,
        } => run_stubs_generate(&packages, all, mode, &python),
        StubAction::Status => run_stubs_status(),
    }
}

/// Cache a generated stub and print the result. Returns `true` on success.
fn cache_stub(
    cache_dir: &std::path::Path,
    package: &str,
    stub: &basilisk_stubs::generate::GeneratedStub,
) -> bool {
    use basilisk_stubs::generate::cache;

    let source_hash = cache::hash_source(&stub.pyi_content);
    match cache::write_cache(cache_dir, package, &stub.pyi_content, source_hash) {
        Ok(path) => {
            println!(
                "{} Generated stub for `{package}` → {}",
                "✓".green(),
                path.display()
            );
            true
        }
        Err(err) => {
            eprintln!("{} Failed to write stub for `{package}`: {err}", "✗".red());
            false
        }
    }
}

/// Generate stubs for specified packages.
fn run_stubs_generate(packages: &[String], all: bool, mode: StubGenModeArg, python: &str) -> u8 {
    use basilisk_stubs::generate::{self, StubGenMode};

    let gen_mode = match mode {
        StubGenModeArg::Runtime => StubGenMode::Runtime,
        StubGenModeArg::Ast => StubGenMode::Ast,
        StubGenModeArg::Hybrid => StubGenMode::Hybrid,
    };

    let python_path = std::path::Path::new(python);
    let cache_dir = std::path::Path::new(generate::cache::DEFAULT_CACHE_DIR);

    if all {
        warn!("--all not yet implemented; specify package names explicitly");
        return 1;
    }

    if packages.is_empty() {
        eprintln!("{}: specify package names or use --all", "error".red());
        return 1;
    }

    let mut errors = 0u8;
    for package in packages {
        let source_path = find_package_source(package, python_path);

        let ok = match source_path {
            Some(ref src) => {
                info!(package, source = %src.display(), "generating stubs");
                match generate::generate_stubs(package, src, python_path, gen_mode) {
                    Ok(stub) => cache_stub(cache_dir, package, &stub),
                    Err(err) => {
                        eprintln!(
                            "{} Failed to generate stub for `{package}`: {err}",
                            "✗".red()
                        );
                        false
                    }
                }
            }
            None if gen_mode == StubGenMode::Ast => {
                eprintln!(
                    "{} Cannot find source for `{package}` — AST mode requires source files",
                    "✗".red()
                );
                false
            }
            None => match generate::runtime::generate_runtime_stubs(package, python_path) {
                Ok(stub) => cache_stub(cache_dir, package, &stub),
                Err(err) => {
                    eprintln!(
                        "{} Failed to generate stub for `{package}`: {err}",
                        "✗".red()
                    );
                    false
                }
            },
        };
        if !ok {
            errors = errors.saturating_add(1);
        }
    }

    errors.min(1)
}

/// Find the source path for an installed package by querying Python.
fn find_package_source(package: &str, python_path: &std::path::Path) -> Option<std::path::PathBuf> {
    let output = std::process::Command::new(python_path)
        .args([
            "-c",
            &format!("import {package}; import os; print(os.path.dirname({package}.__file__))"),
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let dir = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let init = std::path::PathBuf::from(&dir).join("__init__.py");
    if init.exists() {
        Some(init)
    } else {
        // Single-file module.
        let single = std::path::PathBuf::from(format!("{dir}.py"));
        if single.exists() {
            Some(single)
        } else {
            None
        }
    }
}

/// Show stub coverage status.
fn run_stubs_status() -> u8 {
    let cache_dir = std::path::Path::new(basilisk_stubs::generate::cache::DEFAULT_CACHE_DIR);

    if !cache_dir.exists() {
        println!("No generated stubs found ({})", cache_dir.display());
        return 0;
    }

    let mut count = 0u32;
    if let Ok(entries) = std::fs::read_dir(cache_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "pyi") {
                let module = path.file_stem().and_then(|s| s.to_str()).unwrap_or("?");
                println!("  {} {module}", "✓".green());
                count = count.saturating_add(1);
            }
        }
    }

    if count == 0 {
        println!("No generated stubs found");
    } else {
        println!("\n{count} generated stub(s) in {}", cache_dir.display());
    }

    0
}

/// Run the check subcommand.
///
/// Implements [CHKARCH-CLI-EXITCODES]. Exit codes:
/// - `0` — clean, no errors
/// - `1` — type errors found
/// - `3` — internal error
///
/// Note: the spec's exit code `2` (configuration error) is not produced — a
/// malformed config silently falls back to defaults rather than erroring (see
/// report). See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI-EXITCODES
fn run_check(paths: &[String], format: OutputFormat, cache: &cache_check::CacheOptions) -> u8 {
    let mut stats = cache_check::CacheStats::default();
    let result = collect_and_check(paths, cache, &mut stats);
    if cache.stats {
        stats.report();
    }
    match result {
        // Implements [CHKARCH-CLI-OUTPUT]: the human-readable text default and
        // machine-readable JSON. The spec's `sarif`/`junit` formats are not
        // implemented (see report).
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
                    println!("{}", "All checked. No issues found.".green().bold());
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
                }
            }
        },
        Err(err) => {
            error!(%err, "internal error");
            3
        }
    }
}

/// Resolve the paths a check run walks. Implements [CHKARCH-CONFIG-INCLUDE]:
/// explicit CLI paths win, then the configured `include` roots, then `.`.
fn effective_check_paths(
    paths: &[String],
    config: &basilisk_config::BasiliskConfig,
    config_root: &std::path::Path,
) -> Vec<String> {
    if !paths.is_empty() {
        return paths.to_vec();
    }
    if config.include.is_empty() {
        return vec![".".to_owned()];
    }
    config
        .include
        .iter()
        .map(|inc| config_root.join(inc).to_string_lossy().into_owned())
        .collect()
}

fn collect_and_check(
    paths: &[String],
    cache: &cache_check::CacheOptions,
    stats: &mut cache_check::CacheStats,
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
    let mut config = basilisk_config::load_basilisk_config(&config_root);
    // [CHKARCH-VERSION-TARGET] Detect the target version from project files
    // when the config does not pin one, matching the LSP (issue #93).
    if config.python_version.is_none() {
        config.python_version =
            basilisk_uv::python_version::resolve_target_python_version(&config_root);
    }

    let excluded = excluded_dirs_and_log(&config, &config_root);

    // Implements [CHKARCH-CONFIG-INCLUDE] (issue #37): a no-args run walks
    // only the configured include roots, never the whole repository.
    let paths = &effective_check_paths(paths, &config, &config_root);
    let python_files = collect_python_files(paths, &excluded)?;

    // Build import search paths (venv, uv registry, workspace members).
    // Use cwd as the project root — pyproject.toml, uv.lock, and .venv
    // live at the project root, not necessarily in the checked path.
    let project_root = find_project_root(&config_root);
    let canonical_root = std::fs::canonicalize(&project_root).unwrap_or(project_root.clone());
    let mut roots = vec![canonical_root.clone()];

    // Add checked directories as search roots for sibling imports.
    for path_str in paths {
        let p = std::path::Path::new(path_str);
        let dir = if p.is_dir() {
            p
        } else {
            p.parent().unwrap_or(p)
        };
        if let Ok(abs) = std::fs::canonicalize(dir) {
            if !roots.contains(&abs) {
                roots.push(abs);
            }
        }
    }

    // Add include paths from WorkspaceConfig as search roots.
    let lsp_config = basilisk_lsp::config::load_config(&project_root);
    let registry = build_uv_registry(&roots);
    let mut search_paths =
        basilisk_lsp::import_resolver::search_paths_from_config(&roots, &lsp_config, registry);
    // Ensure all roots are also in extra_paths for module resolution.
    search_paths.roots = roots;
    info!(
        site_packages = ?search_paths.site_packages,
        has_registry = search_paths.registry.is_some(),
        "built import search paths"
    );

    let cache_context = cache_check::build_context(cache, &config, &search_paths, &project_root);

    let mut all_diagnostics = Vec::new();
    let mut sources = Vec::new();

    for path in python_files {
        let outcome = cache_check::check_file(cache_context.as_ref(), stats, &path, || {
            process_file(&path, &search_paths, &config)
        });
        match outcome {
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

/// Build a uv package registry from workspace roots, if this is a uv project.
fn build_uv_registry(
    roots: &[std::path::PathBuf],
) -> Option<std::sync::Arc<basilisk_uv::PackageRegistry>> {
    let uv_info = basilisk_uv::detect_uv_project(roots)?;

    if !uv_info.has_lockfile {
        info!(
            root = %uv_info.root.display(),
            "uv project detected but no uv.lock — skipping registry"
        );
        return None;
    }

    let lock_path = uv_info.root.join("uv.lock");
    let lock_file = match basilisk_uv::parse_lock_file(&lock_path) {
        Ok(lock) => lock,
        Err(err) => {
            warn!(
                path = %lock_path.display(),
                %err,
                "failed to parse uv.lock — package registry unavailable"
            );
            return None;
        }
    };

    let deps = basilisk_uv::extract_pyproject_deps(&uv_info.root);
    let registry = basilisk_uv::PackageRegistry::from_lock_file(&lock_file, &deps);

    let pkg_count = registry.all_packages().count();
    info!(
        root = %uv_info.root.display(),
        packages = pkg_count,
        direct_deps = deps.len(),
        "built uv package registry"
    );

    Some(std::sync::Arc::new(registry))
}

fn process_file(
    path: &str,
    search_paths: &basilisk_lsp::import_resolver::ImportSearchPaths,
    config: &basilisk_config::BasiliskConfig,
) -> Result<(Vec<basilisk_checker::Diagnostic>, String), String> {
    let parsed = basilisk_parser::parse_file(path).map_err(|e| e.to_string())?;
    let source = parsed.source.clone();
    let mut resolved = basilisk_resolver::resolve(&parsed).map_err(|e| e.to_string())?;

    // Resolve imports against venv/site-packages and uv registry using the same
    // routine the LSP uses, so the CLI and editor agree on what resolves and on
    // package-dependency metadata (BSK-W0011 transitive-import warnings, etc.).
    basilisk_lsp::import_resolver::resolve_module_imports(&mut resolved, search_paths);

    // Apply the project's `[tool.basilisk.rules]` / per-path overrides so the
    // CLI and editor agree on severity (e.g. a project can promote "no type
    // stubs" to a hard error). Using `check` here would silently drop config.
    let diags = basilisk_checker::check_with_config(&resolved, config);
    Ok((diags, source))
}

/// Walk up from `start` to find the project root (directory containing
/// `pyproject.toml` or `uv.lock`). Falls back to cwd, then `start`.
fn find_project_root(start: &std::path::Path) -> std::path::PathBuf {
    let abs = std::fs::canonicalize(start).unwrap_or_else(|_| start.to_path_buf());
    let mut current = abs.as_path();
    loop {
        if current.join("pyproject.toml").is_file() || current.join("uv.lock").is_file() {
            return current.to_path_buf();
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }
    // Fallback: cwd, then the original start path.
    std::env::current_dir().unwrap_or_else(|_| start.to_path_buf())
}

/// Return `"s"` for counts != 1, empty string otherwise.
pub(crate) fn pluralise(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}

/// Whether `path` is excluded by any configured pattern, matched
/// gitignore-style against the path relative to the walk `root`.
///
/// Implements [CHKARCH-CONFIG-EXCLUDE]. See
/// docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-EXCLUDE
///
/// Uses the same [`basilisk_config::path_matches_pattern`] matcher as the LSP
/// workspace scan, so `basilisk check` and the editor agree on what is skipped:
/// bare names (`build`) at any depth, directory globs (`**/generated/**`),
/// and file globs (`*.pb.py`) all work — not just literal directory names.
fn is_excluded_path(
    path: &std::path::Path,
    root: &std::path::Path,
    excluded: &HashSet<&str>,
) -> bool {
    let relative = path.strip_prefix(root).unwrap_or(path);
    excluded
        .iter()
        .any(|pattern| basilisk_config::path_matches_pattern(relative, pattern))
}

/// Build the excluded-directory set from `config` and log that the config at
/// `config_root` was loaded. Shared setup prologue for the CLI subcommands.
pub(crate) fn excluded_dirs_and_log<'a>(
    config: &'a basilisk_config::BasiliskConfig,
    config_root: &std::path::Path,
) -> HashSet<&'a str> {
    let excluded: HashSet<&str> = config.exclude.iter().map(String::as_str).collect();
    info!(
        excluded_dirs = ?config.exclude,
        "loaded config from {}",
        config_root.display()
    );
    excluded
}

/// `true` for the Python source extensions Basilisk type-checks: `.py`
/// implementation files and `.pyi` stub files (whose overload-definition and
/// `@final`/`@override` rules differ — see `overloads_*`). Stubs were silently
/// dropped before, so a `basilisk check foo.pyi` produced no diagnostics.
fn is_python_source_ext(ext: &std::ffi::OsStr) -> bool {
    ext.eq_ignore_ascii_case("py") || ext.eq_ignore_ascii_case("pyi")
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
                .is_some_and(is_python_source_ext)
            {
                files.push(root.clone());
            }
        } else {
            let root_path = std::path::Path::new(root);
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
                    !is_excluded_path(e.path(), root_path, excluded)
                })
                .filter_map(Result::ok)
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().extension().is_some_and(is_python_source_ext))
                // File-level globs (e.g. `*.pb.py`, `**/conftest.py`) are honoured
                // here; directory globs are already pruned above before recursing.
                .filter(|e| !is_excluded_path(e.path(), root_path, excluded))
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

    /// Disabled cache options for tests that exercise the plain check pipeline.
    fn no_cache() -> cache_check::CacheOptions {
        cache_check::CacheOptions {
            enabled: false,
            dir: None,
            stats: false,
        }
    }

    /// Run `collect_and_check` with the cache disabled and a throwaway tally.
    fn collect_and_check_uncached(
        paths: &[String],
    ) -> Result<(Vec<basilisk_checker::Diagnostic>, Vec<FileSource>), String> {
        collect_and_check(paths, &no_cache(), &mut cache_check::CacheStats::default())
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
        let result = collect_and_check_uncached(&[path]);
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
        // `def foo(x)` violates the annotation house rules (BSK-E0001/E0002),
        // which are OFF by default — the default config is pure PEP conformance.
        // Run inside an isolated project that opts in, exactly as a user would.
        // See [CHKARCH-CONFIGURATION-ONLY].
        let dir = unique_project_dir("basilisk_test_bad_code");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(
            dir.join("basilisk.json"),
            b"{\"strictAnnotations\": true}\n",
        )?;
        let py = dir.join("bad.py");
        std::fs::write(&py, b"def foo(x):\n    pass\n")?;
        let path = py.to_string_lossy().into_owned();
        let (diags, _) = collect_and_check_uncached(&[path])?;
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            !diags.is_empty(),
            "unannotated function must produce diagnostics"
        );
        Ok(())
    }

    // ── collect_and_check: project config severity overrides are applied ──────

    /// Unique temp dir for tests that need an isolated project root.
    fn unique_project_dir(prefix: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static CTR: AtomicU64 = AtomicU64::new(0);
        let n = CTR.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("{prefix}_{}_{n}", std::process::id()))
    }

    /// Regression: the `basilisk check` CLI must honor `[tool.basilisk.rules]`
    /// severity overrides from `pyproject.toml`. A project that escalates a
    /// warning-default rule (BSK-W0050) to "error" must see it surface as a
    /// hard error through the real CLI pipeline — otherwise strictness config
    /// is silently ignored in CI. Previously `process_file` called
    /// `basilisk_checker::check` (default config), dropping all overrides.
    #[test]
    fn collect_and_check_applies_project_rule_severity_override(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_project_dir("basilisk_cli_cfg_promote");
        std::fs::create_dir_all(&dir)?;
        // A severity override only re-grades a rule that is ALREADY enabled;
        // BSK-W0050 is an off-by-default house rule, so the project must opt in
        // (`strict-annotations = true`) AND escalate it. See
        // [CHKARCH-CONFIGURATION-ONLY].
        std::fs::write(
            dir.join("pyproject.toml"),
            b"[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n\
              [tool.basilisk]\nstrict-annotations = true\n\n\
              [tool.basilisk.rules]\n\"BSK-W0050\" = \"error\"\n",
        )?;
        let py = dir.join("m.py");
        std::fs::write(&py, b"x: int = 42\n")?;

        let path = py.to_string_lossy().into_owned();
        let (diags, _) = collect_and_check_uncached(&[path])?;
        let _ = std::fs::remove_dir_all(&dir);

        let w0050: Vec<_> = diags
            .iter()
            .filter(|d| d.code.code == "BSK-W0050")
            .collect();
        assert!(!w0050.is_empty(), "BSK-W0050 must still fire");
        assert!(
            w0050
                .iter()
                .all(|d| d.severity == basilisk_checker::Severity::Error),
            "project config `BSK-W0050 = \"error\"` must promote BSK-W0050 to error \
             through the CLI; got {:?}",
            w0050.iter().map(|d| d.severity).collect::<Vec<_>>()
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
        let (diags, _) = collect_and_check_uncached(&[path])?;
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
        // BSK-E0001 (unannotated `x`) is off by default; opt in via project config.
        let dir = unique_project_dir("basilisk_test_rc_json_bad");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(
            dir.join("basilisk.json"),
            b"{\"strictAnnotations\": true}\n",
        )?;
        let py = dir.join("bad.py");
        std::fs::write(&py, b"def foo(x) -> None:\n    pass\n")?;
        let path = py.to_string_lossy().into_owned();
        let code = run_check(&[path], OutputFormat::Json, &no_cache());
        let _ = std::fs::remove_dir_all(&dir);
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
        let code = run_check(&[path], OutputFormat::Json, &no_cache());
        let _ = std::fs::remove_file(&py);
        assert_eq!(code, 0, "clean code must make run_check return 0 (Json)");
        Ok(())
    }

    /// `run_check` Text path: bad code must return 1.
    /// Kills `>=` mutant at line 81 (which always returns 1 since usize >= 0).
    #[test]
    fn run_check_text_bad_code_returns_one() -> Result<(), Box<dyn std::error::Error>> {
        // BSK-E0001 (unannotated `x`) is off by default; opt in via project config.
        let dir = unique_project_dir("basilisk_test_rc_text_bad");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(
            dir.join("basilisk.json"),
            b"{\"strictAnnotations\": true}\n",
        )?;
        let py = dir.join("bad.py");
        std::fs::write(&py, b"def foo(x) -> None:\n    pass\n")?;
        let path = py.to_string_lossy().into_owned();
        let code = run_check(&[path], OutputFormat::Text, &no_cache());
        let _ = std::fs::remove_dir_all(&dir);
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
        let code = run_check(&[path], OutputFormat::Text, &no_cache());
        let _ = std::fs::remove_file(&py);
        assert_eq!(code, 0, "clean code must make run_check return 0 (Text)");
        Ok(())
    }

    /// `run_check` internal error path: nonexistent path must return 3.
    #[test]
    fn run_check_nonexistent_path_returns_three() {
        let code = run_check(
            &["/no/such/path.py".to_owned()],
            OutputFormat::Text,
            &no_cache(),
        );
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
        let code = run_check(&[path], OutputFormat::Text, &no_cache());
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
        let code = run_check(&[path], OutputFormat::Json, &no_cache());
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

    /// Regression: the `basilisk check` CLI ignored user **glob** excludes.
    /// `collect_python_files` matched only bare directory names against the
    /// exclude set (and never applied any exclude to individual files), so a
    /// project configuring `exclude = ["**/generated/**", "*.pb.py"]` still had
    /// its generated tree and `*.pb.py` files type-checked — diverging from the
    /// LSP workspace scan, which honours the same gitignore-style globs via
    /// `basilisk_config::path_matches_pattern`. The CLI must agree with the LSP.
    #[test]
    fn collect_python_files_honors_user_glob_excludes() -> Result<(), Box<dyn std::error::Error>> {
        let base = std::env::temp_dir().join(format!(
            "basilisk_test_cli_glob_exclude_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let gen = base.join("src").join("generated");
        std::fs::create_dir_all(&gen)?;
        std::fs::write(base.join("app.py"), b"x = 1")?; // real code — must survive
        std::fs::write(gen.join("models.py"), b"y = 2")?; // excluded by **/generated/**
        std::fs::write(base.join("schema.pb.py"), b"z = 3")?; // excluded by *.pb.py

        let excludes: HashSet<&str> = ["**/generated/**", "*.pb.py"].into_iter().collect();
        let path = base.to_string_lossy().into_owned();
        let files = collect_python_files(&[path], &excludes)?;
        let _ = std::fs::remove_dir_all(&base);

        let names: Vec<String> = files.iter().map(|f| f.replace('\\', "/")).collect();
        assert_eq!(
            files.len(),
            1,
            "only app.py should survive the glob excludes, got: {names:?}"
        );
        assert!(
            names.iter().any(|f| f.ends_with("/app.py")),
            "app.py must still be collected: {names:?}"
        );
        assert!(
            !names.iter().any(|f| f.contains("generated")),
            "**/generated/** must exclude the nested directory: {names:?}"
        );
        assert!(
            !names.iter().any(|f| f.contains("schema.pb.py")),
            "*.pb.py glob must exclude the file: {names:?}"
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
