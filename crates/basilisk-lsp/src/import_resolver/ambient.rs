//! Fast ambient-Python `site-packages` discovery.
//!
//! Implements [ANALYSIS-CROSSLSP-IMPORT]. A short-lived `basilisk check`
//! process must not start Python merely to recover a path that is encoded in
//! the interpreter layout. Common installations are resolved directly; the
//! interpreter probe remains the authoritative fallback for custom layouts.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Default)]
struct AmbientPythonEnvironment {
    search_path: Option<OsString>,
    python_path: Option<OsString>,
    home: Option<PathBuf>,
    user_base: Option<PathBuf>,
    app_data: Option<PathBuf>,
    python_home: Option<OsString>,
    no_user_site: Option<OsString>,
}

impl AmbientPythonEnvironment {
    fn current() -> Self {
        Self {
            search_path: std::env::var_os("PATH"),
            python_path: std::env::var_os("PYTHONPATH"),
            home: std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
            user_base: std::env::var_os("PYTHONUSERBASE")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
            app_data: std::env::var_os("APPDATA")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
            python_home: std::env::var_os("PYTHONHOME"),
            no_user_site: std::env::var_os("PYTHONNOUSERSITE"),
        }
    }
}

/// Discover ambient `site-packages`.
///
/// When an explicit interpreter is configured (VS Code's `basilisk.python` /
/// `--python`), resolve against **that** interpreter's `site-packages` —
/// including for cross-version checking ([TYPESHEDRT-ACCEPTANCE-TARGET]).
/// Otherwise fall back to conventional direct discovery and a `python3` probe.
pub(super) fn detect_python_site_packages(explicit_interpreter: Option<&Path>) -> Option<PathBuf> {
    if let Some(interpreter) = explicit_interpreter {
        return detect_for_interpreter(interpreter, None);
    }
    let environment = AmbientPythonEnvironment::current();
    detect_direct(&environment).or_else(|| {
        detect_for_interpreter_on(
            Path::new("python3"),
            None,
            environment.search_path.as_deref(),
        )
    })
}

/// `site-packages` for a configured interpreter — an explicit binary path or a
/// bare command name like `python3.12`
/// ([ANALYSIS-CROSSLSP-IMPORT], [TYPESHEDRT-ACCEPTANCE-TARGET]).
///
/// The interpreter's installation prefix is inspected first — no process spawn,
/// so it is deterministic and covers the venv / cross-version case; a `sys.path`
/// probe of the binary itself is the fallback for custom layouts.
pub(super) fn detect_for_interpreter(
    interpreter: &Path,
    target_version: Option<&str>,
) -> Option<PathBuf> {
    detect_for_interpreter_on(
        interpreter,
        target_version,
        std::env::var_os("PATH").as_deref(),
    )
}

/// [`detect_for_interpreter`] with the search path injected (tests cannot
/// mutate process env — `unsafe` under `unsafe_code = "deny"`).
///
/// A bare command name (`python3.12`) is first located on the search path so
/// its real installation prefix can be inspected: launching an interpreter on
/// every `check` to recover a directory the conventional `bin/` + `lib/`
/// layout already encodes is pure per-invocation waste. The raw location is
/// inspected before its canonicalized form — a venv `python` is a symlink to
/// the base interpreter, and canonicalizing first would skip the venv's own
/// `site-packages`.
fn detect_for_interpreter_on(
    interpreter: &Path,
    target_version: Option<&str>,
    search_path: Option<&std::ffi::OsStr>,
) -> Option<PathBuf> {
    let located = locate_bare_name(interpreter, search_path);
    let direct = located.as_deref().unwrap_or(interpreter);
    prefix_site_packages(direct, target_version)
        .or_else(|| {
            let canonical = std::fs::canonicalize(direct).ok()?;
            prefix_site_packages(&canonical, target_version)
        })
        .or_else(|| detect_with_interpreter(interpreter))
}

/// Prefix-encoded `site-packages` of an interpreter binary, if conventional.
fn prefix_site_packages(interpreter: &Path, target_version: Option<&str>) -> Option<PathBuf> {
    interpreter_prefix(interpreter)
        .and_then(|prefix| super::site_packages_in_dir_for_version(&prefix, target_version))
}

/// Locate a bare command name (no path separator) on the search path, exactly
/// as spawning it would. Explicit paths return `None` — they are already
/// located.
fn locate_bare_name(interpreter: &Path, search_path: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    let name = interpreter.to_str().filter(|name| {
        !name.is_empty() && !name.contains(std::path::MAIN_SEPARATOR) && !name.contains('/')
    })?;
    let suffix = std::env::consts::EXE_SUFFIX;
    let file_name = if suffix.is_empty() || name.to_ascii_lowercase().ends_with(suffix) {
        name.to_owned()
    } else {
        format!("{name}{suffix}")
    };
    search_path
        .into_iter()
        .flat_map(std::env::split_paths)
        .map(|directory| directory.join(&file_name))
        .find(|candidate| is_executable_file(candidate))
}

/// Installation prefix of an interpreter binary: strip a trailing `bin`/
/// `Scripts` directory (`<prefix>/bin/python` → `<prefix>`), otherwise the
/// binary's own directory (`<prefix>/python.exe` → `<prefix>`).
fn interpreter_prefix(interpreter: &Path) -> Option<PathBuf> {
    let parent = interpreter.parent()?;
    match parent.file_name().and_then(std::ffi::OsStr::to_str) {
        Some("bin" | "Scripts") => parent.parent().map(Path::to_path_buf),
        _ => Some(parent.to_path_buf()),
    }
}

/// Resolve the same conventional entries Python's `site` module adds, without
/// paying process startup on every CLI check.
fn detect_direct(environment: &AmbientPythonEnvironment) -> Option<PathBuf> {
    if environment
        .python_home
        .as_deref()
        .is_some_and(|value| !value.is_empty())
    {
        return None;
    }
    let executable = find_python_executable(environment)?;
    python_path_site_packages(environment).or_else(|| {
        if environment
            .no_user_site
            .as_deref()
            .is_some_and(python_env_flag_enabled)
        {
            return None;
        }
        let executable = std::fs::canonicalize(&executable).unwrap_or(executable);
        let version = python_version_from_path(&executable)?;
        user_site_packages(environment, &version)
    })
}

fn python_env_flag_enabled(value: &std::ffi::OsStr) -> bool {
    value
        .to_str()
        .is_none_or(|value| !value.is_empty() && value.parse::<i64>() != Ok(0))
}

fn python_path_site_packages(environment: &AmbientPythonEnvironment) -> Option<PathBuf> {
    environment
        .python_path
        .as_deref()
        .into_iter()
        .flat_map(std::env::split_paths)
        .find(|path| path.ends_with("site-packages") && path.is_dir())
        .map(|path| std::fs::canonicalize(&path).unwrap_or(path))
}

fn find_python_executable(environment: &AmbientPythonEnvironment) -> Option<PathBuf> {
    locate_bare_name(Path::new("python3"), environment.search_path.as_deref())
}

fn is_executable_file(candidate: &Path) -> bool {
    let Ok(metadata) = candidate.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn python_version_from_path(executable: &Path) -> Option<String> {
    executable.components().rev().find_map(|component| {
        component
            .as_os_str()
            .to_str()
            .and_then(parse_python_version_component)
    })
}

fn parse_python_version_component(component: &str) -> Option<String> {
    let lowercase = component.to_ascii_lowercase();
    let without_exe = lowercase.strip_suffix(".exe").unwrap_or(&lowercase);
    let version = without_exe.strip_prefix("python")?.trim_start_matches('@');
    parse_dotted_version(version).or_else(|| parse_compact_version(version))
}

fn parse_dotted_version(version: &str) -> Option<String> {
    let mut parts = version.split('.');
    let major = parts.next()?;
    let minor = parts.next()?;
    (numeric(major) && numeric(minor)).then(|| format!("{major}.{minor}"))
}

fn parse_compact_version(version: &str) -> Option<String> {
    let (major, minor) = version.split_at_checked(1)?;
    (version.len() >= 3 && numeric(major) && numeric(minor)).then(|| format!("{major}.{minor}"))
}

fn numeric(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn user_site_packages(environment: &AmbientPythonEnvironment, version: &str) -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        let base = environment.user_base.as_ref().map_or_else(
            || {
                environment
                    .home
                    .as_ref()
                    .map(|home| home.join("Library").join("Python").join(version))
            },
            |base| Some(base.clone()),
        )?;
        return existing_directory(base.join("lib/python/site-packages"));
    }
    if cfg!(windows) {
        let path = environment.user_base.as_ref().map_or_else(
            || {
                environment
                    .app_data
                    .as_ref()
                    .map(|base| windows_default_user_site(base, version))
            },
            |base| Some(windows_custom_user_site(base, version)),
        )?;
        return existing_directory(path);
    }
    let base = environment.user_base.as_ref().map_or_else(
        || environment.home.as_ref().map(|home| home.join(".local")),
        |base| Some(base.clone()),
    )?;
    existing_directory(base.join(format!("lib/python{version}/site-packages")))
}

fn windows_default_user_site(base: &Path, version: &str) -> PathBuf {
    base.join("Python")
        .join(format!("Python{}", version.replace('.', "")))
        .join("site-packages")
}

fn windows_custom_user_site(base: &Path, version: &str) -> PathBuf {
    base.join(format!("Python{}", version.replace('.', "")))
        .join("site-packages")
}

fn existing_directory(path: PathBuf) -> Option<PathBuf> {
    path.is_dir().then_some(path)
}

/// Custom Python builds can rewrite `sys.path` arbitrarily. Preserve the prior
/// interpreter-backed discovery when conventional direct lookup cannot prove a
/// valid directory.
fn detect_with_interpreter(interpreter: &Path) -> Option<PathBuf> {
    let output = std::process::Command::new(interpreter)
        .args(["-c", "import sys; print('\\n'.join(sys.path))"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;

    output
        .status
        .success()
        .then_some(output.stdout)
        .and_then(|stdout| {
            String::from_utf8_lossy(&stdout)
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(PathBuf::from)
                .find(|path| path.ends_with("site-packages") && path.is_dir())
        })
}

#[cfg(test)]
#[path = "ambient/tests.rs"]
mod tests;
