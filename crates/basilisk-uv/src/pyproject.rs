//! `pyproject.toml` dependency extraction.
//!
//! Reads `[project].dependencies` from a `pyproject.toml` file to determine
//! which packages are direct dependencies of the project. This is used
//! alongside lock file parsing to classify packages as direct vs transitive.

use std::path::Path;

use tracing::debug;

/// Extract the list of direct dependency names from `pyproject.toml`.
///
/// Reads the `[project].dependencies` array and returns normalised package
/// names (lowercased, hyphens replaced with underscores). Returns an empty
/// vec if the file is missing, malformed, or has no dependencies section.
#[must_use]
pub fn extract_pyproject_deps(root: &Path) -> Vec<String> {
    let path = root.join("pyproject.toml");

    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };

    let Ok(table) = content.parse::<toml::Table>() else {
        debug!(path = %path.display(), "failed to parse pyproject.toml as TOML");
        return Vec::new();
    };

    let Some(deps_array) = table
        .get("project")
        .and_then(toml::Value::as_table)
        .and_then(|project| project.get("dependencies"))
        .and_then(toml::Value::as_array)
    else {
        return Vec::new();
    };

    deps_array
        .iter()
        .filter_map(toml::Value::as_str)
        .map(|dep| extract_package_name(dep).to_lowercase().replace('-', "_"))
        .collect()
}

/// Extract the package name from a PEP 508 dependency specifier.
///
/// Strips version constraints, extras, and markers.
/// e.g. `"requests>=2.28,<3.0"` → `"requests"`,
///      `"flask[async]"` → `"flask"`.
fn extract_package_name(specifier: &str) -> &str {
    let trimmed = specifier.trim();

    // Find the first character that isn't part of the package name.
    // Package names: letters, digits, hyphens, underscores, dots.
    let end = trimmed
        .find(|ch: char| !ch.is_alphanumeric() && ch != '-' && ch != '_' && ch != '.')
        .unwrap_or(trimmed.len());

    &trimmed[..end]
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "test-only")]
mod tests {
    use super::*;

    fn setup_dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn extracts_simple_deps() {
        let dir = setup_dir();
        let content = r#"
[project]
name = "my-app"
dependencies = ["requests", "flask", "click"]
"#;
        std::fs::write(dir.path().join("pyproject.toml"), content).unwrap();

        let deps = extract_pyproject_deps(dir.path());
        assert_eq!(deps, vec!["requests", "flask", "click"]);
    }

    #[test]
    fn normalises_names() {
        let dir = setup_dir();
        let content = r#"
[project]
name = "my-app"
dependencies = ["scikit-learn", "Python-dateutil"]
"#;
        std::fs::write(dir.path().join("pyproject.toml"), content).unwrap();

        let deps = extract_pyproject_deps(dir.path());
        assert_eq!(deps, vec!["scikit_learn", "python_dateutil"]);
    }

    #[test]
    fn strips_version_constraints() {
        let dir = setup_dir();
        let content = r#"
[project]
name = "my-app"
dependencies = ["requests>=2.28,<3.0", "flask~=2.0"]
"#;
        std::fs::write(dir.path().join("pyproject.toml"), content).unwrap();

        let deps = extract_pyproject_deps(dir.path());
        assert_eq!(deps, vec!["requests", "flask"]);
    }

    #[test]
    fn strips_extras() {
        let dir = setup_dir();
        let content = r#"
[project]
name = "my-app"
dependencies = ["flask[async]", "uvicorn[standard]>=0.20"]
"#;
        std::fs::write(dir.path().join("pyproject.toml"), content).unwrap();

        let deps = extract_pyproject_deps(dir.path());
        assert_eq!(deps, vec!["flask", "uvicorn"]);
    }

    #[test]
    fn returns_empty_for_no_deps_section() {
        let dir = setup_dir();
        let content = r#"
[project]
name = "my-app"
"#;
        std::fs::write(dir.path().join("pyproject.toml"), content).unwrap();

        let deps = extract_pyproject_deps(dir.path());
        assert!(deps.is_empty());
    }

    #[test]
    fn returns_empty_for_missing_pyproject() {
        let dir = setup_dir();
        let deps = extract_pyproject_deps(dir.path());
        assert!(deps.is_empty());
    }

    #[test]
    fn returns_empty_for_malformed_toml() {
        let dir = setup_dir();
        std::fs::write(dir.path().join("pyproject.toml"), "not { valid toml").unwrap();

        let deps = extract_pyproject_deps(dir.path());
        assert!(deps.is_empty());
    }

    #[test]
    fn extract_package_name_simple() {
        assert_eq!(extract_package_name("requests"), "requests");
    }

    #[test]
    fn extract_package_name_with_version() {
        assert_eq!(extract_package_name("requests>=2.28"), "requests");
    }

    #[test]
    fn extract_package_name_with_extras() {
        assert_eq!(extract_package_name("flask[async]"), "flask");
    }

    #[test]
    fn extract_package_name_with_spaces() {
        assert_eq!(extract_package_name("  requests >= 2.0  "), "requests");
    }
}
