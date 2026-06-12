//! Implements [LSPUV]. See docs/specs/LSP-UV-INTEGRATION-SPEC.md#LSPUV
//! `.python-version` file parsing.
//!
//! uv projects typically contain a `.python-version` file at the workspace root
//! that pins the Python version used by the project.

use std::path::Path;

/// Read the Python version string from a `.python-version` file.
///
/// Returns `None` if the file does not exist or contains no valid version line.
/// A valid line is the first non-empty, non-comment line after stripping
/// whitespace.
pub fn read_python_version(root: &Path) -> Option<String> {
    let path = root.join(".python-version");
    let content = std::fs::read_to_string(&path).ok()?;

    content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .map(String::from)
}

/// Resolve the project's target Python version from project files, per
/// [LSPUV-PYTHON-VERSION-RESOLUTION-ORDER]:
///
/// 1. `.python-version` file (uv standard)
/// 2. `[project].requires-python` lower bound in `pyproject.toml`
/// 3. `uv.lock` top-level `requires-python` lower bound
///
/// Returns `None` when nothing pins a version — callers fall back to the
/// checker's centralized default ([CHKARCH-VERSION-TARGET], issue #93).
#[must_use]
pub fn resolve_target_python_version(root: &Path) -> Option<String> {
    read_python_version(root)
        .or_else(|| pyproject_requires_python(root))
        .or_else(|| lockfile_requires_python(root))
}

/// Lower bound of `[project].requires-python` in `pyproject.toml`, if any.
fn pyproject_requires_python(root: &Path) -> Option<String> {
    let content = std::fs::read_to_string(root.join("pyproject.toml")).ok()?;
    let table: toml::Table = content.parse().ok()?;
    let requires = table
        .get("project")?
        .as_table()?
        .get("requires-python")?
        .as_str()?;
    constraint_lower_bound(requires)
}

/// Lower bound of `requires-python` in `uv.lock`, if any.
fn lockfile_requires_python(root: &Path) -> Option<String> {
    let lock = crate::lockfile::parse_lock_file(&root.join("uv.lock")).ok()?;
    constraint_lower_bound(lock.requires_python.as_deref()?)
}

/// Extract a version from the `>=`/`==`/`~=` clause of a PEP 440 constraint,
/// e.g. `">=3.10,<4"` → `"3.10"`. `<`-only constraints carry no usable target.
fn constraint_lower_bound(constraint: &str) -> Option<String> {
    constraint.split(',').map(str::trim).find_map(|clause| {
        let version = clause
            .strip_prefix(">=")
            .or_else(|| clause.strip_prefix("=="))
            .or_else(|| clause.strip_prefix("~="))?;
        let version = version.trim().trim_end_matches(".*");
        version
            .split('.')
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
            .then(|| version.to_owned())
    })
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "test-only")]
mod tests {
    use super::*;

    #[test]
    fn reads_simple_version() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".python-version"), "3.12.1\n").unwrap();

        assert_eq!(read_python_version(dir.path()), Some("3.12.1".to_owned()));
    }

    #[test]
    fn skips_comments_and_blank_lines() {
        let dir = tempfile::tempdir().unwrap();
        let content = "# managed by uv\n\n  3.11.0  \n";
        std::fs::write(dir.path().join(".python-version"), content).unwrap();

        assert_eq!(read_python_version(dir.path()), Some("3.11.0".to_owned()));
    }

    #[test]
    fn returns_none_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_python_version(dir.path()), None);
    }

    #[test]
    fn returns_none_for_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".python-version"), "").unwrap();

        assert_eq!(read_python_version(dir.path()), None);
    }

    #[test]
    fn returns_none_for_comments_only() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".python-version"), "# 3.12\n# 3.11\n").unwrap();

        assert_eq!(read_python_version(dir.path()), None);
    }

    #[test]
    fn strips_surrounding_whitespace() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".python-version"), "  3.13.0  ").unwrap();

        assert_eq!(read_python_version(dir.path()), Some("3.13.0".to_owned()));
    }
}
