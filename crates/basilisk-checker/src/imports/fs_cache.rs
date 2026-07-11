//! Implements [ANALYSIS-CROSSLSP-IMPORT]. See docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-CROSSLSP-IMPORT
//! Per-resolution-pass directory-listing cache backing [`super::resolve`].
//!
//! Resolving one module name probes up to ~9 candidate paths per search
//! directory with `stat`; a module with many imports repeats those probes for
//! every import. [`FsCache`] reads each directory ONCE and answers every
//! `is_file`/`is_dir` probe from a hash set, turning O(imports × candidates)
//! syscalls into O(distinct directories) `read_dir` calls.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// One directory's entries, read once: file names and subdirectory names
/// (symlinks resolved to their target kind, matching `Path::is_file`/`is_dir`).
struct DirListing {
    files: HashSet<OsString>,
    dirs: HashSet<OsString>,
}

/// Per-resolution-pass cache of directory listings.
///
/// The cache lives for a single resolution pass (one call into the public API,
/// or one [`super::resolve_module_imports`] loop), so it can never serve stale
/// entries across checks.
///
/// Two deliberate points where answering probes from a `read_dir` listing
/// differs from probing each candidate path with `Path::is_file`/`is_dir`:
///
/// * **Name matching is case-SENSITIVE.** Entries are keyed by their exact
///   on-disk name, so `import Foo` will not resolve to `foo.py` even on a
///   case-insensitive filesystem (APFS/NTFS), where a per-path `Path::is_file`
///   would have matched. This is intended: Python import resolution is
///   case-sensitive (as is `CPython` on a case-sensitive FS, and Pyright), so a
///   case-mismatched import is a real portability bug worth surfacing rather
///   than masking on one platform.
/// * **A search directory must be READABLE.** Listing needs read permission on
///   the directory, whereas a per-path `stat` needed only traverse (`--x`)
///   permission on ancestors. An `--x`-only search directory — vanishingly rare
///   for a Python path — reads as empty. Accepted as the cost of turning
///   O(imports × candidates) `stat`s into O(directories) `read_dir`s.
pub(crate) struct FsCache {
    listings: RefCell<HashMap<PathBuf, Option<DirListing>>>,
}

impl FsCache {
    pub(crate) fn new() -> Self {
        Self {
            listings: RefCell::new(HashMap::new()),
        }
    }

    /// `Path::is_file(dir/name)` answered from the cached listing of `dir`.
    pub(crate) fn is_file(&self, path: &Path) -> bool {
        self.probe(path, |listing, name| listing.files.contains(name))
    }

    /// `Path::is_dir(dir/name)` answered from the cached listing of `dir`.
    pub(crate) fn is_dir(&self, path: &Path) -> bool {
        self.probe(path, |listing, name| listing.dirs.contains(name))
    }

    /// Test a file name in an already identified directory without building a
    /// candidate [`PathBuf`]. Import resolution uses this on its overwhelmingly
    /// common miss path.
    pub(crate) fn contains_file(&self, dir: &Path, name: &OsStr) -> bool {
        self.contains(dir, name, |listing, entry| listing.files.contains(entry))
    }

    /// Test a subdirectory name without allocating a joined path.
    pub(crate) fn contains_dir(&self, dir: &Path, name: &OsStr) -> bool {
        self.contains(dir, name, |listing, entry| listing.dirs.contains(entry))
    }

    fn probe(&self, path: &Path, hit: impl Fn(&DirListing, &OsStr) -> bool) -> bool {
        let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
            return false;
        };
        // A bare relative path ("foo.py") has the empty path as parent; read
        // the current directory, as `Path::is_file` would have.
        let parent = if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        };
        self.contains(parent, name, hit)
    }

    fn contains(
        &self,
        dir: &Path,
        name: &OsStr,
        hit: impl Fn(&DirListing, &OsStr) -> bool,
    ) -> bool {
        let mut listings = self.listings.borrow_mut();
        // Fast path: no allocation when the listing is already cached.
        if let Some(listing) = listings.get(dir) {
            return listing.as_ref().is_some_and(|l| hit(l, name));
        }
        let listing = read_listing(dir);
        let found = listing.as_ref().is_some_and(|l| hit(l, name));
        let _ = listings.insert(dir.to_path_buf(), listing);
        found
    }
}

/// Read a directory into a [`DirListing`], or `None` when unreadable/absent.
/// Symlinked entries are classified by their target (follow semantics), so
/// lookups agree with what `Path::is_file`/`is_dir` would have returned.
fn read_listing(dir: &Path) -> Option<DirListing> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut files = HashSet::new();
    let mut dirs = HashSet::new();
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let resolved_is_dir = if file_type.is_symlink() {
            match std::fs::metadata(entry.path()) {
                Ok(meta) => meta.is_dir(),
                Err(_) => continue, // broken symlink: neither file nor dir
            }
        } else {
            file_type.is_dir()
        };
        if resolved_is_dir {
            let _ = dirs.insert(entry.file_name());
        } else {
            let _ = files.insert(entry.file_name());
        }
    }
    Some(DirListing { files, dirs })
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test-only: expect acceptable in unit tests"
)]
mod fs_cache_tests {
    //! Unit tests for [`FsCache`] — the per-pass directory-listing cache that
    //! answers `is_file`/`is_dir` from one `read_dir` per directory. Covers the
    //! edge branches the whole-file import tests never hit: symlink-to-dir,
    //! broken symlinks, cached reuse, and the bare-relative-path (empty parent)
    //! fallback to the current directory.
    use super::FsCache;
    use std::path::Path;

    #[test]
    fn is_file_and_is_dir_answer_from_one_listing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::write(root.join("mod.py"), b"x = 1").expect("write file");
        std::fs::create_dir(root.join("pkg")).expect("mkdir");

        let fs = FsCache::new();
        assert!(fs.is_file(&root.join("mod.py")));
        assert!(!fs.is_dir(&root.join("mod.py")));
        assert!(fs.is_dir(&root.join("pkg")));
        assert!(!fs.is_file(&root.join("pkg")));
        assert!(!fs.is_file(&root.join("absent.py")));
        assert!(!fs.is_dir(&root.join("absent")));
        // Second probe of the same directory is served from the cache (no panic,
        // identical answer) — exercises the cached fast path.
        assert!(fs.is_file(&root.join("mod.py")));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_to_dir_is_a_dir_and_broken_symlink_is_neither() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::create_dir(root.join("real_pkg")).expect("mkdir");
        // Follow semantics: a symlink to a directory reports as a directory.
        std::os::unix::fs::symlink(root.join("real_pkg"), root.join("link_pkg"))
            .expect("symlink to dir");
        // A dangling symlink resolves to nothing: neither file nor dir.
        std::os::unix::fs::symlink(root.join("missing"), root.join("dangling"))
            .expect("broken symlink");

        let fs = FsCache::new();
        assert!(fs.is_dir(&root.join("link_pkg")));
        assert!(!fs.is_file(&root.join("dangling")));
        assert!(!fs.is_dir(&root.join("dangling")));
    }

    #[test]
    fn unreadable_directory_answers_false() {
        let fs = FsCache::new();
        // Parent directory does not exist → read_listing returns None → false.
        assert!(!fs.is_file(Path::new("/nonexistent-basilisk-xyz/mod.py")));
        assert!(!fs.is_dir(Path::new("/nonexistent-basilisk-xyz/pkg")));
    }

    #[test]
    fn bare_relative_path_probes_the_current_directory() {
        // A path with no directory component has an empty parent; the cache must
        // fall back to the current directory (as `Path::is_file` would). During
        // `cargo test` the cwd is the crate root, which always has `Cargo.toml`.
        let fs = FsCache::new();
        assert!(fs.is_file(Path::new("Cargo.toml")));
        assert!(!fs.is_file(Path::new("definitely-not-here.xyz")));
    }
}
