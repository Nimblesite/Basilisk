//! One immutable resolver-facing Typeshed generation.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::archive::ArchiveVfs;
use super::source::{SourceIdentity, TypeshedStatus};
use super::versions::{VersionsError, VersionsIndex};

/// Module-name to immutable VFS path index derived from one snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleIndex(BTreeMap<String, String>);

impl ModuleIndex {
    /// Look up the exact archive path for a dotted stdlib module.
    #[must_use]
    pub fn path(&self, module: &str) -> Option<&str> {
        self.0.get(module).map(String::as_str)
    }

    /// Iterate module/path pairs in deterministic order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(module, path)| (module.as_str(), path.as_str()))
    }
}

/// Import-root to installable stub-distribution index for one snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistributionIndex(BTreeMap<String, String>);

impl DistributionIndex {
    /// Look up the distribution for a dotted import name.
    #[must_use]
    pub fn distribution(&self, module: &str) -> Option<&str> {
        let root = module.split('.').next().unwrap_or(module);
        self.0.get(root).map(String::as_str)
    }

    /// Iterate import-root/distribution pairs in deterministic order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(root, distribution)| (root.as_str(), distribution.as_str()))
    }
}

/// One complete, immutable, gate-accepted step-3 source.
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// Identity shared by status, checker fingerprints, indexes, and VFS URIs.
    pub identity: SourceIdentity,
    /// Shared CLI/LSP/MCP status for this identity.
    pub status: TypeshedStatus,
    /// Exact bytes consumed by the parser and resolver.
    pub vfs: ArchiveVfs,
    /// Exact `stdlib/VERSIONS` text retained without a fallible VFS re-read.
    versions_source: Option<String>,
    /// Parsed target-admission rules from this VFS's `stdlib/VERSIONS`.
    pub versions_index: VersionsIndex,
    /// Stdlib module names and paths derived from this VFS.
    pub module_index: ModuleIndex,
    /// Stub-distribution suggestions derived from this generation.
    pub distribution_index: DistributionIndex,
}

impl Snapshot {
    /// Build every derived view from the same VFS. `distribution_tsv` is used
    /// only by the bundled stdlib-only ZIP and must be attested by its sidecar
    /// before this call.
    ///
    /// # Errors
    ///
    /// Rejects identity drift, malformed `VERSIONS`, duplicate modules, or
    /// malformed distribution data. Stub bodies are parsed on demand so an
    /// unrelated upstream syntax change cannot reject the whole generation.
    pub fn build(
        identity: SourceIdentity,
        status: TypeshedStatus,
        vfs: ArchiveVfs,
        distribution_tsv: Option<&str>,
    ) -> Result<Self, SnapshotError> {
        let expected_identity = identity.uri_component();
        if vfs.identity() != expected_identity {
            return Err(SnapshotError::IdentityMismatch {
                expected: expected_identity,
                actual: vfs.identity().to_owned(),
            });
        }
        let module_index = build_module_index(&vfs)?;
        let versions_source = vfs.read_str("stdlib/VERSIONS").map(str::to_owned);
        let versions_index = match versions_source.as_deref() {
            Some(source) => VersionsIndex::parse(source).map_err(SnapshotError::Versions)?,
            None if matches!(identity, SourceIdentity::Custom { .. }) => {
                VersionsIndex::from_modules(module_index.iter().map(|(module, _)| module))
            }
            None => return Err(SnapshotError::MissingVersions),
        };
        let distribution_index = match distribution_tsv {
            Some(source) => parse_distribution_tsv(source)?,
            None => derive_distribution_index(&vfs),
        };
        Ok(Self {
            identity,
            status,
            vfs,
            versions_source,
            versions_index,
            module_index,
            distribution_index,
        })
    }

    /// The exact `stdlib/VERSIONS` text carried by this generation.
    #[must_use]
    pub fn versions(&self) -> Option<&str> {
        self.versions_source.as_deref()
    }

    /// Read a stdlib module body and its stable logical URI.
    #[must_use]
    pub fn read_stub(&self, module: &str) -> Option<(String, &str)> {
        let path = self.module_index.path(module)?;
        self.vfs
            .read_str(path)
            .map(|source| (self.vfs.logical_uri(path), source))
    }

    /// Read a stdlib module only when `VERSIONS` admits the target.
    #[must_use]
    pub fn read_stub_for_target(&self, module: &str, target: (u32, u32)) -> Option<(String, &str)> {
        self.versions_index
            .admits(module, target)
            .then(|| self.read_stub(module))
            .flatten()
    }
}

/// A derived-view or parse failure prevents snapshot activation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SnapshotError {
    /// The VFS URI identity did not match the snapshot identity.
    #[error("typeshed VFS identity mismatch: expected {expected}, got {actual}")]
    IdentityMismatch {
        /// Expected identity.
        expected: String,
        /// VFS identity.
        actual: String,
    },
    /// `stdlib/VERSIONS` was missing or not UTF-8.
    #[error("missing or non-UTF-8 stdlib/VERSIONS")]
    MissingVersions,
    /// `stdlib/VERSIONS` was malformed.
    #[error("{0}")]
    Versions(VersionsError),
    /// Two paths mapped to one dotted stdlib module.
    #[error("duplicate stdlib module {module}: {first} and {second}")]
    DuplicateModule {
        /// Dotted module name.
        module: String,
        /// First archive path.
        first: String,
        /// Conflicting archive path.
        second: String,
    },
    /// The bundled distribution sidecar had a malformed row.
    #[error("malformed distribution index line {line}: {text}")]
    DistributionLine {
        /// One-based line.
        line: usize,
        /// Bad row.
        text: String,
    },
    /// A distribution sidecar repeated an import root.
    #[error("duplicate distribution import root: {0}")]
    DuplicateDistribution(String),
}

fn build_module_index(vfs: &ArchiveVfs) -> Result<ModuleIndex, SnapshotError> {
    // The entry API moves the module name into the map on the vacant path, so
    // the common case allocates each name once — cloning per insert to keep a
    // copy for the never-taken duplicate branch doubled this loop's allocations
    // across ~750 stdlib modules, on the cold-start critical path.
    let mut modules: BTreeMap<String, String> = BTreeMap::new();
    for path in vfs
        .paths()
        .filter(|path| path.starts_with("stdlib/") && is_pyi_path(path))
    {
        match modules.entry(stdlib_module_name(path)) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                let _ = slot.insert(path.to_owned());
            }
            std::collections::btree_map::Entry::Occupied(slot) => {
                return Err(SnapshotError::DuplicateModule {
                    module: slot.key().clone(),
                    first: slot.get().clone(),
                    second: path.to_owned(),
                })
            }
        }
    }
    Ok(ModuleIndex(modules))
}

fn stdlib_module_name(path: &str) -> String {
    let relative = path
        .strip_prefix("stdlib/")
        .and_then(|path| path.strip_suffix(".pyi"))
        .unwrap_or(path);
    let relative = relative.strip_suffix("/__init__").unwrap_or(relative);
    relative.replace('/', ".")
}

fn derive_distribution_index(vfs: &ArchiveVfs) -> DistributionIndex {
    let mut candidates: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for path in vfs
        .paths()
        .filter(|path| path.starts_with("stubs/") && is_pyi_path(path))
    {
        let mut parts = path.split('/');
        let _ = parts.next();
        let Some(distribution) = parts.next() else {
            continue;
        };
        let Some(import_root) = parts.next() else {
            continue;
        };
        if import_root.starts_with('@') {
            continue;
        }
        let import_root = import_root.strip_suffix(".pyi").unwrap_or(import_root);
        let _ = candidates
            .entry(import_root.to_owned())
            .or_default()
            .insert(format!("types-{distribution}"));
    }
    DistributionIndex(
        candidates
            .into_iter()
            .filter_map(|(root, matches)| {
                (matches.len() == 1).then(|| (root, matches.into_iter().next().unwrap_or_default()))
            })
            .collect(),
    )
}

fn parse_distribution_tsv(source: &str) -> Result<DistributionIndex, SnapshotError> {
    let mut distributions = BTreeMap::new();
    for (offset, raw) in source.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (root, distribution) = line
            .split_once('\t')
            .filter(|(root, distribution)| !root.is_empty() && !distribution.is_empty())
            .ok_or_else(|| SnapshotError::DistributionLine {
                line: offset + 1,
                text: line.to_owned(),
            })?;
        if distributions
            .insert(root.to_owned(), distribution.to_owned())
            .is_some()
        {
            return Err(SnapshotError::DuplicateDistribution(root.to_owned()));
        }
    }
    Ok(DistributionIndex(distributions))
}

fn is_pyi_path(path: &str) -> bool {
    Path::new(path).extension() == Some(std::ffi::OsStr::new("pyi"))
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test-only fixed snapshot fixtures must fail loudly"
)]
mod tests {
    use super::super::archive::{Archive, ArchiveEntry};
    use super::super::gittree::FileMode;
    use super::super::source::{LicenseStatus, SourceKind, StatusWarning, TypeshedStatus};
    use super::*;

    fn reg(path: &str, data: &[u8]) -> ArchiveEntry {
        ArchiveEntry {
            path: path.to_owned(),
            mode: FileMode::Regular,
            data: data.to_vec().into(),
        }
    }

    fn snapshot() -> Snapshot {
        let identity = SourceIdentity::Custom {
            digest: "test".to_owned(),
        };
        let archive = Archive::new(vec![
            reg("stdlib/VERSIONS", b"os: 3.0-\nsys: 3.0-3.12\n"),
            reg("stdlib/sys.pyi", b"argv: list[str]\n"),
            reg("stdlib/os/__init__.pyi", b"def getcwd() -> str: ...\n"),
        ]);
        let status = TypeshedStatus {
            active_source: SourceKind::Custom,
            commit: None,
            tree: None,
            license_status: LicenseStatus::NotSupplied,
            license_reference: None,
            warnings: StatusWarning::list(&[]),
        };
        Snapshot::build(
            identity,
            status,
            ArchiveVfs::new("custom-test", archive),
            Some("requests\ttypes-requests\n"),
        )
        .expect("valid snapshot")
    }

    #[test]
    fn every_view_comes_from_one_identity() {
        let snapshot = snapshot();
        assert_eq!(
            snapshot.read_stub("sys").map(|(_, source)| source),
            Some("argv: list[str]\n")
        );
        assert_eq!(
            snapshot.read_stub("os").map(|(_, source)| source),
            Some("def getcwd() -> str: ...\n")
        );
        assert!(snapshot.read_stub_for_target("sys", (3, 12)).is_some());
        assert!(snapshot.read_stub_for_target("sys", (3, 13)).is_none());
        assert_eq!(
            snapshot.distribution_index.distribution("requests.auth"),
            Some("types-requests")
        );
    }

    #[test]
    fn conflicting_module_layout_blocks_activation() {
        let identity = SourceIdentity::Custom {
            digest: "bad".to_owned(),
        };
        let status = snapshot().status;
        let duplicate = Archive::new(vec![
            reg("stdlib/VERSIONS", b"os: 3.0-\n"),
            reg("stdlib/os.pyi", b"...\n"),
            reg("stdlib/os/__init__.pyi", b"...\n"),
        ]);
        assert!(matches!(
            Snapshot::build(
                identity,
                status,
                ArchiveVfs::new("custom-bad", duplicate),
                None,
            ),
            Err(SnapshotError::DuplicateModule { .. })
        ));
    }

    #[test]
    fn unrelated_parser_drift_does_not_reject_fresh_stdlib() {
        let identity = SourceIdentity::Custom {
            digest: "fresh".to_owned(),
        };
        let archive = Archive::new(vec![
            reg("stdlib/VERSIONS", b"os: 3.0-\ntkinter: 3.0-\n"),
            reg("stdlib/os.pyi", b"def getcwd() -> str: ...\n"),
            reg("stdlib/tkinter.pyi", b"def parser_drift( -> str: ...\n"),
            reg(
                "stubs/unrelated/unrelated/__init__.pyi",
                b"def parser_drift( -> str: ...\n",
            ),
        ]);
        let built = Snapshot::build(
            identity,
            snapshot().status,
            ArchiveVfs::new("custom-fresh", archive),
            None,
        );
        assert!(matches!(
            built,
            Ok(snapshot)
                if snapshot.read_stub("os").map(|(_, source)| source)
                    == Some("def getcwd() -> str: ...\n")
        ));
    }

    #[test]
    fn custom_tree_without_versions_admits_its_present_modules() {
        let identity = SourceIdentity::Custom {
            digest: "minimal".to_owned(),
        };
        let archive = Archive::new(vec![reg("stdlib/uasyncio.pyi", b"def run(coro): ...\n")]);
        let mut status = snapshot().status;
        status.commit = None;
        let built = Snapshot::build(
            identity,
            status,
            ArchiveVfs::new("custom-minimal", archive),
            None,
        );
        assert!(matches!(
            built,
            Ok(snapshot)
                if snapshot.versions().is_none()
                    && snapshot.versions_index.admits("uasyncio", (3, 12))
        ));
    }
}
