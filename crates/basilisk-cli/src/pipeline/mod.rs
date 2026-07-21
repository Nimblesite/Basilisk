//! Implements [CHKARCH-CLI] and [CHKARCH-COMMANDS]. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI
//!
//! The shared collect-and-check pipeline behind `basilisk check` and
//! `basilisk analyze`. Both commands run the identical pipeline — file
//! collection, per-directory config discovery, import resolution, caching,
//! `check_with_config` — and differ only in the [`DiagnosticScope`] edge
//! filter applied to the resulting diagnostics ([CHKARCH-COMMANDS]).

use std::collections::HashSet;

use tracing::{info, warn};

use crate::cache_check;
use crate::output::FileSource;

mod typeshed;

pub(crate) use typeshed::build_import_search_paths;
use typeshed::{
    activate_production_typeshed, build_import_search_paths_with_config, load_cli_workspace_config,
};

/// Which command's diagnostics to keep at the CLI edge.
///
/// Implements [CHKARCH-COMMANDS]: one rule universe, partitioned exactly once
/// by provenance tag. A rule is check-scope iff it carries the `pep` tag;
/// everything else is analyze-scope. The checker runs all selected rules; the
/// CLI edge filters by command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiagnosticScope {
    /// `basilisk check` — `pep`-tagged rules only, always on.
    Check,
    /// `basilisk analyze` — every rule *not* tagged `pep`, config-selected.
    Analyze,
    /// Both scopes — `adopt` reads the union at resolved severities
    /// ([AUTOFIX-ADOPTION]).
    Union,
}

impl DiagnosticScope {
    /// Whether a diagnostic with `code` belongs to this scope.
    pub(crate) fn retains(self, code: &str) -> bool {
        match self {
            Self::Check => basilisk_checker::is_pep_rule(code),
            Self::Analyze => !basilisk_checker::is_pep_rule(code),
            Self::Union => true,
        }
    }
}

/// A pipeline failure, mapped to the [CHKARCH-CLI-EXITCODES] contract.
#[derive(Debug)]
pub(crate) enum PipelineError {
    /// Invalid configuration (exit code `2`) — e.g. a config that resolves a
    /// `pep` rule to `disabled` ([CHKARCH-CONFIG-MODEL]).
    Config(String),
    /// Internal failure (exit code `3`).
    Internal(String),
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(message) => write!(f, "invalid configuration: {message}"),
            Self::Internal(message) => write!(f, "{message}"),
        }
    }
}

#[derive(Debug)]
pub(crate) struct FileAnalysisFailure {
    pub(crate) path: String,
    pub(crate) message: String,
}

pub(crate) struct CheckOutcome {
    pub(crate) diagnostics: Vec<basilisk_checker::Diagnostic>,
    pub(crate) sources: Vec<FileSource>,
    pub(crate) failures: Vec<FileAnalysisFailure>,
    /// How many rules configuration selected that this scope never evaluated
    /// — always `0` outside [`DiagnosticScope::Check`]
    /// ([CHKARCH-CLI-SCOPE-NOTICE]).
    pub(crate) unrun_selected_rules: usize,
}

/// Resolve the paths a check run walks. Implements [CHKARCH-CONFIG-INCLUDE]:
/// explicit CLI paths win, then the configured `include` roots, then `.`.
pub(crate) fn effective_check_paths(
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

/// The per-directory rule configuration of a run, keyed by owning directory
/// ([CHKARCH-CONFIG-DISCOVERY]).
pub(crate) type DirConfigs =
    std::collections::BTreeMap<std::path::PathBuf, std::sync::Arc<basilisk_config::BasiliskConfig>>;

/// The union of the codes `select` reports across the base config and every
/// per-directory config in this run — one file's config never speaks for the
/// whole run ([CHKARCH-CONFIG-DISCOVERY]).
fn codes_across_configs(
    dir_configs: &DirConfigs,
    base: &basilisk_config::BasiliskConfig,
    select: fn(&basilisk_config::BasiliskConfig) -> Vec<&'static str>,
) -> std::collections::BTreeSet<&'static str> {
    let mut codes: std::collections::BTreeSet<&'static str> = select(base).into_iter().collect();
    for config in dir_configs.values() {
        codes.extend(select(config));
    }
    codes
}

/// The codes an invalid configuration resolves to `disabled` although they
/// are `pep`-tagged, across every per-directory config in this run.
///
/// Implements [CHKARCH-CONFIG-MODEL]: `disabled` never applies to a `pep`
/// rule — such a configuration is invalid and fails the run before checking.
fn pep_disable_config_error(
    dir_configs: &DirConfigs,
    base: &basilisk_config::BasiliskConfig,
) -> Option<String> {
    let violations =
        codes_across_configs(dir_configs, base, basilisk_checker::pep_disable_violations);
    if violations.is_empty() {
        return None;
    }
    let codes = violations.into_iter().collect::<Vec<_>>().join(", ");
    Some(format!(
        "configuration resolves PEP typing-spec rules to `disabled`, which is invalid \
         ([CHKARCH-CONFIG-MODEL]): {codes}. PEP rules always run; grade them \
         `error`/`warning`/`info` instead."
    ))
}

/// Collect Python files and check each one under its own discovered config,
/// keeping only diagnostics in `scope` ([CHKARCH-COMMANDS]).
///
/// # Errors
///
/// [`PipelineError::Config`] when any discovered configuration invalidly
/// resolves a `pep` rule to `disabled`; [`PipelineError::Internal`] on
/// collection failures (e.g. nonexistent paths).
pub(crate) fn collect_and_check(
    paths: &[String],
    cache: &cache_check::CacheOptions,
    stats: &mut cache_check::CacheStats,
    scope: DiagnosticScope,
) -> Result<CheckOutcome, PipelineError> {
    collect_and_check_with_overrides(paths, cache, stats, scope, TypeshedOverrides::default())
}

/// One-run command-line overrides for Typeshed acquisition policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TypeshedOverrides {
    pub(crate) no_cache: bool,
    pub(crate) no_verification: bool,
}

pub(crate) fn collect_and_check_with_overrides(
    paths: &[String],
    cache: &cache_check::CacheOptions,
    stats: &mut cache_check::CacheStats,
    scope: DiagnosticScope,
    overrides: TypeshedOverrides,
) -> Result<CheckOutcome, PipelineError> {
    collect_and_check_with_typeshed(
        paths,
        cache,
        stats,
        scope,
        overrides,
        activate_production_typeshed,
    )
}

fn collect_and_check_with_typeshed<F>(
    paths: &[String],
    cache: &cache_check::CacheOptions,
    stats: &mut cache_check::CacheStats,
    scope: DiagnosticScope,
    overrides: TypeshedOverrides,
    activate_typeshed: F,
) -> Result<CheckOutcome, PipelineError>
where
    F: Fn(
        &mut basilisk_lsp::import_resolver::ImportSearchPaths,
        &basilisk_lsp::config::WorkspaceConfig,
    ) -> Result<(), PipelineError>,
{
    // [CHKARCH-CONFIG-DISCOVERY] The first path only anchors project-level
    // concerns (include expansion, version detection, cache location); rule
    // config is resolved per checked file below, so diagnostics never depend
    // on argument order (GitHub #311).
    let config_root = first_path_dir(paths);
    // Project metadata can live above the checked path (for example,
    // `conformance/tests/case.py` inherits `conformance/pyproject.toml`).
    // Resolve the project root before reading target-version evidence so a
    // nested invocation observes the same explicit project target as the LSP.
    let project_root = find_project_root(&config_root);
    let mut config = basilisk_config::load_basilisk_config(&config_root);
    // [CHKARCH-VERSION-TARGET] Detect the target version from project files
    // when the config does not pin one, matching the LSP (issue #93).
    if config.python_version.is_none() {
        config.python_version =
            basilisk_uv::python_version::resolve_target_python_version(&project_root);
    }

    let mut workspace_config =
        load_cli_workspace_config(&project_root, config.python_version.as_deref());
    if config.python_platform.is_none() {
        config
            .python_platform
            .clone_from(&workspace_config.python_platform);
    }

    let excluded = excluded_dirs_and_log(&config, &config_root);

    // Implements [CHKARCH-CONFIG-INCLUDE] (issue #37): a no-args run walks
    // only the configured include roots, never the whole repository.
    let paths = &effective_check_paths(paths, &config, &config_root);
    let python_files = collect_python_files(paths, &excluded).map_err(PipelineError::Internal)?;

    // Build import search paths (venv, uv registry, workspace members).
    // pyproject.toml, uv.lock, and .venv live at the discovered project root,
    // not necessarily in the checked path.
    let roots = analysis_roots(paths, &project_root);
    if overrides.no_cache {
        workspace_config.typeshed_cache = false;
    }
    if overrides.no_verification {
        workspace_config.typeshed_verify = false;
    }
    let mut search_paths = if crate::import_search::files_might_import(&python_files) {
        build_import_search_paths_with_config(roots, &workspace_config)
    } else {
        crate::import_search::roots_only(roots)
    };
    activate_typeshed(&mut search_paths, &workspace_config)?;

    // Per-file rule config, memoized per directory ([CHKARCH-CONFIG-DISCOVERY]).
    // The cache fingerprint covers every directory's config so a child config
    // edit invalidates cached results.
    let dir_configs = resolve_dir_configs(&python_files, &config);

    // A config that disables a PEP rule is invalid and fails the run before
    // any checking ([CHKARCH-CONFIG-MODEL], [CHKARCH-CLI-EXITCODES] code 2).
    if let Some(message) = pep_disable_config_error(&dir_configs, &config) {
        return Err(PipelineError::Config(message));
    }

    // [CHKARCH-CLI-SCOPE-NOTICE] (GitHub #334): the edge filter below drops
    // every analyze-scope diagnostic from a `check` run. Count the rules
    // configuration selected but this scope will never evaluate, so the
    // renderer can say so — a silent clean run is indistinguishable from a
    // clean project.
    let unrun_selected_rules = match scope {
        DiagnosticScope::Check => codes_across_configs(
            &dir_configs,
            &config,
            basilisk_checker::analyze_selected_rules,
        )
        .len(),
        DiagnosticScope::Analyze | DiagnosticScope::Union => 0,
    };

    let cache_context =
        cache_check::build_context(cache, &dir_configs, &search_paths, &project_root);

    let mut all_diagnostics = Vec::new();
    let mut sources = Vec::new();
    let mut failures = Vec::new();

    for path in python_files {
        let file_config = config_for_path(&dir_configs, &path, &config);
        let outcome = cache_check::check_file(cache_context.as_ref(), stats, &path, || {
            process_file(&path, &search_paths, &file_config)
        });
        match outcome {
            Ok((diags, source)) => {
                // The command's edge filter ([CHKARCH-COMMANDS]). Applied
                // after the cache layer so cached entries stay scope-free
                // and both commands share them.
                all_diagnostics.extend(diags.into_iter().filter(|d| scope.retains(d.code.code)));
                sources.push(FileSource { path, text: source });
            }
            Err(err) => {
                failures.push(FileAnalysisFailure { path, message: err });
            }
        }
    }

    Ok(CheckOutcome {
        diagnostics: all_diagnostics,
        sources,
        failures,
        unrun_selected_rules,
    })
}

/// Canonical project and checked-directory roots used for import resolution.
pub(crate) fn analysis_roots(
    paths: &[String],
    project_root: &std::path::Path,
) -> Vec<std::path::PathBuf> {
    let canonical = std::fs::canonicalize(project_root).unwrap_or_else(|_| project_root.into());
    paths.iter().fold(vec![canonical], |mut roots, path| {
        let candidate = std::path::Path::new(path);
        let directory = if candidate.is_dir() {
            candidate.to_path_buf()
        } else {
            parent_dir_of(path)
        };
        if let Ok(absolute) = std::fs::canonicalize(directory) {
            if !roots.contains(&absolute) {
                roots.push(absolute);
            }
        }
        roots
    })
}

fn process_file(
    path: &str,
    search_paths: &basilisk_lsp::import_resolver::ImportSearchPaths,
    config: &basilisk_config::BasiliskConfig,
) -> Result<(Vec<basilisk_checker::Diagnostic>, String), String> {
    let target_version =
        basilisk_checker::context::CheckContext::from_config(config).target_version;
    let (resolved, source) = resolve_file_imports(path, search_paths, target_version)?;
    // Apply the project's `[tool.basilisk]` tables so the CLI and editor
    // agree on selection and severity ([CHKARCH-CONFIG-MODEL]). Using `check`
    // here would silently drop config.
    let diagnostics = basilisk_checker::check_with_config(&resolved, config);
    Ok((diagnostics, source))
}

/// Parse a source file and resolve its imports through the shared CLI/LSP paths.
pub(crate) fn resolve_file_imports(
    path: &str,
    search_paths: &basilisk_lsp::import_resolver::ImportSearchPaths,
    target_version: Option<(u32, u32)>,
) -> Result<(basilisk_resolver::ResolvedModule, String), String> {
    let parsed = basilisk_parser::parse_file(path).map_err(|e| e.to_string())?;
    let source = parsed.source.clone();
    let mut resolved = match target_version {
        Some(target_version) => basilisk_resolver::resolve_with_target(&parsed, target_version),
        None => basilisk_resolver::resolve(&parsed),
    }
    .map_err(|e| e.to_string())?;

    // Resolve imports against venv/site-packages and uv registry using the same
    // routine the LSP uses, so the CLI and editor agree on what resolves and on
    // package-dependency metadata (BSK-0011 transitive-import warnings, etc.).
    basilisk_lsp::import_resolver::resolve_module_imports(&mut resolved, search_paths);
    Ok((resolved, source))
}

/// The directory anchoring project-level concerns for a CLI invocation: the
/// first path argument's own directory (or its parent for a file), else cwd.
pub(crate) fn first_path_dir(paths: &[String]) -> std::path::PathBuf {
    paths.first().map(std::path::Path::new).map_or_else(
        || std::path::PathBuf::from("."),
        |p| {
            if p.is_dir() {
                p.to_path_buf()
            } else {
                p.parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                    .unwrap_or_else(|| std::path::Path::new("."))
                    .to_path_buf()
            }
        },
    )
}

/// The directory owning `path` (its parent, or `.` for a bare filename).
pub(crate) fn parent_dir_of(path: &str) -> std::path::PathBuf {
    std::path::Path::new(path)
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .map_or_else(
            || std::path::PathBuf::from("."),
            std::path::Path::to_path_buf,
        )
}

/// Resolve the merged rule config for each checked file's directory.
///
/// Implements [CHKARCH-CONFIG-DISCOVERY] (GitHub #311): every file is checked
/// with the config discovered from its own ancestor chain, so diagnostics are
/// independent of argument order, path spelling, and cwd. Memoized per
/// directory; `fallback` supplies the detected Python version when a
/// directory's chain does not pin one ([CHKARCH-VERSION-TARGET]).
pub(crate) fn resolve_dir_configs(
    python_files: &[String],
    fallback: &basilisk_config::BasiliskConfig,
) -> DirConfigs {
    let mut dir_configs = std::collections::BTreeMap::new();
    for path in python_files {
        let _ = dir_configs
            .entry(parent_dir_of(path))
            .or_insert_with_key(|dir| {
                let mut cfg = basilisk_config::load_basilisk_config(dir);
                if cfg.python_version.is_none() {
                    cfg.python_version.clone_from(&fallback.python_version);
                }
                if cfg.python_platform.is_none() {
                    cfg.python_platform.clone_from(&fallback.python_platform);
                }
                std::sync::Arc::new(cfg)
            });
    }
    dir_configs
}

/// The per-directory config for `path`, falling back to `fallback` (only
/// reachable if `path` was not in the file list the map was built from).
pub(crate) fn config_for_path(
    dir_configs: &DirConfigs,
    path: &str,
    fallback: &basilisk_config::BasiliskConfig,
) -> std::sync::Arc<basilisk_config::BasiliskConfig> {
    dir_configs
        .get(&parent_dir_of(path))
        .cloned()
        .unwrap_or_else(|| std::sync::Arc::new(fallback.clone()))
}

/// Walk up from `start` to find the project root (directory containing
/// `pyproject.toml` or `uv.lock`). Falls back to cwd, then `start`.
pub(crate) fn find_project_root(start: &std::path::Path) -> std::path::PathBuf {
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
mod tests;
