//! Ordinary CLI acceptance for [STUBRES-TYPESHED-WARN].
//! See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-WARN.

use std::process::Command;

#[test]
fn check_uses_custom_typeshed_and_routes_status_only_to_stderr(
) -> Result<(), Box<dyn std::error::Error>> {
    let workspace = tempfile::tempdir()?;
    let stdlib = workspace.path().join("typeshed").join("stdlib");
    std::fs::create_dir_all(&stdlib)?;
    std::fs::write(stdlib.join("VERSIONS"), "os: 3.8-\n")?;
    std::fs::write(stdlib.join("os.pyi"), "def getcwd() -> str: ...\n")?;
    std::fs::write(workspace.path().join("app.py"), "from os import getcwd\n")?;
    std::fs::write(
        workspace.path().join("pyproject.toml"),
        "[tool.basilisk]\ntypeshed-path = \"typeshed\"\n",
    )?;

    let output = Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("check")
        .arg(workspace.path().join("app.py"))
        .args(["--output", "json", "--color", "never"])
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        output.status.success(),
        "custom Typeshed check failed: stdout={stdout}; stderr={stderr}"
    );
    let diagnostics: serde_json::Value = serde_json::from_str(&stdout)?;
    assert_eq!(diagnostics, serde_json::json!([]));
    assert!(!stdout.contains("UNPINNED"));
    assert!(stderr.contains("typeshed source status"), "{stderr}");
    assert!(stderr.contains("active_source=\"custom\""), "{stderr}");
    assert!(stderr.contains("license_status=NotSupplied"), "{stderr}");
    assert!(
        !stderr.contains("provenance="),
        "active_source IS the trust story — no provenance field may reappear: {stderr}"
    );
    assert!(
        !stderr.contains("signed_release="),
        "active_source IS the trust story — no signed_release field may reappear: {stderr}"
    );
    let unpinned = stderr.find("warning_code=\"UNPINNED\"");
    let user_managed = stderr.find("warning_code=\"USER-MANAGED SOURCE\"");
    assert!(
        unpinned
            .zip(user_managed)
            .is_some_and(|(first, second)| first < second),
        "status warnings must preserve canonical order: {stderr}"
    );
    Ok(())
}

#[test]
fn a_pin_missing_from_the_store_does_not_fall_back() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = tempfile::tempdir()?;
    std::fs::write(workspace.path().join("app.py"), "from os import getcwd\n")?;
    std::fs::write(
        workspace.path().join("pyproject.toml"),
        concat!(
            "[tool.basilisk]\n",
            "typeshed-commit = \"0000000000000000000000000000000000000000\"\n",
            "typeshed-store-path = \"store\"\n",
        ),
    )?;

    let output = Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("check")
        .arg(workspace.path().join("app.py"))
        .args(["--output", "json", "--color", "never"])
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;
    let stderr = String::from_utf8(output.stderr)?;
    assert_eq!(
        output.status.code(),
        Some(3),
        "stdout={stdout}; stderr={stderr}"
    );
    assert!(
        stderr.contains("NO SOURCE")
            && stderr.contains("0000000000000000000000000000000000000000"),
        "the failure must carry the spec's NO SOURCE line naming the pin: {stderr}"
    );
    assert!(
        !stderr.contains("typeshed source status"),
        "a missing pin must not activate or report a fallback: {stderr}"
    );
    Ok(())
}
