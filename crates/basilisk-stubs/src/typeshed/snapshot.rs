//! One immutable resolver-facing Typeshed generation.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use super::archive::ArchiveVfs;
use super::source::{SourceIdentity, TypeshedStatus};
use super::versions::{VersionsError, VersionsIndex};

/// Module-name to immutable VFS path index derived from one snapshot.
///
/// Both halves are `Cow` because both are usually substrings of an archive
/// path the bundled snapshot already holds as `&'static str`: `stdlib/os.pyi`
/// yields the path verbatim and the module name `os` as a slice of it. Only a
/// package module (`os/path.pyi` → `os.path`) has to allocate, so indexing the
/// ~750-module bundle costs a few dozen allocations instead of ~1500 on every
/// cold start ([STUBRES-TYPESHED-BASELINE]).
///
/// A hash map, not an ordered one: every cold start builds the whole index and
/// then performs a handful of lookups, so the ~750 inserts are the cost that
/// matters and `iter`'s ordering is not. The deterministic order `iter`
/// promises is produced by sorting there instead — paid only by the callers
/// that actually walk the index, none of which are on the cold path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleIndex(HashMap<Cow<'static, str>, Cow<'static, str>>);

impl ModuleIndex {
    /// Look up the exact archive path for a dotted stdlib module.
    #[must_use]
    pub fn path(&self, module: &str) -> Option<&str> {
        self.0.get(module).map(Cow::as_ref)
    }

    /// Iterate module/path pairs in deterministic (module-name) order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        let mut pairs: Vec<(&str, &str)> = self
            .0
            .iter()
            .map(|(module, path)| (module.as_ref(), path.as_ref()))
            .collect();
        pairs.sort_unstable();
        pairs.into_iter()
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
    // Entries are walked rather than `vfs.paths()` so each path keeps its
    // `Cow`: for the bundled archive both the key and the value are then
    // slices of the `include_bytes!` constant, and the whole index builds
    // without touching the allocator. The entry API moves the name in on the
    // vacant path, so even the allocating cases allocate once — cloning per
    // insert to hold a copy for the never-taken duplicate branch doubled this
    // loop's cost across ~750 stdlib modules, on the cold-start critical path.
    let mut modules: HashMap<Cow<'static, str>, Cow<'static, str>> =
        HashMap::with_capacity(vfs.archive().len());
    for path in vfs
        .archive()
        .entries()
        .iter()
        .map(|entry| &entry.path)
        .filter(|path| path.starts_with("stdlib/") && is_pyi_path(path))
    {
        let (module, path) = stdlib_module_entry(path.clone());
        match modules.entry(module) {
            std::collections::hash_map::Entry::Vacant(slot) => {
                let _ = slot.insert(path);
            }
            std::collections::hash_map::Entry::Occupied(slot) => {
                return Err(SnapshotError::DuplicateModule {
                    module: slot.key().clone().into_owned(),
                    first: slot.get().clone().into_owned(),
                    second: path.into_owned(),
                })
            }
        }
    }
    Ok(ModuleIndex(modules))
}

/// A `stdlib/…` path paired with its dotted module name, which is borrowed
/// straight out of the path whenever the two coincide — as they do for every
/// top-level module, the large majority of the stdlib.
///
/// The path is taken and handed back by value because the name may borrow from
/// it: only the `Cow` variant carries the `'static` lifetime a slice of a
/// bundled path needs to reach the index, and a `&Cow` parameter would erase it.
fn stdlib_module_entry(path: Cow<'static, str>) -> (Cow<'static, str>, Cow<'static, str>) {
    let module = match &path {
        Cow::Borrowed(text) => {
            module_slice(text).map_or_else(|| Cow::Owned(dotted_module_name(text)), Cow::Borrowed)
        }
        Cow::Owned(text) => {
            Cow::Owned(module_slice(text).map_or_else(|| dotted_module_name(text), str::to_owned))
        }
    };
    (module, path)
}

/// The module name when it is a plain slice of the path — i.e. nothing is left
/// to rewrite into a dot, so the name and the slice are byte-identical.
fn module_slice(path: &str) -> Option<&str> {
    let relative = module_relative_path(path);
    (!relative.contains('/')).then_some(relative)
}

fn dotted_module_name(path: &str) -> String {
    module_relative_path(path).replace('/', ".")
}

fn module_relative_path(path: &str) -> &str {
    let relative = path
        .strip_prefix("stdlib/")
        .and_then(|path| path.strip_suffix(".pyi"))
        .unwrap_or(path);
    relative.strip_suffix("/__init__").unwrap_or(relative)
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
            path: path.to_owned().into(),
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
