//! Implements [STUBRES-TYPESHED-LICENSE]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-LICENSE
//!
//! License-file identity manifest.
//!
//! The License gate compares the **set** of legal files an archive carries — each
//! `LICENSE*`/`NOTICE*` path mapped to its SHA-256 — against a build-approved
//! identity. Any addition, removal, or digest change is drift and blocks
//! activation for human review. The reviewed `python/typeshed@83c2518` identity
//! is a single root `LICENSE` (SHA-256 `295f8538…cabe`) with no `NOTICE` and no
//! nested legal files.

use std::collections::BTreeMap;

use sha2::{Digest as _, Sha256};

use crate::typeshed::archive::Archive;

/// The set of legal files (`LICENSE*`/`NOTICE*`) an archive carries, each mapped
/// to its SHA-256, used to detect license drift.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LicenseManifest {
    files: BTreeMap<String, String>,
}

impl LicenseManifest {
    /// Build a manifest from explicit `(path, sha256-hex)` pairs.
    #[must_use]
    pub fn from_pairs(pairs: &[(&str, &str)]) -> Self {
        let files = pairs
            .iter()
            .map(|(path, sha)| ((*path).to_owned(), (*sha).to_owned()))
            .collect();
        Self { files }
    }

    /// Discover the legal-file manifest present in an archive.
    #[must_use]
    pub fn discover(archive: &Archive) -> Self {
        let files = archive
            .entries()
            .iter()
            .filter(|entry| is_legal_file(&entry.path))
            .map(|entry| (entry.path.clone(), sha256_hex(&entry.data)))
            .collect();
        Self { files }
    }

    /// The `(path, sha256)` map.
    #[must_use]
    pub fn files(&self) -> &BTreeMap<String, String> {
        &self.files
    }

    /// Whether no legal files were found.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// Whether a path names a legal file (`LICENSE*`/`NOTICE*`, case-insensitive)
/// within the **relevant scope** — the archive root or under `stdlib/`. This
/// matches the bundle updater, so a full archive's unrelated `stubs/**` legal
/// files never register as drift. Public because the store writer and reader
/// share this exact rule for which paths a store entry materializes
/// ([STUBRES-TYPESHED-STORE]).
#[must_use]
pub fn is_legal_file(path: &str) -> bool {
    let relevant = !path.contains('/') || path.starts_with("stdlib/");
    if !relevant {
        return false;
    }
    let base = path.rsplit('/').next().unwrap_or(path).to_ascii_uppercase();
    base.starts_with("LICENSE") || base.starts_with("NOTICE")
}

/// SHA-256 of `data` as 64 lowercase hex characters.
#[must_use]
pub fn sha256_hex(data: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        // Writing to a `String` is infallible; the discard is deliberate.
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeshed::archive::ArchiveEntry;
    use crate::typeshed::gittree::FileMode;

    fn reg(path: &str, data: &[u8]) -> ArchiveEntry {
        ArchiveEntry {
            path: path.to_owned(),
            mode: FileMode::Regular,
            data: data.to_vec().into(),
        }
    }

    #[test]
    fn sha256_matches_known_vector() {
        // SHA-256("") — a fixed, externally verifiable vector.
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn only_license_and_notice_files_are_discovered() {
        let archive = Archive::new(vec![
            reg("LICENSE", b"composite\n"),
            reg("stdlib/NOTICE.txt", b"nested\n"),
            reg("stdlib/os.pyi", b"code\n"),
        ]);
        let manifest = LicenseManifest::discover(&archive);
        let mut names: Vec<&str> = manifest.files().keys().map(String::as_str).collect();
        names.sort_unstable();
        assert_eq!(names, vec!["LICENSE", "stdlib/NOTICE.txt"]);
    }

    #[test]
    fn identical_content_yields_identical_manifest() {
        let a = LicenseManifest::discover(&Archive::new(vec![reg("LICENSE", b"x\n")]));
        let b = LicenseManifest::from_pairs(&[("LICENSE", &sha256_hex(b"x\n"))]);
        assert_eq!(a, b);
        assert!(!a.is_empty());
    }
}
