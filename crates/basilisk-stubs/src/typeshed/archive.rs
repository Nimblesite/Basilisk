//! Implements [STUBRES-TYPESHED] archive model + VFS. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED
//!
//! In-memory archive model and archive VFS.
//!
//! An [`Archive`] is the logical, prefix-stripped view of a downloaded commit
//! archive or the bundled snapshot: a flat set of files, each with a
//! repo-relative path, a Git file mode, and its bytes. Every activation gate and
//! the `.pyi` reader operate on this model, so the whole verification pipeline is
//! testable with no network and no on-disk extraction.
//!
//! [`ArchiveVfs`] is the read side the resolver consumes. It returns a **stable
//! logical URI** plus bytes for `parse_pyi_source` ([STUBRES-PYI]) — there is no
//! temp extraction and no real filesystem path, so a cached immutable ZIP is
//! never trusted through a mutable extracted tree ([STUBRES-TYPESHED-PIN]).

use std::collections::HashMap;
use std::sync::Arc;

use super::gittree::{git_blob_oid, reconstruct_root_tree_oid, FileMode, GitFile, Oid, TreeError};

/// One file in a logical archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveEntry {
    /// Repo-relative path with the archive's top-level prefix removed
    /// (e.g. `stdlib/os.pyi`), always `/`-separated.
    pub path: String,
    /// The file's Git mode (regular, executable, symlink, or submodule).
    pub mode: FileMode,
    /// The file's raw bytes (for a symlink, the link target).
    pub data: Vec<u8>,
}

/// A logical, prefix-stripped archive: a flat set of files by path.
#[derive(Debug, Clone)]
pub struct Archive {
    entries: Vec<ArchiveEntry>,
    by_path: HashMap<String, usize>,
}

impl Archive {
    /// Build an archive from its entries.
    ///
    /// The path index keeps the **last** entry for any repeated path; duplicate
    /// detection is the Safety gate's job, not this constructor's.
    #[must_use]
    pub fn new(entries: Vec<ArchiveEntry>) -> Self {
        let by_path = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (entry.path.clone(), index))
            .collect();
        Self { entries, by_path }
    }

    /// All entries, in insertion order.
    #[must_use]
    pub fn entries(&self) -> &[ArchiveEntry] {
        &self.entries
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the archive has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Total decompressed byte size across all entries, saturating.
    #[must_use]
    pub fn total_data_len(&self) -> u64 {
        self.entries
            .iter()
            .map(|entry| u64::try_from(entry.data.len()).unwrap_or(u64::MAX))
            .fold(0u64, u64::saturating_add)
    }

    /// Look up a single entry by exact path.
    #[must_use]
    pub fn get(&self, path: &str) -> Option<&ArchiveEntry> {
        self.by_path
            .get(path)
            .and_then(|index| self.entries.get(*index))
    }

    /// The files as [`GitFile`]s for Content-gate tree reconstruction.
    #[must_use]
    pub fn git_files(&self) -> Vec<GitFile> {
        self.entries
            .iter()
            .map(|entry| GitFile {
                path: entry.path.clone(),
                oid: git_blob_oid(&entry.data),
                mode: entry.mode,
            })
            .collect()
    }

    /// Reconstruct this archive's Git root-tree object ID ([`gittree`]).
    ///
    /// # Errors
    ///
    /// Returns [`TreeError`] if a path is empty, duplicated, or used as both a
    /// file and a directory.
    ///
    /// [`gittree`]: super::gittree
    pub fn root_tree_oid(&self) -> Result<Oid, TreeError> {
        reconstruct_root_tree_oid(&self.git_files())
    }
}

/// The read side of an [`Archive`]: stable logical URIs plus bytes.
#[derive(Debug, Clone)]
pub struct ArchiveVfs {
    identity: String,
    archive: Arc<Archive>,
}

impl ArchiveVfs {
    /// Wrap an archive under a source identity (e.g. a commit SHA or
    /// `bundled-<sha>`), which anchors every logical URI.
    #[must_use]
    pub fn new(identity: impl Into<String>, archive: Archive) -> Self {
        Self {
            identity: identity.into(),
            archive: Arc::new(archive),
        }
    }

    /// The source identity anchoring this VFS.
    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// The stable logical URI for a path — `typeshed:<identity>/<path>`. It is a
    /// pure identifier, never a filesystem path to read.
    #[must_use]
    pub fn logical_uri(&self, path: &str) -> String {
        format!("typeshed:{}/{}", self.identity, path)
    }

    /// The raw bytes of a file, if present.
    #[must_use]
    pub fn read(&self, path: &str) -> Option<&[u8]> {
        self.archive.get(path).map(|entry| entry.data.as_slice())
    }

    /// The UTF-8 text of a file, if present and valid UTF-8 (`.pyi` always is).
    #[must_use]
    pub fn read_str(&self, path: &str) -> Option<&str> {
        self.read(path)
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
    }

    /// Read a stable logical URI belonging to this exact VFS identity.
    #[must_use]
    pub fn read_uri(&self, uri: &str) -> Option<&str> {
        let prefix = format!("typeshed:{}/", self.identity);
        self.read_str(uri.strip_prefix(&prefix)?)
    }

    /// Iterate over every logical path in the archive.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.archive
            .entries()
            .iter()
            .map(|entry| entry.path.as_str())
    }

    /// The underlying archive.
    #[must_use]
    pub fn archive(&self) -> &Archive {
        &self.archive
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, data: &[u8]) -> ArchiveEntry {
        ArchiveEntry {
            path: path.to_owned(),
            mode: FileMode::Regular,
            data: data.to_vec(),
        }
    }

    fn sample() -> Archive {
        Archive::new(vec![
            entry("stdlib/os.pyi", b"def getcwd() -> str: ...\n"),
            entry("stdlib/VERSIONS", b"os: 3.0-\n"),
        ])
    }

    #[test]
    fn get_and_read_round_trip() {
        let archive = sample();
        assert_eq!(archive.len(), 2);
        assert!(!archive.is_empty());
        assert_eq!(
            archive
                .get("stdlib/os.pyi")
                .map(|entry| entry.data.as_slice()),
            Some(b"def getcwd() -> str: ...\n".as_slice())
        );
        assert!(archive.get("stdlib/missing.pyi").is_none());
    }

    #[test]
    fn total_data_len_sums_entries() {
        let archive = sample();
        let bytes = "def getcwd() -> str: ...\n".len() + "os: 3.0-\n".len();
        let expected = u64::try_from(bytes).unwrap_or(u64::MAX);
        assert_eq!(archive.total_data_len(), expected);
    }

    #[test]
    fn empty_archive_has_empty_tree_oid() {
        let archive = Archive::new(vec![]);
        assert!(archive.is_empty());
        assert_eq!(
            archive.root_tree_oid().map(|oid| oid.to_hex()),
            Ok("4b825dc642cb6eb9a060e54bf8d69288fbee4904".to_owned())
        );
    }

    #[test]
    fn vfs_exposes_logical_uri_and_bytes_not_paths() {
        let vfs = ArchiveVfs::new("83c2518", sample());
        assert_eq!(vfs.identity(), "83c2518");
        assert_eq!(
            vfs.logical_uri("stdlib/os.pyi"),
            "typeshed:83c2518/stdlib/os.pyi"
        );
        assert_eq!(
            vfs.read_str("stdlib/os.pyi"),
            Some("def getcwd() -> str: ...\n")
        );
        assert!(vfs.read("nope").is_none());
    }

    #[test]
    fn vfs_paths_lists_every_entry() {
        let vfs = ArchiveVfs::new("x", sample());
        let mut paths: Vec<&str> = vfs.paths().collect();
        paths.sort_unstable();
        assert_eq!(paths, vec!["stdlib/VERSIONS", "stdlib/os.pyi"]);
    }
}
