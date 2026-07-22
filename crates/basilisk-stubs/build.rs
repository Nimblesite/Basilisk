//! Compile-time verification of the embedded typeshed assets
//! ([STUBRES-TYPESHED-BASELINE]). See
//! docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-BASELINE.
//!
//! The digests `src/typeshed/bundle.rs` re-checks at runtime are enforced
//! FIRST here, where failure is cheapest and loudest: a corrupt, truncated,
//! or stale `data/typeshed/stdlib.zip` (or distribution sidecar) fails the
//! BUILD, so no basilisk binary can ever be produced — let alone deployed —
//! without a verified typeshed standard library.

use std::error::Error;
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let manifest_path = manifest_dir.join("data/typeshed/manifest.json");
    let bundle_path = manifest_dir.join("data/typeshed/stdlib.zip");
    let distributions_path = manifest_dir.join("data/typeshed_stub_distributions.tsv");
    for path in [&manifest_path, &bundle_path, &distributions_path] {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path)?)?;
    verify_digest(&bundle_path, &manifest, "/bundle/sha256")?;
    verify_digest(
        &distributions_path,
        &manifest,
        "/derived_indexes/stub_distributions/sha256",
    )
}

/// Fail the build unless `path` hashes to the digest the bundle manifest
/// declares at `pointer` — the same identities the runtime gate re-checks.
fn verify_digest(
    path: &Path,
    manifest: &serde_json::Value,
    pointer: &str,
) -> Result<(), Box<dyn Error>> {
    let expected = manifest
        .pointer(pointer)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("data/typeshed/manifest.json is missing `{pointer}`"))?;
    let bytes = std::fs::read(path)
        .map_err(|error| format!("cannot read typeshed asset {}: {error}", path.display()))?;
    let actual = format!("{:x}", Sha256::digest(&bytes));
    if actual == expected {
        return Ok(());
    }
    Err(format!(
        "BUILD HALTED — embedded typeshed asset {} does not match its manifest digest \
(expected {expected}, found {actual}). A basilisk binary must NEVER be produced without a \
verified typeshed standard library. Restore the pristine asset (`git checkout -- \
crates/basilisk-stubs/data`) or regenerate the bundle with \
`python3 scripts/update_typeshed_bundle.py`. [STUBRES-TYPESHED-BASELINE]",
        path.display()
    )
    .into())
}
