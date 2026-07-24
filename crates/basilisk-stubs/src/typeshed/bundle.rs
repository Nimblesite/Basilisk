//! Implements [STUBRES-TYPESHED-BASELINE]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-BASELINE
//!
//! The bundled offline snapshot — Basilisk's step-3 floor.
//!
//! A release-pinned ZIP of typeshed's `stdlib/` (real `.pyi` bodies, `VERSIONS`,
//! and the composite `LICENSE`) is embedded with `include_bytes!`. Because the
//! bundle is a **stdlib subset**, it cannot reconstruct the full-repository Git
//! root tree, so it is verified by its **embedded ZIP SHA-256 plus the license
//! manifest**, never by tree reconstruction ([STUBRES-TYPESHED-BASELINE]). It
//! is the data an unset `typeshed-commit` resolves to; the selector reports
//! that default `typeshed_source_unpinned` (a build-time pin is not a user pin) and an explicit
//! pin of the same commit as pinned ([STUBRES-TYPESHED-WARN]).

use std::sync::OnceLock;

use serde::Deserialize;

use super::archive::ArchiveVfs;
use super::codec::{decode_zip, DecodeError, DecodeLimits, ZipLayout};
use super::gate::manifest::sha256_hex;
use super::gate::{
    license_gate, safety_gate, shape_gate, LicenseManifest, LicenseViolation, SafetyLimits,
    SafetyViolation, ShapeViolation,
};
use super::gittree::{Oid, OidParseError};
use super::snapshot::{Snapshot, SnapshotError};
use super::source::{LicenseStatus, SourceIdentity, SourceKind, TypeshedStatus};

/// The embedded bundle ZIP.
static BUNDLE_ZIP: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/data/typeshed/stdlib.zip"
));

/// The embedded bundle manifest.
static BUNDLE_MANIFEST_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/data/typeshed/manifest.json"
));

/// Identity-bound distribution index derived from the same full commit.
static BUNDLE_DISTRIBUTIONS_TSV: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/data/typeshed_stub_distributions.tsv"
));

/// A failure building the bundled snapshot from embedded assets.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BundleError {
    /// The embedded manifest JSON could not be parsed.
    #[error("bundle manifest parse error: {0}")]
    Manifest(String),
    /// The embedded ZIP's SHA-256 did not match the manifest.
    #[error("bundle digest mismatch: embedded {actual}, manifest {expected}")]
    DigestMismatch {
        /// The manifest-declared digest.
        expected: String,
        /// The embedded ZIP's actual digest.
        actual: String,
    },
    /// A manifest SHA field was not a valid object ID.
    #[error("bundle manifest has an invalid SHA: {0}")]
    BadSha(OidParseError),
    /// The ZIP could not be decoded.
    #[error("bundle decode: {0}")]
    Decode(DecodeError),
    /// The bundle failed the Safety gate.
    #[error("bundle safety: {0}")]
    Safety(SafetyViolation),
    /// The bundle failed the Shape gate.
    #[error("bundle shape: {0}")]
    Shape(ShapeViolation),
    /// The bundle failed the License gate.
    #[error("bundle license: {0}")]
    License(LicenseViolation),
    /// The identity-bound distribution sidecar digest did not match.
    #[error("bundle distribution digest mismatch: embedded {actual}, manifest {expected}")]
    DistributionDigestMismatch {
        /// Manifest digest.
        expected: String,
        /// Embedded sidecar digest.
        actual: String,
    },
    /// A derived snapshot view or `.pyi` body was invalid.
    #[error("bundle snapshot: {0}")]
    Snapshot(SnapshotError),
}

/// The subset of the manifest the runtime needs.
#[derive(Debug, Deserialize)]
struct BundleManifest {
    bundle: BundleSection,
    derived_indexes: DerivedIndexes,
    license_manifest: LicenseSection,
    source: SourceSection,
}

#[derive(Debug, Deserialize)]
struct DerivedIndexes {
    stub_distributions: StubDistributionSection,
}

#[derive(Debug, Deserialize)]
struct StubDistributionSection {
    sha256: String,
}

#[derive(Debug, Deserialize)]
struct BundleSection {
    sha256: String,
}

#[derive(Debug, Deserialize)]
struct LicenseSection {
    files: Vec<LegalFile>,
}

#[derive(Debug, Deserialize)]
struct LegalFile {
    path: String,
    sha256: String,
}

#[derive(Debug, Deserialize)]
struct SourceSection {
    commit_sha: String,
    tree_sha: String,
}

/// Build the bundled snapshot from the embedded assets, verifying the ZIP digest
/// and running the Safety, Shape, and License gates.
///
/// # Errors
///
/// Returns a [`BundleError`] if the manifest is invalid, the embedded ZIP digest
/// does not match, or any gate rejects the decoded archive.
pub fn bundled_snapshot() -> Result<Snapshot, BundleError> {
    static SNAPSHOT: OnceLock<Result<Snapshot, BundleError>> = OnceLock::new();
    SNAPSHOT.get_or_init(build_bundled_snapshot).clone()
}

fn build_bundled_snapshot() -> Result<Snapshot, BundleError> {
    let manifest: BundleManifest = serde_json::from_str(BUNDLE_MANIFEST_JSON)
        .map_err(|err| BundleError::Manifest(err.to_string()))?;

    let actual = sha256_hex(BUNDLE_ZIP);
    if actual != manifest.bundle.sha256 {
        return Err(BundleError::DigestMismatch {
            expected: manifest.bundle.sha256,
            actual,
        });
    }
    let distribution_actual = sha256_hex(BUNDLE_DISTRIBUTIONS_TSV.as_bytes());
    if distribution_actual != manifest.derived_indexes.stub_distributions.sha256 {
        return Err(BundleError::DistributionDigestMismatch {
            expected: manifest.derived_indexes.stub_distributions.sha256,
            actual: distribution_actual,
        });
    }

    // The bundle has no top-level prefix; entries are `stdlib/…` and `LICENSE`.
    let archive = decode_zip(
        BUNDLE_ZIP,
        ZipLayout::BundledRootless,
        &DecodeLimits::default(),
    )
    .map_err(BundleError::Decode)?;

    safety_gate(&archive, &SafetyLimits::default()).map_err(BundleError::Safety)?;
    shape_gate(&archive).map_err(BundleError::Shape)?;
    let approved = license_manifest_from_section(&manifest.license_manifest);
    license_gate(&archive, &approved).map_err(BundleError::License)?;

    let commit = Oid::from_hex(&manifest.source.commit_sha).map_err(BundleError::BadSha)?;
    let tree = Oid::from_hex(&manifest.source.tree_sha).map_err(BundleError::BadSha)?;
    let identity = SourceIdentity::Bundled { commit };
    // Warnings are set by the selector, which knows whether an explicit pin
    // selected this bundle (suppressing typeshed_source_unpinned) or the default did not.
    let status = TypeshedStatus {
        active_source: SourceKind::Bundled,
        commit: Some(commit),
        tree: Some(tree),
        license_status: LicenseStatus::Approved,
        license_reference: Some(license_reference(&manifest.source.commit_sha)),
        warnings: Vec::new(),
    };
    let vfs = ArchiveVfs::new(identity.uri_component(), archive);
    Snapshot::build(identity, status, vfs, Some(BUNDLE_DISTRIBUTIONS_TSV))
        .map_err(BundleError::Snapshot)
}

/// The bundle's build-time commit SHA, without decoding the whole archive.
#[must_use]
pub fn bundled_commit_sha() -> &'static str {
    "83c2518a9e6abbda0c44592c3483de459198f887"
}

/// Build the approved license manifest from the manifest's legal-file list.
/// Return the build-reviewed legal-file identity shared by bundle and runtime
/// download activation. This parses the one embedded sidecar; callers never
/// duplicate the approved LICENSE/NOTICE hashes.
///
/// # Errors
///
/// Returns [`BundleError::Manifest`] when the embedded sidecar is malformed.
pub fn approved_license_manifest() -> Result<LicenseManifest, BundleError> {
    let manifest: BundleManifest = serde_json::from_str(BUNDLE_MANIFEST_JSON)
        .map_err(|error| BundleError::Manifest(error.to_string()))?;
    Ok(license_manifest_from_section(&manifest.license_manifest))
}

fn license_manifest_from_section(section: &LicenseSection) -> LicenseManifest {
    let pairs: Vec<(&str, &str)> = section
        .files
        .iter()
        .map(|file| (file.path.as_str(), file.sha256.as_str()))
        .collect();
    LicenseManifest::from_pairs(&pairs)
}

/// The immutable, pinned LICENSE URL for a commit.
fn license_reference(commit_sha: &str) -> String {
    format!("https://github.com/python/typeshed/blob/{commit_sha}/LICENSE")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_snapshot_activates_from_the_real_asset() {
        let snapshot = bundled_snapshot();
        assert!(
            snapshot.is_ok(),
            "bundled snapshot must build: {:?}",
            snapshot.as_ref().err()
        );
        let Ok(snapshot) = snapshot else {
            return;
        };
        // Identity + status honesty.
        assert_eq!(snapshot.status.active_source, SourceKind::Bundled);
        assert_eq!(snapshot.status.license_status, LicenseStatus::Approved);
        // Warnings belong to the selector, which knows whether an explicit pin
        // chose this bundle; the raw bundle carries none of its own.
        assert!(snapshot.status.warnings.is_empty());
        assert_eq!(
            snapshot.status.commit.map(|oid| oid.to_hex()).as_deref(),
            Some(bundled_commit_sha())
        );
    }

    #[test]
    fn bundled_snapshot_supplies_real_pyi_bodies_offline() {
        let Ok(snapshot) = bundled_snapshot() else {
            return;
        };
        // Real bodies, not names-only: `os` resolves and carries a signature.
        let os = snapshot.read_stub("os");
        assert!(os.is_some(), "os.pyi must be present in the bundle");
        assert!(snapshot
            .versions()
            .is_some_and(|versions| versions.contains("os")));
        // A representative module for #288/#289 offline coverage.
        assert!(snapshot.read_stub("unittest.mock").is_some());
    }
}
