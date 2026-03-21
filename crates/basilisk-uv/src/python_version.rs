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
