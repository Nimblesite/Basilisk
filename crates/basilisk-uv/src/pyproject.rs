//! Implements [LSPUV]. See docs/specs/LSP-UV-INTEGRATION-SPEC.md#LSPUV
//! `pyproject.toml` dependency extraction.
//!
//! Reads `[project].dependencies` from a `pyproject.toml` file to determine
//! which packages are direct dependencies of the project. This is used
//! alongside lock file parsing to classify packages as direct vs transitive.

use std::path::Path;

use tracing::debug;

/// Extract the list of direct dependency names from `pyproject.toml`.
///
/// Includes:
/// - `[project].dependencies` (PEP 621 runtime deps)
/// - `[project.optional-dependencies].*` (PEP 621 extras, e.g. `dev`, `test`)
/// - `[dependency-groups].*` (PEP 735, used by uv for dev groups)
///
/// All names are normalised (lowercased, hyphens replaced with underscores).
/// Returns an empty vec if the file is missing, malformed, or declares no deps.
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

    let mut deps = Vec::new();

    if let Some(arr) = table
        .get("project")
        .and_then(toml::Value::as_table)
        .and_then(|project| project.get("dependencies"))
        .and_then(toml::Value::as_array)
    {
        collect_deps_into(arr, &mut deps);
    }

    if let Some(extras) = table
        .get("project")
        .and_then(toml::Value::as_table)
        .and_then(|project| project.get("optional-dependencies"))
        .and_then(toml::Value::as_table)
    {
        for value in extras.values() {
            if let Some(arr) = value.as_array() {
                collect_deps_into(arr, &mut deps);
            }
        }
    }

    if let Some(groups) = table
        .get("dependency-groups")
        .and_then(toml::Value::as_table)
    {
        for value in groups.values() {
            if let Some(arr) = value.as_array() {
                collect_deps_into(arr, &mut deps);
            }
        }
    }

    deps
}

/// Append normalised package names from a TOML array of PEP 508 specifiers.
///
/// Skips entries that aren't strings (e.g. `{ include-group = "..." }` table
/// references in PEP 735 groups — those resolve transitively via the named
/// groups themselves, which we already iterate).
fn collect_deps_into(array: &[toml::Value], out: &mut Vec<String>) {
    for value in array {
        if let Some(spec) = value.as_str() {
            out.push(extract_package_name(spec).to_lowercase().replace('-', "_"));
        }
    }
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

    /// Write `content` as the workspace's `pyproject.toml` then return the deps it produces.
    /// The `TempDir` is returned so the caller controls its lifetime (drop = cleanup).
    fn deps_from_pyproject(content: &str) -> (tempfile::TempDir, Vec<String>) {
        let dir = setup_dir();
        std::fs::write(dir.path().join("pyproject.toml"), content).unwrap();
        let deps = extract_pyproject_deps(dir.path());
        (dir, deps)
    }

    #[test]
    fn extracts_simple_deps() {
        let (_dir, deps) = deps_from_pyproject(
            r#"
[project]
name = "my-app"
dependencies = ["requests", "flask", "click"]
"#,
        );
        assert_eq!(deps, vec!["requests", "flask", "click"]);
    }

    #[test]
    fn normalises_names() {
        let (_dir, deps) = deps_from_pyproject(
            r#"
[project]
name = "my-app"
dependencies = ["scikit-learn", "Python-dateutil"]
"#,
        );
        assert_eq!(deps, vec!["scikit_learn", "python_dateutil"]);
    }

    #[test]
    fn strips_version_constraints() {
        let (_dir, deps) = deps_from_pyproject(
            r#"
[project]
name = "my-app"
dependencies = ["requests>=2.28,<3.0", "flask~=2.0"]
"#,
        );
        assert_eq!(deps, vec!["requests", "flask"]);
    }

    #[test]
    fn strips_extras() {
        let (_dir, deps) = deps_from_pyproject(
            r#"
[project]
name = "my-app"
dependencies = ["flask[async]", "uvicorn[standard]>=0.20"]
"#,
        );
        assert_eq!(deps, vec!["flask", "uvicorn"]);
    }

    #[test]
    fn returns_empty_for_no_deps_section() {
        let (_dir, deps) = deps_from_pyproject(
            r#"
[project]
name = "my-app"
"#,
        );
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
        let (_dir, deps) = deps_from_pyproject("not { valid toml");
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

    /// Regression for issue #25: dev/optional dependencies declared in
    /// `[project.optional-dependencies]` (PEP 621) must also be returned, or
    /// `pytest`/`Pillow`/`GitPython` etc. wrongly trigger BSK-E0010.
    #[test]
    fn includes_pep621_optional_dependencies() {
        let (_dir, deps) = deps_from_pyproject(
            r#"
[project]
name = "my-app"
dependencies = ["requests"]

[project.optional-dependencies]
dev = ["pytest>=8.3.0", "Pillow>=11.0.0", "GitPython>=3.1.44"]
test = ["httpx>=0.28.0"]
"#,
        );
        for name in ["pytest", "pillow", "gitpython", "httpx"] {
            assert!(
                deps.contains(&name.to_owned()),
                "issue #25: {name} from optional-dependencies must be included; got {deps:?}"
            );
        }
    }

    /// Regression for issue #25: PEP 735 `[dependency-groups]` (uv-native) must
    /// be parsed too — `pytest`/`httpx` declared there are real direct deps.
    #[test]
    fn includes_pep735_dependency_groups() {
        let (_dir, deps) = deps_from_pyproject(
            r#"
[project]
name = "my-app"
dependencies = ["requests"]

[dependency-groups]
dev = ["pytest>=8.3.0", "httpx>=0.28.0"]
docs = ["sphinx>=7.0"]
"#,
        );
        for name in ["pytest", "httpx", "sphinx"] {
            assert!(
                deps.contains(&name.to_owned()),
                "issue #25: {name} from dependency-groups must be included; got {deps:?}"
            );
        }
    }
}
