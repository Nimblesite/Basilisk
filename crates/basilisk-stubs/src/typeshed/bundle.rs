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
use super::codec::{decode_zip, decode_zip_static, DecodeError, DecodeLimits, ZipLayout};
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

/// Build the bundled snapshot from the embedded assets.
///
/// The assets are `include_bytes!`/`include_str!` constants — the SAME bytes
/// every process start — so their digest and gate verification is a BUILD
/// invariant enforced in CI ([`verify_bundled_assets`] under test), not a
/// per-process runtime cost: re-hashing an immutable 3 MB constant on every
/// short-lived `check` taxes cold start for information the build already
/// proved. Runtime-acquired sources (pinned downloads, custom trees) are
/// mutable inputs and keep their full runtime verification.
///
/// # Errors
///
/// Returns a [`BundleError`] if the manifest is invalid or the embedded ZIP
/// cannot be decoded into a snapshot.
pub fn bundled_snapshot() -> Result<Snapshot, BundleError> {
    static SNAPSHOT: OnceLock<Result<Snapshot, BundleError>> = OnceLock::new();
    SNAPSHOT.get_or_init(build_bundled_snapshot).clone()
}

/// Verify the embedded assets against their manifest: ZIP + sidecar digests,
/// then the Safety, Shape, and License gates over the decoded archive.
///
/// This is the build invariant behind [`bundled_snapshot`]'s trust in the
/// embedded bytes; it runs in CI (see
/// `bundled_assets_match_manifest_and_pass_all_gates`), never on the
/// per-process activation path.
///
/// # Errors
///
/// Returns a [`BundleError`] naming the first mismatched digest or violated
/// gate.
pub fn verify_bundled_assets() -> Result<(), BundleError> {
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
    let archive = decode_zip(
        BUNDLE_ZIP,
        ZipLayout::BundledRootless,
        &DecodeLimits::default(),
    )
    .map_err(BundleError::Decode)?;
    safety_gate(&archive, &SafetyLimits::default()).map_err(BundleError::Safety)?;
    shape_gate(&archive).map_err(BundleError::Shape)?;
    let approved = license_manifest_from_section(&manifest.license_manifest);
    license_gate(&archive, &approved).map_err(BundleError::License)
}

/// The status a pin of the bundled commit reports, built from manifest
/// metadata alone — NO archive decode. `explicit` mirrors the selector's pin
/// policy: an explicit `typeshed-commit` suppresses
/// `typeshed_source_unpinned`; the bundled default keeps it
/// ([STUBRES-TYPESHED-WARN]).
///
/// This exists so the CLI can print the status banner while the snapshot
/// itself resolves on a background thread; a unit test pins it equal to the
/// selector-produced status so the two can never drift.
///
/// # Errors
///
/// Returns a [`BundleError`] if the embedded manifest is invalid.
pub fn bundled_pinned_status(explicit: bool) -> Result<TypeshedStatus, BundleError> {
    use super::warning::{TypeshedWarning, UnpinnedKind};

    let manifest: BundleManifest = serde_json::from_str(BUNDLE_MANIFEST_JSON)
        .map_err(|err| BundleError::Manifest(err.to_string()))?;
    let commit = Oid::from_hex(&manifest.source.commit_sha).map_err(BundleError::BadSha)?;
    let tree = Oid::from_hex(&manifest.source.tree_sha).map_err(BundleError::BadSha)?;
    let warnings: &[TypeshedWarning] = if explicit {
        &[]
    } else {
        &[TypeshedWarning::Unpinned(UnpinnedKind::BundledDefault)]
    };
    Ok(TypeshedStatus {
        active_source: SourceKind::Bundled,
        commit: Some(commit),
        tree: Some(tree),
        license_status: LicenseStatus::Approved,
        license_reference: Some(license_reference(&manifest.source.commit_sha)),
        warnings: super::source::StatusWarning::list(warnings),
    })
}

fn build_bundled_snapshot() -> Result<Snapshot, BundleError> {
    let manifest: BundleManifest = serde_json::from_str(BUNDLE_MANIFEST_JSON)
        .map_err(|err| BundleError::Manifest(err.to_string()))?;

    // The bundle has no top-level prefix; entries are `stdlib/…` and `LICENSE`.
    // Digests and gates are the build invariant ([`verify_bundled_assets`]);
    // activation only decodes — zero-copy, borrowing STORED entries from the
    // embedded bytes.
    let archive = decode_zip_static(
        BUNDLE_ZIP,
        ZipLayout::BundledRootless,
        &DecodeLimits::default(),
    )
    .map_err(BundleError::Decode)?;

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

    /// [STUBRES-TYPESHED-BASELINE] build invariant: the embedded assets match
    /// their manifest digests and pass every gate. `include_bytes!` data
    /// cannot change between process starts, so this holds as a CI-enforced
    /// invariant of the BUILD — the runtime activation path deliberately does
    /// not re-verify immutable bytes on every short-lived `check` process.
    #[test]
    fn bundled_assets_match_manifest_and_pass_all_gates() {
        assert_eq!(
            verify_bundled_assets(),
            Ok(()),
            "embedded bundle must match its manifest digests and pass Safety/Shape/License"
        );
    }

    /// [STUBRES-TYPESHED-WARN] `bundled_pinned_status` must report EXACTLY
    /// what the selector produces for a pin of the bundled commit — it exists
    /// so the banner can print without waiting for the archive decode, and a
    /// drift here would let the fast banner lie about the resolved source.
    #[test]
    #[expect(clippy::expect_used, reason = "test-only: fail loudly on fixtures")]
    fn manifest_status_matches_selector_status_for_both_pin_policies() {
        for explicit in [false, true] {
            let selected = crate::typeshed::selector::select_snapshot(
                &crate::typeshed::source::TypeshedRequest {
                    selection: crate::typeshed::source::SourceSelection::Pinned {
                        commit: Oid::from_hex(bundled_commit_sha()).expect("bundled sha parses"),
                        explicit,
                    },
                    store_path: None,
                },
                &crate::typeshed::runtime::RuntimeBackend::new(None),
            )
            .expect("bundled pin selects");
            let fast = bundled_pinned_status(explicit).expect("manifest status builds");
            assert_eq!(fast, selected.status, "explicit={explicit}");
        }
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
