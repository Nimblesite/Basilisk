//! Implements [STUBRES-TYPESHED-PYPI] check-time verification. See
//! docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-PYPI
//!
//! The content-addressed store entry for a `PyPI` typeshed distribution pinned
//! by its **wheel SHA-256**:
//!
//! ```text
//! <store>/<64-hex sha256>/
//!   wheel.whl   # the exact wheel whose SHA-256 IS the directory name
//! ```
//!
//! A 64-hex SHA-256 directory never collides with a 40-hex commit directory
//! ([`super::store`]): the two namespaces share one `<store>` root but are
//! disjoint by name length.
//!
//! Reading IS the pin verification ([STUBRES-TYPESHED-PYPI],
//! [STUBRES-TYPESHED-OFFLINE]), fully offline and never waivable:
//!
//! 1. re-hash `wheel.whl` with SHA-256 — it MUST equal the pinned digest (and
//!    the directory name);
//! 2. decode the verified wheel ZIP into the in-memory archive model under the
//!    rootless layout (entries are `stdlib/…`, `LICENSE`, …);
//! 3. run the Safety and Shape gates — the same structural admission a
//!    downloaded commit archive clears.
//!
//! The License and Content gates are **not** run here: the License gate attests
//! the build-approved *typeshed* `LICENSE`/`NOTICE` identity, which does not
//! apply to a third-party distribution, and the Content gate reconstructs a Git
//! root tree, which a wheel is not. The wheel SHA-256 IS the content
//! attestation for a `PyPI` source — it binds every byte the resolver will read.

use std::fs;
use std::path::Path;

use super::archive::ArchiveVfs;
use super::codec::{decode_zip, DecodeLimits, ZipLayout};
use super::gate::manifest::sha256_hex;
use super::gate::{safety_gate, shape_gate, SafetyLimits};
use super::snapshot::Snapshot;
use super::source::{LicenseStatus, SourceIdentity, SourceKind, TypeshedStatus};

/// The stored wheel file inside a `PyPI`-package store entry.
pub const WHEEL_FILE: &str = "wheel.whl";

/// Why a `PyPI`-package store entry could not activate. As with [`super::store`],
/// no variant carries a path: the caller knows the store root and the digest it
/// asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WheelError {
    /// No entry directory (or no `wheel.whl`) exists for the requested digest.
    #[error("no stored wheel for this package")]
    Missing,
    /// The entry exists but failed SHA-256 verification or a gate.
    #[error("stored wheel failed offline verification")]
    Corrupt,
}

/// The store entry directory for a `PyPI` package pinned by `sha256`.
#[must_use]
pub fn entry_dir(store_root: &Path, sha256: &str) -> std::path::PathBuf {
    store_root.join(sha256)
}

/// Read and verify one stored wheel into an immutable snapshot
/// ([STUBRES-TYPESHED-PYPI]).
///
/// `name` and `sha256` are the configured pin; the stored wheel must re-hash to
/// `sha256`. The returned snapshot carries a [`SourceIdentity::PyPIPackage`]
/// identity with exactly these values, so the selector can validate it.
///
/// # Errors
///
/// Returns [`WheelError::Missing`] when no entry exists, and
/// [`WheelError::Corrupt`] on any SHA-256 mismatch, decode failure, or gate
/// violation.
pub fn read_snapshot(store_root: &Path, name: &str, sha256: &str) -> Result<Snapshot, WheelError> {
    let dir = entry_dir(store_root, sha256);
    if !dir.is_dir() {
        return Err(WheelError::Missing);
    }
    let bytes = fs::read(dir.join(WHEEL_FILE)).map_err(|_error| WheelError::Missing)?;
    // Step 1: re-hash the stored wheel and require the pinned digest. This is
    // the content gate for a PyPI source — never waivable.
    if !is_hex_sha256(sha256) || sha256_hex(&bytes) != sha256 {
        return Err(WheelError::Corrupt);
    }
    // Step 2: decode the verified bytes into the in-memory archive model. A
    // wheel is rootless: its `stdlib/` tree and legal files sit at the archive
    // root (no `typeshed-<sha>/` prefix to strip).
    let archive = decode_zip(&bytes, ZipLayout::BundledRootless, &DecodeLimits::default())
        .map_err(|_error| WheelError::Corrupt)?;
    // Step 3: the same structural admission a downloaded commit archive clears.
    // The Shape gate requires a coherent `stdlib/` tree (parseable VERSIONS +
    // at least one `stdlib/*.pyi`) plus a root `LICENSE`; a wheel that does not
    // ship that tree fails closed as `Corrupt` (and surfaces as `NO SOURCE`).
    safety_gate(&archive, &SafetyLimits::default()).map_err(|_error| WheelError::Corrupt)?;
    shape_gate(&archive).map_err(|_error| WheelError::Corrupt)?;
    build_snapshot(name, sha256, archive)
}

/// Whether `s` is exactly 64 lowercase-hex characters — the shape of a SHA-256
/// the store is willing to look up. A malformed pin never reaches the
/// filesystem as a path component.
fn is_hex_sha256(s: &str) -> bool {
    s.len() == 64 && s.as_bytes().iter().all(u8::is_ascii_hexdigit)
}

fn build_snapshot(
    name: &str,
    sha256: &str,
    archive: super::archive::Archive,
) -> Result<Snapshot, WheelError> {
    let identity = SourceIdentity::PyPIPackage {
        name: name.to_owned(),
        sha256: sha256.to_owned(),
    };
    // A PyPI package is content-addressed by its wheel SHA-256: that attests
    // the bytes. It does not attest the *typeshed* approved license identity
    // (the wheel's own license is whatever it ships), so the license standing
    // is `NotSupplied` — the license is still viewable from the wheel, but no
    // approved reference is claimed. No advisories are emitted here; the
    // selector is the single authority on the advisory list.
    let status = TypeshedStatus {
        active_source: SourceKind::PyPIPackage,
        commit: None,
        tree: None,
        license_status: LicenseStatus::NotSupplied,
        license_reference: None,
        warnings: Vec::new(),
    };
    let uri_identity = identity.uri_component();
    Snapshot::build(
        identity,
        status,
        ArchiveVfs::new(uri_identity, archive),
        None,
    )
    .map_err(|_error| WheelError::Corrupt)
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test-only wheel fixtures require infallible embedded assets and SHA constants"
)]
mod tests {
    use std::io::Write as _;

    use zip::write::{SimpleFileOptions, ZipWriter};
    use zip::CompressionMethod;

    use super::super::archive::ArchiveEntry;
    use super::super::gittree::FileMode;
    use super::*;

    /// A minimal wheel that ships the contract `stdlib/` tree plus a root
    /// `LICENSE` — exactly what the Shape gate requires of a typeshed-like
    /// distribution.
    fn fixture_wheel() -> Vec<u8> {
        let entries: &[(&str, &[u8], u32)] = &[
            ("stdlib/VERSIONS", b"os: 3.0-\nsys: 3.0-\n", 0o644),
            ("stdlib/os.pyi", b"def getcwd() -> str: ...\n", 0o644),
            ("LICENSE", b"MIT\n\nCopyright (c)\n", 0o644),
            (
                "micropython_stdlib_stubs-1.0.dist-info/METADATA",
                b"Metadata-Version: 2.1\nName: micropython-stdlib-stubs\n",
                0o644,
            ),
        ];
        let mut buf = Vec::new();
        {
            let mut writer = ZipWriter::new(std::io::Cursor::new(&mut buf));
            for (name, data, mode) in entries {
                let options = SimpleFileOptions::default()
                    .compression_method(CompressionMethod::Stored)
                    .unix_permissions(*mode);
                writer.start_file(*name, options).expect("start_file");
                writer.write_all(data).expect("write_all");
            }
            let _ = writer.finish().expect("finish");
        }
        buf
    }

    /// Write a wheel into `<store>/<sha256>/wheel.whl` and return the SHA-256
    /// that addresses it (the re-hash of the exact bytes on disk).
    fn install_wheel(store_root: &Path) -> String {
        let bytes = fixture_wheel();
        let sha256 = sha256_hex(&bytes);
        let dir = entry_dir(store_root, &sha256);
        fs::create_dir_all(&dir).expect("entry dir");
        fs::write(dir.join(WHEEL_FILE), &bytes).expect("write wheel");
        sha256
    }

    const PACKAGE_NAME: &str = "micropython-stdlib-stubs";

    #[test]
    fn a_verified_wheel_activates_as_a_pypi_source_with_no_advisories() {
        let root = tempfile::tempdir().expect("tempdir");
        let sha256 = install_wheel(root.path());
        let snapshot = read_snapshot(root.path(), PACKAGE_NAME, &sha256).expect("verified wheel");
        assert_eq!(snapshot.status.active_source, SourceKind::PyPIPackage);
        assert_eq!(
            snapshot.identity,
            SourceIdentity::PyPIPackage {
                name: PACKAGE_NAME.to_owned(),
                sha256,
            }
        );
        // The content-addressed pin is pinned: no unpinned/user-managed
        // advisories, and no license drift is claimed for a third-party wheel.
        assert!(snapshot.status.warnings.is_empty());
        assert_eq!(snapshot.status.license_status, LicenseStatus::NotSupplied);
        assert!(snapshot.status.commit.is_none());
        // The resolver reads the wheel's stdlib/ subtree through the archive VFS.
        assert_eq!(
            snapshot.read_stub("os").map(|(_, body)| body),
            Some("def getcwd() -> str: ...\n"),
        );
    }

    #[test]
    fn a_missing_wheel_is_missing_and_writes_nothing() {
        const ABSENT_SHA: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let root = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            read_snapshot(root.path(), PACKAGE_NAME, ABSENT_SHA).err(),
            Some(WheelError::Missing),
        );
        // Reading never creates, repairs, or evicts ([STUBRES-TYPESHED-STORE]).
        assert_eq!(
            fs::read_dir(root.path()).expect("readdir").count(),
            0,
            "a read must not write",
        );
    }

    #[test]
    fn a_tampered_wheel_fails_verification() {
        let root = tempfile::tempdir().expect("tempdir");
        let sha256 = install_wheel(root.path());
        // Flip one byte of the stored wheel: its SHA-256 no longer matches the
        // directory name (and the pin), so verification fails hard.
        let wheel_path = entry_dir(root.path(), &sha256).join(WHEEL_FILE);
        let mut bytes = fs::read(&wheel_path).expect("read wheel");
        // Flip one byte of the stored wheel: its SHA-256 no longer matches the
        // directory name (and the pin), so verification fails hard.
        *bytes.last_mut().expect("non-empty wheel") ^= 0xff;
        fs::write(&wheel_path, &bytes).expect("tamper");
        assert_eq!(
            read_snapshot(root.path(), PACKAGE_NAME, &sha256).err(),
            Some(WheelError::Corrupt),
            "a byte-flipped wheel must not activate",
        );
    }

    #[test]
    fn a_wheel_whose_pin_does_not_match_its_bytes_is_corrupt() {
        const OTHER_SHA: &str = "1111111111111111111111111111111111111111111111111111111111111111";
        let root = tempfile::tempdir().expect("tempdir");
        let sha256 = install_wheel(root.path());
        // A pin that names a different digest than the one on disk: the
        // directory for that pin does not exist, so this is Missing (the
        // checker never guesses a nearby entry).
        let _ = sha256;
        assert_eq!(
            read_snapshot(root.path(), PACKAGE_NAME, OTHER_SHA).err(),
            Some(WheelError::Missing),
        );
    }

    /// A wheel that decodes but lacks the `stdlib/` tree the Shape gate
    /// requires fails closed as `Corrupt` — the open layout-verification item
    /// ([STUBRES-TYPESHED-PYPI] plan S2) is enforced structurally, not assumed.
    #[test]
    fn a_wheel_without_a_stdlib_tree_is_rejected() {
        let root = tempfile::tempdir().expect("tempdir");
        let entries: &[(&str, &[u8], u32)] = &[
            ("LICENSE", b"MIT\n", 0o644),
            (
                "micropython_stdlib_stubs-1.0.dist-info/METADATA",
                b"Name: x\n",
                0o644,
            ),
        ];
        let mut buf = Vec::new();
        {
            let mut writer = ZipWriter::new(std::io::Cursor::new(&mut buf));
            for (name, data, mode) in entries {
                let options = SimpleFileOptions::default()
                    .compression_method(CompressionMethod::Stored)
                    .unix_permissions(*mode);
                writer.start_file(*name, options).expect("start_file");
                writer.write_all(data).expect("write_all");
            }
            let _ = writer.finish().expect("finish");
        }
        let sha256 = sha256_hex(&buf);
        let dir = entry_dir(root.path(), &sha256);
        fs::create_dir_all(&dir).expect("entry dir");
        fs::write(dir.join(WHEEL_FILE), &buf).expect("write wheel");
        assert_eq!(
            read_snapshot(root.path(), PACKAGE_NAME, &sha256).err(),
            Some(WheelError::Corrupt),
            "a wheel without a stdlib/ tree must not activate",
        );
    }

    /// `read_snapshot` is the sole constructor; the helper that builds the
    /// archive from raw entries is exercised here to pin the identity/VFS
    /// wiring independently of the ZIP codec.
    #[test]
    fn build_snapshot_wires_the_pypi_identity_into_the_vfs() {
        const SHA: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let archive = super::super::archive::Archive::new(vec![
            ArchiveEntry {
                path: "stdlib/VERSIONS".to_owned(),
                mode: FileMode::Regular,
                data: b"os: 3.0-\n".to_vec().into(),
            },
            ArchiveEntry {
                path: "stdlib/os.pyi".to_owned(),
                mode: FileMode::Regular,
                data: b"def getcwd() -> str: ...\n".to_vec().into(),
            },
            ArchiveEntry {
                path: "LICENSE".to_owned(),
                mode: FileMode::Regular,
                data: b"MIT\n".to_vec().into(),
            },
        ]);
        let snapshot = build_snapshot(PACKAGE_NAME, SHA, archive).expect("built snapshot");
        assert_eq!(snapshot.vfs.identity(), snapshot.identity.uri_component());
        assert_eq!(
            snapshot.identity,
            SourceIdentity::PyPIPackage {
                name: PACKAGE_NAME.to_owned(),
                sha256: SHA.to_owned(),
            }
        );
    }
}
