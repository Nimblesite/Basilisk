//! Stub-management CLI implementation for [STUBRES-AUTOGEN].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use basilisk_resolver::{ImportInfo, ImportResolution};
use basilisk_stubs::generate::{self, GeneratedStub, StubGenError, StubGenMode};
use clap::{Args, Subcommand};
use colored::Colorize as _;
use tracing::info;

/// Stub management subcommands.
#[derive(Subcommand)]
pub(super) enum StubAction {
    /// Generate best-effort `.pyi` stubs for untyped packages.
    Generate {
        /// Package names to generate stubs for.
        packages: Vec<String>,
        /// Generate stubs for every untyped import in the project.
        #[arg(long, conflicts_with = "packages")]
        all: bool,
        /// Generation mode: runtime, ast, or hybrid (default).
        #[arg(long, default_value = "hybrid")]
        mode: StubGenModeArg,
        /// Path to the Python interpreter.
        #[arg(long, default_value = "python3")]
        python: String,
    },
    /// Show stub coverage status for the project.
    Status,
}

/// Arguments accepted by the Pyright-compatible `--createstub` alias.
#[derive(Args)]
pub(super) struct CreateStubArgs {
    /// Package name to generate a stub for.
    package: String,
    /// Generation mode: runtime, ast, or hybrid (default).
    #[arg(long, default_value = "hybrid")]
    mode: StubGenModeArg,
    /// Path to the Python interpreter.
    #[arg(long, default_value = "python3")]
    python: String,
}

/// CLI-friendly stub generation mode.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub(super) enum StubGenModeArg {
    /// Generate through runtime introspection.
    Runtime,
    /// Generate by parsing package source.
    Ast,
    /// Prefer runtime introspection and fall back to source parsing.
    Hybrid,
}

impl From<StubGenModeArg> for StubGenMode {
    fn from(mode: StubGenModeArg) -> Self {
        match mode {
            StubGenModeArg::Runtime => Self::Runtime,
            StubGenModeArg::Ast => Self::Ast,
            StubGenModeArg::Hybrid => Self::Hybrid,
        }
    }
}

struct GenerationTarget {
    module: String,
    source_path: Option<PathBuf>,
}

/// Run a nested `stubs` command.
pub(super) fn run(action: StubAction) -> u8 {
    match action {
        StubAction::Generate {
            packages,
            all,
            mode,
            python,
        } => run_generate(&packages, all, mode, &python),
        StubAction::Status => run_status(),
    }
}

/// Map Pyright's top-level spelling to the named-package generation workflow.
// Implements [STUBRES-AUTOGEN]: `basilisk --createstub X` and
// `basilisk stubs generate X` share one backend and output contract.
pub(super) fn run_create_stub(args: CreateStubArgs) -> u8 {
    run_generate(&[args.package], false, args.mode, &args.python)
}

fn run_generate(packages: &[String], all: bool, mode: StubGenModeArg, python: &str) -> u8 {
    let project_root = crate::find_project_root(Path::new("."));
    let python_path = Path::new(python);
    let targets = match generation_targets(packages, all, python_path, &project_root) {
        Ok(targets) => targets,
        Err(message) => {
            eprintln!("{}: {message}", "error".red());
            return 1;
        }
    };
    if targets.is_empty() {
        println!("No untyped imports found");
        return 0;
    }
    let cache_dir = project_root.join(generate::cache::DEFAULT_CACHE_DIR);
    let failed = targets.iter().fold(false, |had_failure, target| {
        !generate_target(target, mode.into(), python_path, &cache_dir) || had_failure
    });
    u8::from(failed)
}

fn generation_targets(
    packages: &[String],
    all: bool,
    python_path: &Path,
    project_root: &Path,
) -> Result<Vec<GenerationTarget>, String> {
    if all {
        return discover_untyped_imports(project_root);
    }
    if packages.is_empty() {
        return Err("specify package names or use --all".to_owned());
    }
    Ok(packages
        .iter()
        .map(|module| GenerationTarget {
            module: module.clone(),
            source_path: find_package_source(module, python_path),
        })
        .collect())
}

// Implements [STUBRES-AUTOGEN]: scan the configured project inputs with the
// same parser, resolver, exclusions, and import search paths as `check`, then
// generate only imports that resolve to untyped site-packages source.
fn discover_untyped_imports(project_root: &Path) -> Result<Vec<GenerationTarget>, String> {
    let config = basilisk_config::load_basilisk_config(project_root);
    let excluded = crate::excluded_dirs_and_log(&config, project_root);
    let paths = crate::effective_check_paths(&[], &config, project_root);
    let files = crate::collect_python_files(&paths, &excluded)?;
    let roots = crate::analysis_roots(&paths, project_root);
    let search_paths = crate::build_import_search_paths(roots, project_root);
    let Some(site_packages) = search_paths.site_packages.as_deref() else {
        return Ok(Vec::new());
    };
    let mut targets = BTreeMap::new();
    for file in files {
        collect_file_targets(&file, &search_paths, site_packages, &mut targets)?;
    }
    Ok(targets
        .into_iter()
        .map(|(module, source_path)| GenerationTarget {
            module,
            source_path: Some(source_path),
        })
        .collect())
}

fn collect_file_targets(
    file: &str,
    search_paths: &basilisk_lsp::import_resolver::ImportSearchPaths,
    site_packages: &Path,
    targets: &mut BTreeMap<String, PathBuf>,
) -> Result<(), String> {
    let parsed = basilisk_parser::parse_file(file).map_err(|error| error.to_string())?;
    let mut resolved = basilisk_resolver::resolve(&parsed).map_err(|error| error.to_string())?;
    basilisk_lsp::import_resolver::resolve_module_imports(&mut resolved, search_paths);
    resolved
        .imports
        .iter()
        .filter(|import| is_untyped_third_party_import(import, site_packages))
        .filter_map(|import| {
            import
                .resolved_path
                .as_ref()
                .map(|path| (import.module.clone(), path.clone()))
        })
        .for_each(|(module, path)| {
            let _ = targets.entry(module).or_insert(path);
        });
    Ok(())
}

fn is_untyped_third_party_import(import: &ImportInfo, site_packages: &Path) -> bool {
    import.resolution == ImportResolution::SourcePy
        && !basilisk_stubs::is_stdlib_module(&import.module)
        && import.resolved_path.as_ref().is_some_and(|path| {
            path.starts_with(site_packages) && !basilisk_stubs::has_py_typed_marker(path)
        })
}

fn generate_target(
    target: &GenerationTarget,
    mode: StubGenMode,
    python_path: &Path,
    cache_dir: &Path,
) -> bool {
    let result = match target.source_path.as_deref() {
        Some(source) => {
            info!(module = target.module, source = %source.display(), "generating stubs");
            generate::generate_stubs(&target.module, source, python_path, mode)
        }
        None if mode == StubGenMode::Ast => {
            eprintln!(
                "{} Cannot find source for `{}` — AST mode requires source files",
                "✗".red(),
                target.module
            );
            return false;
        }
        None => generate::runtime::generate_runtime_stubs(&target.module, python_path),
    };
    cache_generation_result(cache_dir, &target.module, result)
}

fn cache_generation_result(
    cache_dir: &Path,
    package: &str,
    result: Result<GeneratedStub, StubGenError>,
) -> bool {
    match result {
        Ok(stub) => cache_stub(cache_dir, package, &stub),
        Err(error) => {
            eprintln!(
                "{} Failed to generate stub for `{package}`: {error}",
                "✗".red()
            );
            false
        }
    }
}

/// Cache a generated stub and print the result.
pub(super) fn cache_stub(cache_dir: &Path, package: &str, stub: &GeneratedStub) -> bool {
    let source_hash = generate::cache::hash_source(&stub.pyi_content);
    match generate::cache::write_cache(cache_dir, package, &stub.pyi_content, source_hash) {
        Ok(path) => {
            println!(
                "{} Generated stub for `{package}` → {}",
                "✓".green(),
                path.display()
            );
            true
        }
        Err(error) => {
            eprintln!(
                "{} Failed to write stub for `{package}`: {error}",
                "✗".red()
            );
            false
        }
    }
}

/// Import a module named by `sys.argv[1]` and print its source path.
const FIND_PACKAGE_SOURCE_SCRIPT: &str = r#"
import importlib
import sys

module = importlib.import_module(sys.argv[1])
source = getattr(module, "__file__", None)
if source is None:
    raise SystemExit(1)
print(source)
"#;

fn is_valid_module_name(name: &str) -> bool {
    name.split('.').all(|component| {
        let mut chars = component.chars();
        chars
            .next()
            .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
            && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
    })
}

/// Find the source path for an installed package by querying Python.
pub(super) fn find_package_source(package: &str, python_path: &Path) -> Option<PathBuf> {
    if !is_valid_module_name(package) {
        return None;
    }
    let output = std::process::Command::new(python_path)
        .args(["-c", FIND_PACKAGE_SOURCE_SCRIPT, package])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let source = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    source.is_file().then_some(source).filter(|path| {
        path.extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("py"))
    })
}

fn run_status() -> u8 {
    let project_root = crate::find_project_root(Path::new("."));
    let cache_dir = project_root.join(generate::cache::DEFAULT_CACHE_DIR);
    if !cache_dir.exists() {
        println!("No generated stubs found ({})", cache_dir.display());
        return 0;
    }
    let modules: Vec<String> = walkdir::WalkDir::new(&cache_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| stub_module_name(entry.path(), &cache_dir))
        .collect();
    for module in &modules {
        println!("  {} {module}", "✓".green());
    }
    if modules.is_empty() {
        println!("No generated stubs found");
    } else {
        println!(
            "\n{} generated stub(s) in {}",
            modules.len(),
            cache_dir.display()
        );
    }
    0
}

fn stub_module_name(path: &Path, cache_dir: &Path) -> Option<String> {
    (path.extension()? == "pyi").then(|| {
        path.strip_prefix(cache_dir)
            .unwrap_or(path)
            .with_extension("")
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join(".")
    })
}
