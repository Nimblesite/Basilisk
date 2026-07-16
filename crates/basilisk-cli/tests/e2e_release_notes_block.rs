//! Tests for [LSPFMT-RELEASE-NOTES]. See
//! docs/specs/LSP-FORMATTING-SPEC.md#LSPFMT-RELEASE-NOTES
#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic
)]
//! Drift test for the generated release-notes component block
//! (`scripts/gen_release_notes.py`): the block is generated from
//! `shipwright.json` and the real binary, so the notes can never claim
//! different formatter bytes from the build. This test runs the generator
//! against the freshly compiled binary and proves the block enumerates every
//! manifest component and reports exactly the binary's embedded Ruff version.

use std::path::Path;
use std::process::Command;

/// The `Ruff formatter: X` line straight from the binary — the ground truth
/// the generated block must match.
fn formatter_version_from_binary() -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("--version")
        .output()
        .expect("run basilisk --version");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("Ruff formatter: "))
        .unwrap_or_else(|| panic!("--version must report the embedded formatter: {stdout}"))
        .to_owned()
}

#[test]
fn generated_block_matches_the_binary_and_the_manifest() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root");

    let out = Command::new("python3")
        .arg(repo_root.join("scripts/gen_release_notes.py"))
        .arg(env!("CARGO_BIN_EXE_basilisk"))
        .arg("v9.9.9-test")
        .arg(repo_root.join("shipwright.json"))
        .output()
        .expect("run gen_release_notes.py (python3 required, as for conformance)");
    assert!(
        out.status.success(),
        "generator must succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let block = String::from_utf8_lossy(&out.stdout).into_owned();

    // Every shipwright.json component is enumerated; versioned components
    // carry the release version passed in.
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root.join("shipwright.json")).expect("read manifest"),
    )
    .expect("parse manifest");
    let components = manifest["components"].as_array().expect("components");
    assert!(!components.is_empty(), "manifest must declare components");
    for component in components {
        let id = component["id"].as_str().expect("component id");
        assert!(
            block.contains(&format!("| `{id}` |")),
            "block must enumerate component `{id}`:\n{block}"
        );
        if component["expectedVersion"].as_str() == Some("${PRODUCT_VERSION}") {
            assert!(
                block
                    .lines()
                    .any(|l| l.contains(&format!("| `{id}` |")) && l.contains("v9.9.9-test")),
                "component `{id}` must carry the release version:\n{block}"
            );
        }
    }

    // The formatter line is the binary's own, byte for byte — the whole
    // point of generating the block ([LSPFMT-RELEASE-NOTES]).
    let expected = formatter_version_from_binary();
    assert!(
        block.contains(&format!("Embedded Ruff formatter: `{expected}`")),
        "block must report the binary's embedded Ruff version {expected}:\n{block}"
    );
}
