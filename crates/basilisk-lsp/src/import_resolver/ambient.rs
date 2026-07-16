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

pub(super) fn detect_python_site_packages() -> Option<PathBuf> {
    let environment = AmbientPythonEnvironment::current();
    detect_direct(&environment).or_else(detect_with_interpreter)
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
    let executable_name = format!("python3{}", std::env::consts::EXE_SUFFIX);
    environment
        .search_path
        .as_deref()
        .into_iter()
        .flat_map(std::env::split_paths)
        .map(|directory| directory.join(&executable_name))
        .find(|candidate| is_executable_file(candidate))
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
fn detect_with_interpreter() -> Option<PathBuf> {
    let output = std::process::Command::new("python3")
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
mod tests {
    use super::*;

    fn fake_python(prefix: &Path, version: &str) -> std::io::Result<PathBuf> {
        let bin = prefix.join("bin");
        std::fs::create_dir_all(&bin)?;
        let executable = bin.join(format!("python3{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&executable, [])?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755))?;
        }
        std::fs::create_dir_all(prefix.join(format!("lib/python{version}/site-packages")))?;
        Ok(executable)
    }

    #[test]
    fn parses_exact_python_versions_only() {
        assert_eq!(
            parse_python_version_component("python3.12"),
            Some("3.12".to_owned())
        );
        assert_eq!(
            parse_python_version_component("Python312"),
            Some("3.12".to_owned())
        );
        assert_eq!(
            parse_python_version_component("python3.14.exe"),
            Some("3.14".to_owned())
        );
        assert_eq!(parse_python_version_component("python3"), None);
    }

    #[test]
    fn direct_discovery_defers_without_a_user_site() -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let prefix = root.path().join("python3.12");
        let executable = fake_python(&prefix, "3.12")?;
        let search_path = std::env::join_paths([executable.parent().ok_or("missing bin")?])?;
        let environment = AmbientPythonEnvironment {
            search_path: Some(search_path),
            ..Default::default()
        };

        assert_eq!(detect_direct(&environment), None);
        Ok(())
    }

    #[test]
    fn direct_discovery_prefers_user_base() -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let prefix = root.path().join("python3.12");
        let executable = fake_python(&prefix, "3.12")?;
        let user_base = root.path().join("user");
        let user_site = if cfg!(target_os = "macos") {
            user_base.join("lib/python/site-packages")
        } else if cfg!(windows) {
            user_base.join("Python312/site-packages")
        } else {
            user_base.join("lib/python3.12/site-packages")
        };
        std::fs::create_dir_all(&user_site)?;
        let environment = AmbientPythonEnvironment {
            search_path: Some(std::env::join_paths([executable
                .parent()
                .ok_or("missing bin")?])?),
            user_base: Some(user_base),
            ..Default::default()
        };

        assert_eq!(detect_direct(&environment), Some(user_site));
        Ok(())
    }

    #[test]
    fn python_path_site_packages_has_priority() -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let site_packages = root.path().join("site-packages");
        std::fs::create_dir_all(&site_packages)?;
        let executable = fake_python(&root.path().join("python3.12"), "3.12")?;
        let environment = AmbientPythonEnvironment {
            search_path: Some(std::env::join_paths([executable
                .parent()
                .ok_or("missing bin")?])?),
            python_path: Some(std::env::join_paths([&site_packages])?),
            ..Default::default()
        };

        assert_eq!(
            detect_direct(&environment),
            Some(std::fs::canonicalize(site_packages)?)
        );
        Ok(())
    }

    #[test]
    fn no_user_site_flag_defers_to_interpreter() -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let prefix = root.path().join("python3.12");
        let executable = fake_python(&prefix, "3.12")?;
        let user_base = root.path().join("user");
        std::fs::create_dir_all(user_base.join("lib/python3.12/site-packages"))?;
        let environment = AmbientPythonEnvironment {
            search_path: Some(std::env::join_paths([executable
                .parent()
                .ok_or("missing bin")?])?),
            user_base: Some(user_base),
            no_user_site: Some("1".into()),
            ..Default::default()
        };

        assert_eq!(detect_direct(&environment), None);
        Ok(())
    }

    #[test]
    fn python_home_defers_even_with_python_path() -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let site_packages = root.path().join("site-packages");
        std::fs::create_dir_all(&site_packages)?;
        let executable = fake_python(&root.path().join("python3.12"), "3.12")?;
        let environment = AmbientPythonEnvironment {
            search_path: Some(std::env::join_paths([executable
                .parent()
                .ok_or("missing bin")?])?),
            python_path: Some(std::env::join_paths([site_packages])?),
            python_home: Some("/custom/python".into()),
            ..Default::default()
        };

        assert_eq!(detect_direct(&environment), None);
        Ok(())
    }

    #[test]
    fn zero_no_user_site_flag_keeps_user_site_enabled() {
        assert!(!python_env_flag_enabled(std::ffi::OsStr::new("0")));
        assert!(!python_env_flag_enabled(std::ffi::OsStr::new("00")));
        assert!(python_env_flag_enabled(std::ffi::OsStr::new("1")));
        assert!(python_env_flag_enabled(std::ffi::OsStr::new("yes")));
        assert!(python_env_flag_enabled(std::ffi::OsStr::new(" 0 ")));
        assert!(!python_env_flag_enabled(std::ffi::OsStr::new("+0")));
        assert!(!python_env_flag_enabled(std::ffi::OsStr::new("-0")));
    }

    #[test]
    #[cfg(unix)]
    fn path_lookup_rejects_non_executable_python() -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let executable = root.path().join("python3");
        std::fs::write(executable, [])?;
        let environment = AmbientPythonEnvironment {
            search_path: Some(std::env::join_paths([root.path()])?),
            ..Default::default()
        };

        assert_eq!(find_python_executable(&environment), None);
        Ok(())
    }
}
