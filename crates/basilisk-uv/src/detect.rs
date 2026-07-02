//! Implements [LSPUV]. See docs/specs/LSP-UV-INTEGRATION-SPEC.md#LSPUV
//! uv project detection.
//!
//! Determines whether a workspace directory is a uv-managed Python project by
//! checking for characteristic files and markers.

use std::path::{Path, PathBuf};

use tracing::debug;

/// Information about a detected uv-managed project.
///
/// Implements [LSPUV-DETECTION-RESULT]: raw boolean signals plus the matched
/// root — consumers derive paths from `root` themselves.
#[derive(Debug, Clone)]
pub struct UvProjectInfo {
    /// Absolute path to the project root directory.
    pub root: PathBuf,

    /// Whether a `uv.lock` file exists at the root.
    pub has_lockfile: bool,

    /// Whether `pyproject.toml` contains a `[tool.uv]` section.
    pub has_tool_uv: bool,

    /// Whether the virtual environment was created by uv.
    pub uv_managed_venv: bool,

    /// Whether `.python-version` exists with no `poetry.lock`/`Pipfile.lock` —
    /// the Medium-confidence signal (`uv init` writes `.python-version` before
    /// the first sync creates `uv.lock`).
    pub has_python_version: bool,
}

/// Scan workspace roots and return info for the first directory that looks like
/// a uv project.
///
/// A directory qualifies if **any** of the following are true:
/// - A `uv.lock` file exists
/// - `pyproject.toml` contains a `[tool.uv]` section
/// - `.venv/pyvenv.cfg` contains a `uv = true` marker
/// - `.python-version` exists with no `poetry.lock`/`Pipfile.lock`
//
// Implements [LSPARCH-UV-DETECT] — the startup detection step that gates building
// the PackageRegistry (registry build/workspace-member discovery live in the LSP
// server's build_uv_registry and basilisk-uv's registry/workspace modules).
// Implements [LSPUV-DETECTION-SIGNALS] — filesystem-only signal check (no
// subprocess), evaluated in spec order: uv.lock, then `[tool.uv]`, then
// `.venv/pyvenv.cfg` `uv = true`, then the Medium-confidence `.python-version`
// signal (guarded against poetry/pipenv projects).
pub fn detect_uv_project(workspace_roots: &[PathBuf]) -> Option<UvProjectInfo> {
    for root in workspace_roots {
        let has_lockfile = root.join("uv.lock").is_file();
        let has_tool_uv = check_tool_uv_section(root);
        let uv_managed_venv = check_uv_managed_venv(root);
        let has_python_version = check_python_version_signal(root);

        if has_lockfile || has_tool_uv || uv_managed_venv || has_python_version {
            debug!(
                root = %root.display(),
                has_lockfile,
                has_tool_uv,
                uv_managed_venv,
                has_python_version,
                "detected uv project"
            );

            return Some(UvProjectInfo {
                root: root.clone(),
                has_lockfile,
                has_tool_uv,
                uv_managed_venv,
                has_python_version,
            });
        }
    }

    None
}

/// Check whether `pyproject.toml` at `root` contains a `[tool.uv]` section.
//
// Implements [LSPUV-DETECTION-SIGNALS] — the "uv config section" Definitive
// signal, via real TOML parsing (not string matching) per the avoid-regex rule.
fn check_tool_uv_section(root: &Path) -> bool {
    let path = root.join("pyproject.toml");

    let Ok(content) = std::fs::read_to_string(&path) else {
        return false;
    };

    // Parse the TOML and look for tool.uv rather than doing string matching.
    let Ok(table) = content.parse::<toml::Table>() else {
        return false;
    };

    table
        .get("tool")
        .and_then(toml::Value::as_table)
        .is_some_and(|tool| tool.contains_key("uv"))
}

/// Check whether `.venv/pyvenv.cfg` contains a `uv = true` marker.
//
// Implements [LSPUV-DETECTION-SIGNALS] — the "uv-created venv" High-confidence
// signal.
fn check_uv_managed_venv(root: &Path) -> bool {
    let path = root.join(".venv").join("pyvenv.cfg");

    let Ok(content) = std::fs::read_to_string(&path) else {
        return false;
    };

    content.lines().any(|line| {
        let trimmed = line.trim();
        trimmed == "uv = true"
            || trimmed
                .strip_prefix("uv")
                .is_some_and(|rest| rest.trim_start().starts_with("= true"))
    })
}

/// Check whether `.python-version` exists with no competing lock file.
//
// Implements [LSPUV-DETECTION-SIGNALS] — the ".python-version + no other
// manager" Medium-confidence signal. The poetry/pipenv lock guard keeps a
// pyenv-managed poetry/pipenv project from being misread as a uv project.
fn check_python_version_signal(root: &Path) -> bool {
    root.join(".python-version").is_file()
        && !root.join("poetry.lock").is_file()
        && !root.join("Pipfile.lock").is_file()
}

// Tests for [LSPUV-DETECTION-SIGNALS]: each detection signal in isolation,
// all together, the negative cases, and first-matching-root selection.
#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "test-only")]
mod tests {
    use super::*;

    fn setup_dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn detects_lockfile_only() {
        let dir = setup_dir();
        std::fs::write(dir.path().join("uv.lock"), "version = 1\n").unwrap();

        let info = detect_uv_project(&[dir.path().to_path_buf()]).unwrap();
        assert!(info.has_lockfile);
        assert!(!info.has_tool_uv);
        assert!(!info.uv_managed_venv);
    }

    #[test]
    fn detects_tool_uv_section() {
        let dir = setup_dir();
        let pyproject = "[project]\nname = \"foo\"\n\n[tool.uv]\ndev-dependencies = []\n";
        std::fs::write(dir.path().join("pyproject.toml"), pyproject).unwrap();

        let info = detect_uv_project(&[dir.path().to_path_buf()]).unwrap();
        assert!(!info.has_lockfile);
        assert!(info.has_tool_uv);
    }

    #[test]
    fn detects_uv_managed_venv() {
        let dir = setup_dir();
        let venv_dir = dir.path().join(".venv");
        std::fs::create_dir_all(&venv_dir).unwrap();
        std::fs::write(
            venv_dir.join("pyvenv.cfg"),
            "home = /usr/bin\nuv = true\nversion = 3.12\n",
        )
        .unwrap();

        let info = detect_uv_project(&[dir.path().to_path_buf()]).unwrap();
        assert!(info.uv_managed_venv);
    }

    #[test]
    fn detects_python_version_only() {
        let dir = setup_dir();
        std::fs::write(dir.path().join(".python-version"), "3.12\n").unwrap();

        let info = detect_uv_project(&[dir.path().to_path_buf()]).unwrap();
        assert!(info.has_python_version);
        assert!(!info.has_lockfile);
        assert!(!info.has_tool_uv);
        assert!(!info.uv_managed_venv);
    }

    #[test]
    fn python_version_with_poetry_lock_not_detected() {
        let dir = setup_dir();
        std::fs::write(dir.path().join(".python-version"), "3.12\n").unwrap();
        std::fs::write(dir.path().join("poetry.lock"), "").unwrap();

        assert!(detect_uv_project(&[dir.path().to_path_buf()]).is_none());
    }

    #[test]
    fn python_version_with_pipfile_lock_not_detected() {
        let dir = setup_dir();
        std::fs::write(dir.path().join(".python-version"), "3.12\n").unwrap();
        std::fs::write(dir.path().join("Pipfile.lock"), "{}").unwrap();

        assert!(detect_uv_project(&[dir.path().to_path_buf()]).is_none());
    }

    #[test]
    fn returns_none_for_non_uv_project() {
        let dir = setup_dir();
        assert!(detect_uv_project(&[dir.path().to_path_buf()]).is_none());
    }

    #[test]
    fn returns_none_for_empty_roots() {
        assert!(detect_uv_project(&[]).is_none());
    }

    #[test]
    fn picks_first_matching_root() {
        let dir1 = setup_dir();
        let dir2 = setup_dir();
        std::fs::write(dir2.path().join("uv.lock"), "version = 1\n").unwrap();

        let roots = vec![dir1.path().to_path_buf(), dir2.path().to_path_buf()];
        let info = detect_uv_project(&roots).unwrap();
        assert_eq!(info.root, dir2.path());
    }

    #[test]
    fn all_signals_detected() {
        let dir = setup_dir();
        std::fs::write(dir.path().join("uv.lock"), "version = 1\n").unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[tool.uv]\ndev-dependencies = []\n",
        )
        .unwrap();

        let venv_dir = dir.path().join(".venv");
        std::fs::create_dir_all(&venv_dir).unwrap();
        std::fs::write(venv_dir.join("pyvenv.cfg"), "uv = true\n").unwrap();

        std::fs::write(dir.path().join(".python-version"), "3.12\n").unwrap();

        let info = detect_uv_project(&[dir.path().to_path_buf()]).unwrap();
        assert!(info.has_lockfile);
        assert!(info.has_tool_uv);
        assert!(info.uv_managed_venv);
        assert!(info.has_python_version);
    }

    #[test]
    fn ignores_malformed_pyproject() {
        let dir = setup_dir();
        std::fs::write(dir.path().join("pyproject.toml"), "not valid { toml").unwrap();

        assert!(detect_uv_project(&[dir.path().to_path_buf()]).is_none());
    }

    #[test]
    fn uv_false_in_pyvenv_cfg_not_detected() {
        let dir = setup_dir();
        let venv_dir = dir.path().join(".venv");
        std::fs::create_dir_all(&venv_dir).unwrap();
        std::fs::write(venv_dir.join("pyvenv.cfg"), "uv = false\n").unwrap();

        assert!(detect_uv_project(&[dir.path().to_path_buf()]).is_none());
    }
}
