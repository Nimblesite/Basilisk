//! Implements [STUBRES-TYPESHED-DOWNLOAD] activation gates. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-DOWNLOAD
//!
//! The four activation gates.
//!
//! An archive is admitted only after it clears, in order:
//!
//! 1. **Safety** — reject absolute/`..` paths, escaping symlinks, duplicate
//!    entries, and entry/total-size and entry-count limits (zip-bomb defence).
//! 2. **Shape** — require one coherent stdlib tree with a parseable
//!    `stdlib/VERSIONS`, at least one `stdlib/*.pyi`, and root license metadata.
//! 3. **License** — the discovered `LICENSE*`/`NOTICE*` path+SHA-256 set must
//!    exactly match a build-approved identity ([STUBRES-TYPESHED-LICENSE]).
//! 4. **Content** — reconstruct the Git root tree and match the trusted
//!    root-tree SHA. No gate can be waived ([STUBRES-TYPESHED-PIN]).

pub mod errors;
pub mod manifest;

pub use errors::{ContentViolation, GateError, LicenseViolation, SafetyViolation, ShapeViolation};
pub use manifest::LicenseManifest;

use std::collections::HashSet;

use super::archive::{Archive, ArchiveVfs};
use super::gittree::{FileMode, Oid};

/// Byte and count limits enforced by the Safety gate.
#[derive(Debug, Clone, Copy)]
pub struct SafetyLimits {
    /// Maximum number of archive entries.
    pub max_entries: usize,
    /// Maximum decompressed size of any single entry, in bytes.
    pub max_entry_bytes: u64,
    /// Maximum decompressed size across all entries, in bytes.
    pub max_total_bytes: u64,
}

impl Default for SafetyLimits {
    fn default() -> Self {
        Self {
            max_entries: 200_000,
            max_entry_bytes: 64 * 1024 * 1024,
            max_total_bytes: 512 * 1024 * 1024,
        }
    }
}

/// Run the **Safety** gate.
///
/// # Errors
///
/// Returns a [`SafetyViolation`] on the first unsafe entry or breached limit.
pub fn safety_gate(archive: &Archive, limits: &SafetyLimits) -> Result<(), SafetyViolation> {
    let entries = archive.entries();
    if entries.len() > limits.max_entries {
        return Err(SafetyViolation::TooManyEntries {
            count: entries.len(),
            limit: limits.max_entries,
        });
    }
    let mut seen: HashSet<&str> = HashSet::with_capacity(entries.len());
    for entry in entries {
        check_path_safe(&entry.path)?;
        let size = u64::try_from(entry.data.len()).unwrap_or(u64::MAX);
        if size > limits.max_entry_bytes {
            return Err(SafetyViolation::EntryTooLarge {
                path: entry.path.clone(),
                size,
                limit: limits.max_entry_bytes,
            });
        }
        match entry.mode {
            FileMode::Symlink => {
                return Err(SafetyViolation::DisallowedSymlink(entry.path.clone()))
            }
            FileMode::Submodule => {
                return Err(SafetyViolation::DisallowedSubmodule(entry.path.clone()))
            }
            FileMode::Regular | FileMode::Executable => {}
        }
        if !seen.insert(entry.path.as_str()) {
            return Err(SafetyViolation::DuplicatePath(entry.path.clone()));
        }
    }
    let total = archive.total_data_len();
    if total > limits.max_total_bytes {
        return Err(SafetyViolation::TotalTooLarge {
            size: total,
            limit: limits.max_total_bytes,
        });
    }
    Ok(())
}

/// Validate a single archive path for traversal safety.
fn check_path_safe(path: &str) -> Result<(), SafetyViolation> {
    if path.is_empty() {
        return Err(SafetyViolation::EmptyPath);
    }
    if path.starts_with('/') || is_windows_absolute(path) {
        return Err(SafetyViolation::AbsolutePath(path.to_owned()));
    }
    if path.contains('\\') || path.contains('\0') {
        return Err(SafetyViolation::MalformedPath(path.to_owned()));
    }
    for segment in path.split('/') {
        match segment {
            // Empty segment => `a//b` or a trailing `/`; a normalized alias.
            "" => return Err(SafetyViolation::MalformedPath(path.to_owned())),
            ".." => return Err(SafetyViolation::ParentTraversal(path.to_owned())),
            "." => return Err(SafetyViolation::CurrentDirSegment(path.to_owned())),
            _ => {}
        }
    }
    Ok(())
}

/// Whether a path begins with a Windows drive prefix (e.g. `C:`).
fn is_windows_absolute(path: &str) -> bool {
    let mut chars = path.chars();
    matches!(
        (chars.next(), chars.next()),
        (Some(drive), Some(':')) if drive.is_ascii_alphabetic()
    )
}

/// Whether a path names a `.pyi` stub. The extension is compared
/// **case-sensitively**: only lowercase `.pyi` is a usable stub, so a `.PYI`
/// entry must not pass Shape and then supply nothing to the resolver.
fn is_pyi(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        == Some("pyi")
}

/// Run the **Shape** gate.
///
/// # Errors
///
/// Returns a [`ShapeViolation`] if the stdlib tree, `VERSIONS`, or license
/// metadata is missing or malformed.
pub fn shape_gate(archive: &Archive) -> Result<(), ShapeViolation> {
    let versions = archive
        .get("stdlib/VERSIONS")
        .ok_or(ShapeViolation::MissingVersions)?;
    let text = std::str::from_utf8(&versions.data)
        .map_err(|err| ShapeViolation::MalformedVersions(err.to_string()))?;
    validate_versions(text)?;
    let has_stub = archive
        .entries()
        .iter()
        .any(|entry| entry.path.starts_with("stdlib/") && is_pyi(&entry.path));
    if !has_stub {
        return Err(ShapeViolation::NoStdlibStubs);
    }
    if archive.get("LICENSE").is_none() {
        return Err(ShapeViolation::MissingLicense);
    }
    Ok(())
}

/// Lightly validate `stdlib/VERSIONS`: every non-comment line is `name: range`.
fn validate_versions(text: &str) -> Result<(), ShapeViolation> {
    if text.trim().is_empty() {
        return Err(ShapeViolation::EmptyVersions);
    }
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match line.split_once(':') {
            Some((name, _)) if !name.trim().is_empty() => {}
            _ => return Err(ShapeViolation::MalformedVersions(line.to_owned())),
        }
    }
    Ok(())
}

/// Run the **License** gate.
///
/// # Errors
///
/// Returns a [`LicenseViolation`] if the discovered legal-file identity does not
/// exactly match the approved manifest.
pub fn license_gate(archive: &Archive, approved: &LicenseManifest) -> Result<(), LicenseViolation> {
    let found = LicenseManifest::discover(archive);
    if found == *approved {
        Ok(())
    } else {
        Err(LicenseViolation {
            approved: approved.clone(),
            found,
        })
    }
}

/// Run the **Content** gate, returning the verified root-tree SHA.
///
/// # Errors
///
/// Returns a [`ContentViolation`] if the tree cannot be reconstructed or does
/// not match the trusted SHA.
pub fn content_gate(archive: &Archive, expected_root: &Oid) -> Result<Oid, ContentViolation> {
    let actual = archive.root_tree_oid().map_err(ContentViolation::Tree)?;
    if actual == *expected_root {
        Ok(actual)
    } else {
        Err(ContentViolation::TreeMismatch {
            expected: *expected_root,
            actual,
        })
    }
}

/// Everything the gates need to admit an archive.
#[derive(Debug, Clone)]
pub struct GateConfig {
    /// Safety limits.
    pub limits: SafetyLimits,
    /// The build-approved legal-file identity.
    pub approved_license: LicenseManifest,
    /// The trusted root-tree SHA to attest against.
    pub expected_root_tree: Oid,
}

/// A successfully activated archive: its VFS and the verified tree SHA.
#[derive(Debug, Clone)]
pub struct Activation {
    /// The archive VFS the resolver reads through.
    pub vfs: ArchiveVfs,
    /// The verified root-tree SHA.
    pub root_tree: Oid,
}

/// Run all four gates in order and produce an [`Activation`].
///
/// Every gate always runs. There is no waiver: a pin you can switch off is a
/// pin that does nothing ([STUBRES-TYPESHED-PIN]).
///
/// # Errors
///
/// Returns the first [`GateError`] encountered.
pub fn run_activation(
    archive: Archive,
    config: &GateConfig,
    identity: impl Into<String>,
) -> Result<Activation, GateError> {
    safety_gate(&archive, &config.limits).map_err(GateError::Safety)?;
    shape_gate(&archive).map_err(GateError::Shape)?;
    license_gate(&archive, &config.approved_license).map_err(GateError::License)?;
    let root_tree =
        content_gate(&archive, &config.expected_root_tree).map_err(GateError::Content)?;
    Ok(Activation {
        vfs: ArchiveVfs::new(identity, archive),
        root_tree,
    })
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only: unwrap acceptable in unit tests"
)]
mod tests {
    use super::manifest::sha256_hex;
    use super::*;
    use crate::typeshed::archive::ArchiveEntry;

    fn reg(path: &str, data: &[u8]) -> ArchiveEntry {
        ArchiveEntry {
            path: path.to_owned(),
            mode: FileMode::Regular,
            data: data.to_vec(),
        }
    }

    fn valid_entries() -> Vec<ArchiveEntry> {
        vec![
            reg("LICENSE", b"Apache-2.0 + MIT composite\n"),
            reg("stdlib/VERSIONS", b"os: 3.0-\nsys: 3.0-\n"),
            reg("stdlib/os.pyi", b"def getcwd() -> str: ...\n"),
        ]
    }

    fn config_for(archive: &Archive) -> GateConfig {
        GateConfig {
            limits: SafetyLimits::default(),
            approved_license: LicenseManifest::discover(archive),
            expected_root_tree: archive.root_tree_oid().unwrap(),
        }
    }

    #[test]
    fn valid_archive_activates_and_verifies() {
        let archive = Archive::new(valid_entries());
        let config = config_for(&archive);
        let root = config.expected_root_tree;
        let activation = run_activation(archive, &config, "83c2518").unwrap();
        assert_eq!(activation.root_tree, root);
        assert_eq!(activation.vfs.identity(), "83c2518");
    }

    /// Content verification always runs — there is no waiver field, flag, or
    /// mode; a mutated tree is terminal ([STUBRES-TYPESHED-PIN] no-waiver).
    #[test]
    fn content_verification_cannot_be_waived() {
        let archive = Archive::new(valid_entries());
        let mut config = config_for(&archive);
        config.expected_root_tree = crate::typeshed::gittree::git_blob_oid(b"not the tree");
        assert!(matches!(
            run_activation(archive, &config, "83c2518"),
            Err(GateError::Content(ContentViolation::TreeMismatch { .. }))
        ));
    }

    #[test]
    fn safety_rejects_traversal_absolute_and_dupes() {
        let limits = SafetyLimits::default();
        assert_eq!(
            safety_gate(&Archive::new(vec![reg("../evil", b"x")]), &limits),
            Err(SafetyViolation::ParentTraversal("../evil".to_owned()))
        );
        assert_eq!(
            safety_gate(&Archive::new(vec![reg("/etc/passwd", b"x")]), &limits),
            Err(SafetyViolation::AbsolutePath("/etc/passwd".to_owned()))
        );
        assert!(matches!(
            safety_gate(
                &Archive::new(vec![reg("stdlib/os.pyi", b"a"), reg("stdlib/os.pyi", b"b")]),
                &limits
            ),
            Err(SafetyViolation::DuplicatePath(_))
        ));
    }

    #[test]
    fn safety_rejects_all_links_submodules_and_over_count() {
        let mut link = reg("stdlib/link.pyi", b"posix.pyi");
        link.mode = FileMode::Symlink;
        assert!(matches!(
            safety_gate(&Archive::new(vec![link]), &SafetyLimits::default()),
            Err(SafetyViolation::DisallowedSymlink(_))
        ));
        let mut sub = reg("vendor", b"");
        sub.mode = FileMode::Submodule;
        assert!(matches!(
            safety_gate(&Archive::new(vec![sub]), &SafetyLimits::default()),
            Err(SafetyViolation::DisallowedSubmodule(_))
        ));
        let tight = SafetyLimits {
            max_entries: 1,
            ..SafetyLimits::default()
        };
        assert!(matches!(
            safety_gate(&Archive::new(vec![reg("a", b"x"), reg("b", b"y")]), &tight),
            Err(SafetyViolation::TooManyEntries { .. })
        ));
    }

    #[test]
    fn safety_rejects_malformed_paths() {
        let limits = SafetyLimits::default();
        assert_eq!(
            safety_gate(&Archive::new(vec![reg("a//b", b"x")]), &limits),
            Err(SafetyViolation::MalformedPath("a//b".to_owned()))
        );
        assert_eq!(
            safety_gate(&Archive::new(vec![reg("dir\\file", b"x")]), &limits),
            Err(SafetyViolation::MalformedPath("dir\\file".to_owned()))
        );
        assert_eq!(
            safety_gate(&Archive::new(vec![reg("C:/x", b"x")]), &limits),
            Err(SafetyViolation::AbsolutePath("C:/x".to_owned()))
        );
    }

    #[test]
    fn shape_requires_versions_stubs_and_license() {
        assert_eq!(
            shape_gate(&Archive::new(vec![reg("LICENSE", b"x")])),
            Err(ShapeViolation::MissingVersions)
        );
        assert_eq!(
            shape_gate(&Archive::new(vec![
                reg("stdlib/VERSIONS", b"os: 3.0-\n"),
                reg("LICENSE", b"x"),
            ])),
            Err(ShapeViolation::NoStdlibStubs)
        );
        assert_eq!(
            shape_gate(&Archive::new(vec![
                reg("stdlib/VERSIONS", b"os: 3.0-\n"),
                reg("stdlib/os.pyi", b"...\n"),
            ])),
            Err(ShapeViolation::MissingLicense)
        );
        assert_eq!(shape_gate(&Archive::new(valid_entries())), Ok(()));
    }

    #[test]
    fn shape_rejects_malformed_versions() {
        assert!(matches!(
            shape_gate(&Archive::new(vec![
                reg("stdlib/VERSIONS", b"this line has no colon\n"),
                reg("stdlib/os.pyi", b"...\n"),
                reg("LICENSE", b"x"),
            ])),
            Err(ShapeViolation::MalformedVersions(_))
        ));
    }

    #[test]
    fn license_gate_detects_add_remove_and_change() {
        let approved = LicenseManifest::from_pairs(&[("LICENSE", &sha256_hex(b"the license\n"))]);
        assert_eq!(
            license_gate(
                &Archive::new(vec![reg("LICENSE", b"the license\n")]),
                &approved
            ),
            Ok(())
        );
        assert!(license_gate(
            &Archive::new(vec![reg("LICENSE", b"tampered\n")]),
            &approved
        )
        .is_err());
        assert!(license_gate(
            &Archive::new(vec![
                reg("LICENSE", b"the license\n"),
                reg("NOTICE", b"x\n")
            ]),
            &approved
        )
        .is_err());
        assert!(license_gate(
            &Archive::new(vec![reg("stdlib/os.pyi", b"...\n")]),
            &approved
        )
        .is_err());
    }

    #[test]
    fn content_gate_matches_and_detects_mutation() {
        let archive = Archive::new(valid_entries());
        let root = archive.root_tree_oid().unwrap();
        assert_eq!(content_gate(&archive, &root), Ok(root));
        let mutated = Archive::new(vec![
            reg("LICENSE", b"Apache-2.0 + MIT composite\n"),
            reg("stdlib/VERSIONS", b"os: 3.0-\nsys: 3.0-\n"),
            reg("stdlib/os.pyi", b"def getcwd() -> bytes: ...\n"),
        ]);
        assert!(matches!(
            content_gate(&mutated, &root),
            Err(ContentViolation::TreeMismatch { .. })
        ));
    }
}
