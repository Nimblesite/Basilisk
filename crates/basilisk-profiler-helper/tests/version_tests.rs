#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic
)]
//! Shipwright version contract tests for the profiler helper binary.

use std::process::Command;

use serde_json::Value;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_basilisk-profiler-helper"))
}

#[test]
fn version_plain_matches_shipwright_contract() -> Result<(), Box<dyn std::error::Error>> {
    let output = binary().arg("--version").output()?;
    assert_eq!(output.status.code(), Some(0), "--version must exit 0");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        concat!("basilisk-profiler-helper ", env!("CARGO_PKG_VERSION"))
    );
    assert!(output.stderr.is_empty(), "--version must keep stderr empty");
    Ok(())
}

#[test]
fn version_json_matches_shipwright_contract() -> Result<(), Box<dyn std::error::Error>> {
    let output = binary().args(["--version", "--json"]).output()?;
    assert_eq!(
        output.status.code(),
        Some(0),
        "--version --json must exit 0"
    );

    let value: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["manifestVersion"], 1);
    assert_eq!(value["name"], "basilisk-profiler-helper");
    assert_eq!(value["binaryName"], "basilisk-profiler-helper");
    assert_eq!(value["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(value["kind"], "tool");
    assert_eq!(value["language"], "rust");
    assert_eq!(value["product"], "basilisk");
    assert!(
        value.get("buildTime").is_some(),
        "buildTime must be present"
    );
    assert!(value.get("gitDirty").is_some(), "gitDirty must be present");
    assert!(
        output.stderr.is_empty(),
        "--version --json must keep stderr empty"
    );
    Ok(())
}
