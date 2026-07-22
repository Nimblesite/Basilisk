//! Compile-into-CI enforcement of [STUBRES-TYPESHED-BASELINE]. See
//! docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-BASELINE.
//!
//! Requirement: a basilisk binary must NEVER be shipped without a verified
//! typeshed standard library. The crate cannot use a `build.rs` to enforce
//! this — the forbidden-policy guard ([TYPESHEDRT-ACCEPTANCE-GATES],
//! `typeshed_forbidden_policy_tests.rs`) bans build scripts on this crate — so
//! the payload-integrity gate lives here, in the test suite that `test-rust.sh`
//! runs before any release build: a corrupt, truncated, or stale
//! `data/typeshed/stdlib.zip` or distribution sidecar fails CI, so the binary
//! is never produced. `src/typeshed/bundle.rs` re-checks the same identities at
//! runtime, defending against post-build corruption of an already-shipped
//! binary.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn manifest_json() -> serde_json::Value {
    let path = crate_dir().join("data/typeshed/manifest.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("{} is not valid JSON: {error}", path.display()))
}

/// Assert the on-disk asset at `relative` hashes to the SHA-256 the manifest
/// declares at `pointer` — the exact identity `include_bytes!` embeds into the
/// binary and `bundled_snapshot()` re-checks at runtime.
fn assert_asset_matches_manifest(manifest: &serde_json::Value, relative: &str, pointer: &str) {
    let expected = manifest
        .pointer(pointer)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("data/typeshed/manifest.json is missing `{pointer}`"));
    let path: &Path = &crate_dir().join(relative);
    let bytes = std::fs::read(path)
        .unwrap_or_else(|error| panic!("cannot read typeshed asset {}: {error}", path.display()));
    let actual = format!("{:x}", Sha256::digest(&bytes));
    assert_eq!(
        actual,
        expected,
        "embedded typeshed asset {} does not match its manifest digest — a basilisk binary must \
NEVER be produced without a verified typeshed standard library. Restore the pristine asset \
(`git checkout -- crates/basilisk-stubs/data`) or regenerate the bundle with \
`python3 scripts/update_typeshed_bundle.py`. [STUBRES-TYPESHED-BASELINE]",
        path.display()
    );
}

/// [STUBRES-TYPESHED-BASELINE]: the embedded stdlib ZIP payload must match its
/// manifest digest, or CI fails before any binary is built.
#[test]
fn embedded_stdlib_zip_matches_its_manifest_digest() {
    assert_asset_matches_manifest(
        &manifest_json(),
        "data/typeshed/stdlib.zip",
        "/bundle/sha256",
    );
}

/// [STUBRES-TYPESHED-BASELINE]: the embedded stub-distribution sidecar must
/// match its manifest digest, or CI fails before any binary is built.
#[test]
fn embedded_stub_distributions_match_their_manifest_digest() {
    assert_asset_matches_manifest(
        &manifest_json(),
        "data/typeshed_stub_distributions.tsv",
        "/derived_indexes/stub_distributions/sha256",
    );
}

/// [STUBRES-TYPESHED-BASELINE]: the whole bundled snapshot — ZIP digest,
/// distribution digest, safety/shape/license gates, and commit/tree identity —
/// must assemble cleanly from the embedded assets. This is the runtime gate
/// `bundled_snapshot()` runs, pinned here so any regression in the embedded
/// payload or the manifest metadata (license file digest, commit SHA) is a CI
/// failure, not a broken shipped binary.
#[test]
fn bundled_snapshot_assembles_from_the_embedded_assets() {
    let snapshot = basilisk_stubs::typeshed::bundle::bundled_snapshot()
        .expect("the embedded typeshed bundle must assemble without error");
    assert!(
        snapshot.read_stub("builtins").is_some(),
        "the verified bundle must expose the stdlib builtins stub"
    );
}
