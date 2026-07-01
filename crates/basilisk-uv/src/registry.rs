//! Implements [LSPUV]. See docs/specs/LSP-UV-INTEGRATION-SPEC.md#LSPUV
//! Package registry built from a uv lock file.
//!
//! Provides fast lookup of Python packages by their import name, with
//! classification into direct, dev, and transitive dependency kinds.

use std::collections::HashMap;
use std::sync::Arc;

use crate::import_map::package_to_import_name;
use crate::lockfile::{LockFile, LockPackage};

/// Classification of a dependency in the project graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DepKind {
    /// Declared in `[project.dependencies]`.
    Direct,
    /// Declared in dev-dependency groups.
    Dev,
    /// Pulled in transitively.
    Transitive,
}

/// Resolved information about a single package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageInfo {
    /// Normalised `PyPI` package name.
    pub name: String,
    /// Resolved version string.
    pub version: String,
    /// Python import name (may differ from package name).
    pub import_name: String,
    /// How this package relates to the project.
    pub kind: DepKind,
    /// Whether this is an editable (local path) install.
    pub is_editable: bool,
}

/// An in-memory index of all resolved packages, keyed by import name.
#[derive(Debug, PartialEq, Eq)]
pub struct PackageRegistry {
    packages: HashMap<String, Arc<PackageInfo>>,
}

impl PackageRegistry {
    /// Build a registry from a parsed lock file and the project's declared
    /// direct dependencies.
    ///
    /// `pyproject_deps` should contain the normalised names from
    /// `[project.dependencies]` in `pyproject.toml`. Packages whose names
    /// appear there are classified as [`DepKind::Direct`]; those that appear
    /// in any package's `dev-dependencies` list are [`DepKind::Dev`]; all
    /// others are [`DepKind::Transitive`].
    //
    // Implements [LSPUV-LOCK-REGISTRY] — the fast lookup structure keyed by
    // normalised import name ([LSPUV-LOCK-IMPORT-MAPPING] via
    // `package_to_import_name`), carrying version, direct/dev/transitive
    // classification, and editable-source flag for each package.
    #[must_use]
    pub fn from_lock_file(lock: &LockFile, pyproject_deps: &[String]) -> Self {
        let dev_names = collect_dev_dep_names(&lock.packages);
        let normalised_deps = normalise_dep_list(pyproject_deps);

        let packages = lock
            .packages
            .iter()
            .map(|pkg| {
                let import_name = package_to_import_name(&pkg.name);
                let kind = classify_dep(&pkg.name, &normalised_deps, &dev_names);
                let is_editable = pkg.source.as_ref().is_some_and(|s| s.editable.is_some());

                let info = Arc::new(PackageInfo {
                    name: pkg.name.clone(),
                    version: pkg.version.clone(),
                    import_name: import_name.clone(),
                    kind,
                    is_editable,
                });

                (import_name, info)
            })
            .collect();

        Self { packages }
    }

    /// Look up a package by its Python import name.
    #[must_use]
    pub fn lookup(&self, import_name: &str) -> Option<&Arc<PackageInfo>> {
        self.packages.get(import_name)
    }

    /// Check whether a package with the given import name is known.
    #[must_use]
    pub fn has_package(&self, import_name: &str) -> bool {
        self.packages.contains_key(import_name)
    }

    /// Search for a type-stub package (e.g. `types-requests` for `requests`).
    ///
    /// Returns the import name of the stub package if found.
    //
    // Implements [LSPUV-DIAGNOSTICS-MISSING-STUBS] — the "a matching stub
    // package exists in uv.lock" precondition for the BSK-E0152 typeshed
    // suggestion. NOTE: only the `types_{name}` form is checked here; the
    // spec also names the `{name}-stubs` form, which this does not detect.
    // See conformance audit (DEVIATION).
    #[must_use]
    pub fn find_stub_package(&self, name: &str) -> Option<String> {
        let stub_import = format!("types_{name}");
        if self.packages.contains_key(&stub_import) {
            return Some(stub_import);
        }
        None
    }

    /// Iterate over all registered packages.
    pub fn all_packages(&self) -> impl Iterator<Item = &Arc<PackageInfo>> {
        self.packages.values()
    }
}

/// Collect all package names that appear in any package's dev-dependencies.
fn collect_dev_dep_names(packages: &[LockPackage]) -> Vec<String> {
    packages
        .iter()
        .flat_map(|pkg| pkg.dev_dependencies.values().flatten())
        .map(|dep| dep.name.to_lowercase())
        .collect()
}

/// Normalise a list of dependency names for comparison.
fn normalise_dep_list(deps: &[String]) -> Vec<String> {
    deps.iter()
        .map(|name| name.to_lowercase().replace('-', "_"))
        .collect()
}

/// Determine the dependency kind for a package.
fn classify_dep(package_name: &str, normalised_direct: &[String], dev_names: &[String]) -> DepKind {
    let normalised = package_name.to_lowercase().replace('-', "_");

    if normalised_direct.contains(&normalised) {
        return DepKind::Direct;
    }

    let lowered = package_name.to_lowercase();
    if dev_names.contains(&lowered) {
        return DepKind::Dev;
    }

    DepKind::Transitive
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "test-only")]
mod tests {
    use super::*;
    use crate::lockfile::{LockDependency, LockSource};

    fn make_dep(name: &str, version: Option<&str>) -> LockDependency {
        LockDependency {
            name: name.to_owned(),
            version: version.map(str::to_owned),
            marker: None,
            extra: HashMap::new(),
        }
    }

    fn make_pkg(
        name: &str,
        version: &str,
        source: Option<LockSource>,
        deps: Vec<LockDependency>,
        dev_deps: HashMap<String, Vec<LockDependency>>,
    ) -> LockPackage {
        LockPackage {
            name: name.to_owned(),
            version: version.to_owned(),
            source,
            dependencies: deps,
            dev_dependencies: dev_deps,
            extra: HashMap::new(),
        }
    }

    fn make_lock_file() -> LockFile {
        let dev_map = HashMap::from([("dev".to_owned(), vec![make_dep("pytest", Some("8.0.0"))])]);

        LockFile {
            version: 1,
            requires_python: Some(">=3.12".to_owned()),
            packages: vec![
                make_pkg(
                    "my-project",
                    "0.1.0",
                    Some(LockSource {
                        registry: None,
                        editable: None,
                        virtual_field: Some(".".to_owned()),
                    }),
                    vec![make_dep("requests", Some("2.31.0"))],
                    dev_map,
                ),
                make_pkg(
                    "requests",
                    "2.31.0",
                    Some(LockSource {
                        registry: Some("https://pypi.org/simple".to_owned()),
                        editable: None,
                        virtual_field: None,
                    }),
                    vec![make_dep("urllib3", Some("2.1.0"))],
                    HashMap::new(),
                ),
                make_pkg(
                    "urllib3",
                    "2.1.0",
                    Some(LockSource {
                        registry: Some("https://pypi.org/simple".to_owned()),
                        editable: None,
                        virtual_field: None,
                    }),
                    vec![],
                    HashMap::new(),
                ),
                make_pkg(
                    "pytest",
                    "8.0.0",
                    Some(LockSource {
                        registry: Some("https://pypi.org/simple".to_owned()),
                        editable: None,
                        virtual_field: None,
                    }),
                    vec![],
                    HashMap::new(),
                ),
                make_pkg(
                    "my-editable",
                    "0.2.0",
                    Some(LockSource {
                        registry: None,
                        editable: Some("../my-editable".to_owned()),
                        virtual_field: None,
                    }),
                    vec![],
                    HashMap::new(),
                ),
            ],
            extra: HashMap::new(),
        }
    }

    // [LSPUV-LOCK-REGISTRY]: direct/dev/transitive classification, editable
    // detection, lookup by import name, and import-name mapping on build.
    #[test]
    fn classifies_direct_dependency() {
        let lock = make_lock_file();
        let deps = vec!["requests".to_owned()];
        let registry = PackageRegistry::from_lock_file(&lock, &deps);

        let info = registry.lookup("requests").unwrap();
        assert_eq!(info.kind, DepKind::Direct);
    }

    #[test]
    fn classifies_dev_dependency() {
        let lock = make_lock_file();
        let deps = vec!["requests".to_owned()];
        let registry = PackageRegistry::from_lock_file(&lock, &deps);

        let info = registry.lookup("pytest").unwrap();
        assert_eq!(info.kind, DepKind::Dev);
    }

    #[test]
    fn classifies_transitive_dependency() {
        let lock = make_lock_file();
        let deps = vec!["requests".to_owned()];
        let registry = PackageRegistry::from_lock_file(&lock, &deps);

        let info = registry.lookup("urllib3").unwrap();
        assert_eq!(info.kind, DepKind::Transitive);
    }

    #[test]
    fn detects_editable_package() {
        let lock = make_lock_file();
        let registry = PackageRegistry::from_lock_file(&lock, &[]);

        let info = registry.lookup("my_editable").unwrap();
        assert!(info.is_editable);
    }

    #[test]
    fn non_editable_packages() {
        let lock = make_lock_file();
        let registry = PackageRegistry::from_lock_file(&lock, &[]);

        let info = registry.lookup("requests").unwrap();
        assert!(!info.is_editable);
    }

    #[test]
    fn has_package_returns_true_for_known() {
        let lock = make_lock_file();
        let registry = PackageRegistry::from_lock_file(&lock, &[]);

        assert!(registry.has_package("requests"));
    }

    #[test]
    fn has_package_returns_false_for_unknown() {
        let lock = make_lock_file();
        let registry = PackageRegistry::from_lock_file(&lock, &[]);

        assert!(!registry.has_package("nonexistent"));
    }

    #[test]
    fn lookup_returns_none_for_unknown() {
        let lock = make_lock_file();
        let registry = PackageRegistry::from_lock_file(&lock, &[]);

        assert!(registry.lookup("nonexistent").is_none());
    }

    #[test]
    fn all_packages_iterates_all() {
        let lock = make_lock_file();
        let registry = PackageRegistry::from_lock_file(&lock, &[]);

        let count = registry.all_packages().count();
        assert_eq!(count, 5);
    }

    #[test]
    fn import_name_mapping_applied() {
        let lock = make_lock_file();
        let registry = PackageRegistry::from_lock_file(&lock, &[]);

        // "my-project" normalises to "my_project"
        assert!(registry.has_package("my_project"));
        // "my-editable" normalises to "my_editable"
        assert!(registry.has_package("my_editable"));
    }

    // [LSPUV-DIAGNOSTICS-MISSING-STUBS]: a `types-<pkg>` entry in uv.lock is
    // discoverable as the matching stub package (keyed `types_<pkg>`).
    #[test]
    fn find_stub_package_found() {
        let lock = LockFile {
            version: 1,
            requires_python: None,
            packages: vec![
                make_pkg("requests", "2.31.0", None, vec![], HashMap::new()),
                make_pkg("types-requests", "2.31.0.0", None, vec![], HashMap::new()),
            ],
            extra: HashMap::new(),
        };

        let registry = PackageRegistry::from_lock_file(&lock, &[]);
        assert_eq!(
            registry.find_stub_package("requests"),
            Some("types_requests".to_owned())
        );
    }

    #[test]
    fn find_stub_package_not_found() {
        let lock = make_lock_file();
        let registry = PackageRegistry::from_lock_file(&lock, &[]);

        assert!(registry.find_stub_package("requests").is_none());
    }

    #[test]
    fn empty_lock_file() {
        let lock = LockFile {
            version: 1,
            requires_python: None,
            packages: vec![],
            extra: HashMap::new(),
        };

        let registry = PackageRegistry::from_lock_file(&lock, &[]);
        assert_eq!(registry.all_packages().count(), 0);
    }
}
