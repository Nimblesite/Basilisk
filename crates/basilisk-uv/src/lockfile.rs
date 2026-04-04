//! `uv.lock` file parser.
//!
//! The uv lock file is a TOML file that records the exact versions of all
//! resolved packages. This module deserialises it into structured Rust types.

use std::path::Path;

use serde::Deserialize;

use crate::error::UvError;

/// Top-level structure of a `uv.lock` file.
///
/// Uses `#[serde(flatten)]` with `HashMap` to tolerate unknown top-level
/// keys (e.g. `revision`) that newer uv versions may add.
#[derive(Debug, Clone, Deserialize)]
pub struct LockFile {
    /// Lock file format version.
    pub version: u32,

    /// Python version constraint from the lock file (e.g. `">=3.12"`).
    #[serde(rename = "requires-python", default)]
    pub requires_python: Option<String>,

    /// All resolved packages.
    #[serde(rename = "package", default)]
    pub packages: Vec<LockPackage>,

    /// Catch-all for unknown top-level fields (e.g. `revision`).
    #[serde(flatten)]
    pub _extra: std::collections::HashMap<String, toml::Value>,
}

/// A single resolved package entry.
///
/// Tolerates unknown fields (`sdist`, `wheels`, `metadata`, etc.) that are
/// present in real `uv.lock` files but not needed for type checking.
#[derive(Debug, Clone, Deserialize)]
pub struct LockPackage {
    /// Normalised package name.
    pub name: String,

    /// Resolved version string.
    pub version: String,

    /// Source information (registry, editable path, etc.).
    #[serde(default)]
    pub source: Option<LockSource>,

    /// Runtime dependencies.
    #[serde(default)]
    pub dependencies: Vec<LockDependency>,

    /// Development-only dependencies, keyed by group name (e.g. `"dev"`).
    ///
    /// In `uv.lock` format this is a table:
    /// ```toml
    /// [package.dev-dependencies]
    /// dev = [{ name = "pytest" }]
    /// ```
    #[serde(rename = "dev-dependencies", default)]
    pub dev_dependencies: std::collections::HashMap<String, Vec<LockDependency>>,

    /// Catch-all for unknown fields (`sdist`, `wheels`, `metadata`, etc.).
    #[serde(flatten)]
    pub _extra: std::collections::HashMap<String, toml::Value>,
}

/// Source metadata for a locked package.
#[derive(Debug, Clone, Deserialize)]
pub struct LockSource {
    /// Registry URL (e.g. `"https://pypi.org/simple"`).
    #[serde(default)]
    pub registry: Option<String>,

    /// Path to an editable install.
    #[serde(default)]
    pub editable: Option<String>,

    /// Virtual source marker.
    #[serde(rename = "virtual", default)]
    pub virtual_field: Option<String>,
}

/// A dependency reference within a lock entry.
#[derive(Debug, Clone, Deserialize)]
pub struct LockDependency {
    /// Dependency package name.
    pub name: String,

    /// Pinned version (omitted for workspace members).
    #[serde(default)]
    pub version: Option<String>,

    /// PEP 508 environment marker expression.
    #[serde(default)]
    pub marker: Option<String>,

    /// Catch-all for unknown fields (e.g. `specifier`).
    #[serde(flatten)]
    pub _extra: std::collections::HashMap<String, toml::Value>,
}

/// Parse a `uv.lock` file at the given path.
///
/// # Errors
///
/// Returns [`UvError::Io`] if the file cannot be read, or
/// [`UvError::TomlParse`] if it is not valid TOML matching the expected
/// schema.
pub fn parse_lock_file(path: &Path) -> Result<LockFile, UvError> {
    let display = path.display().to_string();

    let content = std::fs::read_to_string(path).map_err(|source| UvError::Io {
        path: display.clone(),
        source,
    })?;

    toml::from_str(&content).map_err(|source| UvError::TomlParse {
        path: display,
        source,
    })
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test-only: unwrap/indexing acceptable in unit tests"
)]
mod tests {
    use super::*;

    const REALISTIC_LOCK: &str = r#"
version = 1
requires-python = ">=3.12"

[[package]]
name = "my-project"
version = "0.1.0"
source = { virtual = "." }
dependencies = [
    { name = "requests" },
]

[package.dev-dependencies]
dev = [
    { name = "pytest" },
]

[[package]]
name = "requests"
version = "2.31.0"
source = { registry = "https://pypi.org/simple" }
dependencies = [
    { name = "urllib3" },
]

[[package]]
name = "urllib3"
version = "2.1.0"
source = { registry = "https://pypi.org/simple" }

[[package]]
name = "pytest"
version = "8.0.0"
source = { registry = "https://pypi.org/simple" }

[[package]]
name = "my-editable"
version = "0.2.0"
source = { editable = "../my-editable" }
"#;

    #[test]
    fn parses_realistic_lock_file() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join("uv.lock");
        std::fs::write(&lock_path, REALISTIC_LOCK).unwrap();

        let lock = parse_lock_file(&lock_path).unwrap();

        assert_eq!(lock.version, 1);
        assert_eq!(lock.requires_python.as_deref(), Some(">=3.12"));
        assert_eq!(lock.packages.len(), 5);
    }

    #[test]
    fn parses_package_names_and_versions() {
        let lock: LockFile = toml::from_str(REALISTIC_LOCK).unwrap();
        let names: Vec<&str> = lock.packages.iter().map(|p| p.name.as_str()).collect();

        assert!(names.contains(&"requests"));
        assert!(names.contains(&"urllib3"));
        assert!(names.contains(&"pytest"));
    }

    #[test]
    fn parses_source_registry() {
        let lock: LockFile = toml::from_str(REALISTIC_LOCK).unwrap();

        let requests = lock.packages.iter().find(|p| p.name == "requests").unwrap();

        let source = requests.source.as_ref().unwrap();
        assert_eq!(source.registry.as_deref(), Some("https://pypi.org/simple"));
    }

    #[test]
    fn parses_editable_source() {
        let lock: LockFile = toml::from_str(REALISTIC_LOCK).unwrap();

        let editable = lock
            .packages
            .iter()
            .find(|p| p.name == "my-editable")
            .unwrap();

        let source = editable.source.as_ref().unwrap();
        assert_eq!(source.editable.as_deref(), Some("../my-editable"));
    }

    #[test]
    fn parses_virtual_source() {
        let lock: LockFile = toml::from_str(REALISTIC_LOCK).unwrap();

        let project = lock
            .packages
            .iter()
            .find(|p| p.name == "my-project")
            .unwrap();

        let source = project.source.as_ref().unwrap();
        assert_eq!(source.virtual_field.as_deref(), Some("."));
    }

    #[test]
    fn parses_dependencies() {
        let lock: LockFile = toml::from_str(REALISTIC_LOCK).unwrap();

        let requests = lock.packages.iter().find(|p| p.name == "requests").unwrap();

        assert_eq!(requests.dependencies.len(), 1);
        assert_eq!(requests.dependencies[0].name, "urllib3");
    }

    #[test]
    fn parses_dev_dependencies() {
        let lock: LockFile = toml::from_str(REALISTIC_LOCK).unwrap();

        let project = lock
            .packages
            .iter()
            .find(|p| p.name == "my-project")
            .unwrap();

        assert_eq!(project.dev_dependencies.len(), 1);
        let dev_group = project.dev_dependencies.get("dev").unwrap();
        assert_eq!(dev_group.len(), 1);
        assert_eq!(dev_group[0].name, "pytest");
    }

    #[test]
    fn error_on_missing_file() {
        let result = parse_lock_file(Path::new("/nonexistent/uv.lock"));
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, UvError::Io { .. }));
    }

    #[test]
    fn error_on_invalid_toml() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join("uv.lock");
        std::fs::write(&lock_path, "not valid { toml").unwrap();

        let result = parse_lock_file(&lock_path);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, UvError::TomlParse { .. }));
    }

    #[test]
    fn parses_minimal_lock_file() {
        let minimal = "version = 1\n";
        let lock: LockFile = toml::from_str(minimal).unwrap();

        assert_eq!(lock.version, 1);
        assert!(lock.requires_python.is_none());
        assert!(lock.packages.is_empty());
    }

    #[test]
    fn parses_dependency_with_marker() {
        let content = r#"
version = 1

[[package]]
name = "colorama"
version = "0.4.6"

[[package]]
name = "click"
version = "8.1.7"
dependencies = [
    { name = "colorama", marker = "sys_platform == 'win32'" },
]
"#;
        let lock: LockFile = toml::from_str(content).unwrap();
        let click = lock.packages.iter().find(|p| p.name == "click").unwrap();

        assert_eq!(
            click.dependencies[0].marker.as_deref(),
            Some("sys_platform == 'win32'")
        );
    }
}
