//! Implements [LSPUV]. See docs/specs/LSP-UV-INTEGRATION-SPEC.md#LSPUV
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
    pub extra: std::collections::HashMap<String, toml::Value>,
}

/// A single resolved package entry.
///
/// Tolerates unknown fields (`sdist`, `wheels`, `metadata`, etc.) that are
/// present in real `uv.lock` files but not needed for type checking.
//
// Implements [LSPUV-LOCK-EXTRACT] — extracts name, version, source,
// dependencies, and wheel hashes. Top-level `resolution-markers` are retained
// in `extra` rather than interpreted.
#[derive(Debug, Clone, Deserialize)]
pub struct LockPackage {
    /// Normalised package name.
    pub name: String,

    /// Resolved version string.
    ///
    /// Lockfile revision 3 omits this for editable workspace members with
    /// dynamic versioning (e.g. Airflow's `apache-airflow-ctl`).
    #[serde(default)]
    pub version: Option<String>,

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

    /// Wheel artifacts and their content hashes (`wheels[].hash`), captured so
    /// a recognised typeshed-distribution package can be auto-pinned by its
    /// wheel SHA-256 ([STUBRES-TYPESHED-PYPI], issue #312).
    #[serde(default)]
    pub wheels: Vec<LockWheel>,

    /// Catch-all for unknown fields (`sdist`, `metadata`, etc.).
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, toml::Value>,
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

/// A wheel artifact recorded for a locked package, carrying the content hash
/// `uv` records as `hash = "sha256:<hex>"`
/// ([uv lockfile format](https://docs.astral.sh/uv/reference/files/#lockfile-format)).
///
/// Only `hash` is interpreted; `url` and any future fields are retained in
/// `extra` so an unknown wheel attribute never rejects the whole lock file.
#[derive(Debug, Clone, Deserialize)]
pub struct LockWheel {
    /// Wheel download URL (`files.pythonhosted.org` for registry packages).
    #[serde(default)]
    pub url: Option<String>,

    /// Content hash, e.g. `"sha256:<64-hex>"`.
    #[serde(default)]
    pub hash: Option<String>,

    /// Catch-all for unknown wheel fields (e.g. `filename`).
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, toml::Value>,
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
    pub extra: std::collections::HashMap<String, toml::Value>,
}

/// Parse a `uv.lock` file at the given path.
///
/// # Errors
///
/// Returns [`UvError::Io`] if the file cannot be read, or
/// [`UvError::TomlParse`] if it is not valid TOML matching the expected
/// schema.
//
// Implements [LSPUV-LOCK-EXTRACT] — pure in-Rust TOML deserialisation of the
// lock file (zero subprocess), the foundation of [LSPUV-WHY] / [LSPUV-LOCK].
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

/// Package names `uv.lock` may pin that ship a typeshed-shaped `stdlib/` tree
/// Basilisk can use as a step-3 source ([STUBRES-TYPESHED-PYPI], issue #312).
/// Recognised by normalised name (case- and `-`/`_`-insensitive). Curated so a
/// random dependency never silently replaces the stdlib source.
const TYPESHED_DISTRIBUTION_PACKAGES: &[&str] = &["micropython-stdlib-stubs"];

/// Whether a locked package name is a recognised typeshed distribution.
fn is_typeshed_distribution(name: &str) -> bool {
    let normalised = name.to_lowercase().replace('-', "_");
    TYPESHED_DISTRIBUTION_PACKAGES
        .iter()
        .any(|recognised| recognised.to_lowercase().replace('-', "_") == normalised)
}

/// Strip the `sha256:` prefix from a `uv.lock` wheel `hash` and return the
/// raw 64-hex digest, or `None` if it is absent/malformed.
fn wheel_sha256_hex(hash: &str) -> Option<&str> {
    let hex = hash.strip_prefix("sha256:")?;
    (hex.len() == 64 && hex.as_bytes().iter().all(u8::is_ascii_hexdigit))
        .then_some(hex)
}

/// If `uv.lock` pins **exactly one** recognised typeshed-distribution package,
/// return its name and the 64-hex SHA-256 of its (first hashed) wheel
/// ([STUBRES-TYPESHED-PYPI], issue #312). Ambiguous (more than one candidate)
/// or absent → `None`: no auto-pin, the bundled default stands with
/// `typeshed_source_unpinned`.
///
/// This is pure over a parsed lock file — no disk I/O — so it is trivially
/// testable. A package with no hashed wheel does not count.
#[must_use]
pub fn find_typeshed_package_pin(lock: &LockFile) -> Option<(String, String)> {
    let mut found: Option<(String, String)> = None;
    for pkg in &lock.packages {
        if !is_typeshed_distribution(&pkg.name) {
            continue;
        }
        let Some(hex) = pkg
            .wheels
            .iter()
            .find_map(|wheel| wheel.hash.as_deref().and_then(wheel_sha256_hex))
        else {
            continue;
        };
        if found.is_some() {
            return None;
        }
        found = Some((pkg.name.clone(), hex.to_owned()));
    }
    found
}

/// Resolve the typeshed-distribution pin a `uv.lock` carries, as a
/// `name@sha256:<hex>` spec string ready for `typeshed-package`
/// ([STUBRES-TYPESHED-PYPI], issue #312). Returns `None` when this is not a uv
/// project, has no lockfile, the lockfile is unreadable, or no single
/// recognised package is pinned — callers then fall back to the bundled
/// default. Disk I/O is confined to this function.
#[must_use]
pub fn resolve_typeshed_package_pin(project_root: &Path) -> Option<String> {
    use crate::detect::detect_uv_project;
    let uv_info = detect_uv_project(&[project_root.to_path_buf()])?;
    if !uv_info.has_lockfile {
        return None;
    }
    let lock = parse_lock_file(&uv_info.root.join("uv.lock")).ok()?;
    let (name, sha256) = find_typeshed_package_pin(&lock)?;
    Some(format!("{name}@sha256:{sha256}"))
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test-only: unwrap/indexing acceptable in unit tests"
)]
mod tests {
    use super::*;

    /// Find a package by name in a lock file, panicking if absent.
    fn pkg<'a>(lock: &'a LockFile, name: &str) -> &'a LockPackage {
        lock.packages.iter().find(|p| p.name == name).unwrap()
    }

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

    // [LSPUV-LOCK-EXTRACT]: a realistic uv.lock parses with the right package
    // count, requires-python, names/versions, sources, and (dev-)dependencies.
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
        let source = pkg(&lock, "requests").source.as_ref().unwrap();
        assert_eq!(source.registry.as_deref(), Some("https://pypi.org/simple"));
    }

    #[test]
    fn parses_editable_source() {
        let lock: LockFile = toml::from_str(REALISTIC_LOCK).unwrap();
        let source = pkg(&lock, "my-editable").source.as_ref().unwrap();
        assert_eq!(source.editable.as_deref(), Some("../my-editable"));
    }

    #[test]
    fn parses_virtual_source() {
        let lock: LockFile = toml::from_str(REALISTIC_LOCK).unwrap();
        let source = pkg(&lock, "my-project").source.as_ref().unwrap();
        assert_eq!(source.virtual_field.as_deref(), Some("."));
    }

    #[test]
    fn parses_dependencies() {
        let lock: LockFile = toml::from_str(REALISTIC_LOCK).unwrap();
        let requests = pkg(&lock, "requests");
        assert_eq!(requests.dependencies.len(), 1);
        assert_eq!(requests.dependencies[0].name, "urllib3");
    }

    #[test]
    fn parses_dev_dependencies() {
        let lock: LockFile = toml::from_str(REALISTIC_LOCK).unwrap();
        let project = pkg(&lock, "my-project");
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

    // [LSPUV-LOCK-EXTRACT]: lockfile revision 3 omits `version` for editable
    // workspace members with dynamic versioning (e.g. apache-airflow-ctl in
    // Airflow's uv.lock). Parsing must tolerate the absent field instead of
    // rejecting the whole lock file (issue #320).
    #[test]
    fn parses_revision3_package_without_version() {
        let content = r#"
version = 1
revision = 3
requires-python = ">=3.10"

[[package]]
name = "apache-airflow-ctl"
source = { editable = "airflow-ctl" }
dependencies = [
    { name = "argcomplete" },
]

[[package]]
name = "argcomplete"
version = "3.5.3"
source = { registry = "https://pypi.org/simple" }
"#;
        let lock = toml::from_str::<LockFile>(content).unwrap();

        assert_eq!(lock.packages.len(), 2);
        let member = pkg(&lock, "apache-airflow-ctl");
        assert_eq!(
            member.source.as_ref().unwrap().editable.as_deref(),
            Some("airflow-ctl")
        );
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
        let click = pkg(&lock, "click");

        assert_eq!(
            click.dependencies[0].marker.as_deref(),
            Some("sys_platform == 'win32'")
        );
    }

    // [STUBRES-TYPESHED-PYPI] (issue #312): `wheels[].hash` is captured so a
    // recognised typeshed distribution can be auto-pinned by its wheel SHA-256.
    #[test]
    fn wheels_hash_is_captured() {
        let content = r#"
version = 1

[[package]]
name = "micropython-stdlib-stubs"
version = "1.0.0"
wheels = [
    { url = "https://files.pythonhosted.org/x.whl", hash = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" },
]
"#;
        let lock: LockFile = toml::from_str(content).unwrap();
        let pkg = pkg(&lock, "micropython-stdlib-stubs");
        assert_eq!(pkg.wheels.len(), 1);
        assert_eq!(
            pkg.wheels[0].hash.as_deref(),
            Some("sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
    }

    // [STUBRES-TYPESHED-PYPI] (issue #312): exactly one recognised typeshed
    // distribution pinned in uv.lock → auto-resolve its wheel SHA-256.
    #[test]
    fn find_typeshed_package_pin_resolves_a_single_candidate() {
        let content = r#"
version = 1

[[package]]
name = "my-project"
source = { virtual = "." }

[[package]]
name = "micropython-stdlib-stubs"
version = "1.0.0"
wheels = [
    { url = "https://files.pythonhosted.org/x.whl", hash = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" },
]
"#;
        let lock: LockFile = toml::from_str(content).unwrap();
        assert_eq!(
            find_typeshed_package_pin(&lock),
            Some((
                "micropython-stdlib-stubs".to_owned(),
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_owned(),
            ))
        );
    }

    // [STUBRES-TYPESHED-PYPI] (issue #312): two candidates is ambiguous → no
    // auto-pin (the bundled default + `typeshed_source_unpinned` stand).
    #[test]
    fn find_typeshed_package_pin_is_none_when_ambiguous() {
        let content = r#"
version = 1

[[package]]
name = "micropython-stdlib-stubs"
version = "1.0.0"
wheels = [
    { hash = "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc" },
]

[[package]]
name = "MicroPython_Stdlib_Stubs"
version = "1.0.0"
wheels = [
    { hash = "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd" },
]
"#;
        let lock: LockFile = toml::from_str(content).unwrap();
        assert_eq!(find_typeshed_package_pin(&lock), None);
    }

    // [STUBRES-TYPESHED-PYPI] (issue #312): no recognised package → no auto-pin.
    #[test]
    fn find_typeshed_package_pin_is_none_when_absent() {
        let content = r#"
version = 1

[[package]]
name = "requests"
version = "2.31.0"
wheels = [
    { hash = "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee" },
]
"#;
        let lock: LockFile = toml::from_str(content).unwrap();
        assert_eq!(find_typeshed_package_pin(&lock), None);
    }

    // A recognised package whose wheel has no hash (or a malformed hash) does
    // not count toward a pin — the content attestation IS the pin.
    #[test]
    fn find_typeshed_package_pin_ignores_a_wheel_without_a_valid_hash() {
        let content = r#"
version = 1

[[package]]
name = "micropython-stdlib-stubs"
version = "1.0.0"
wheels = [
    { url = "https://files.pythonhosted.org/x.whl" },
]
"#;
        let lock: LockFile = toml::from_str(content).unwrap();
        assert_eq!(find_typeshed_package_pin(&lock), None);
    }
}
