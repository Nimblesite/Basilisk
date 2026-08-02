//! Tests for [REFACTOR-RENAMEMOD]. See docs/specs/LSP-ARCHITECTURE-SPEC.md
// Coverage-boost tests for `workspace/willRenameFiles`: renaming a Python
// module rewrites every importer's `import`/`from` statement to the new
// module path. Covers the `will_rename_files` handler and the
// `collect_import_edits_for_rename` / `collect_edits_for_importer` paths in
// `file_operations.rs` that the unit tests don't reach (they test the pure
// helpers only).

use super::ws_test_common::*;

/// Renaming `oldmod.py` → `newmod.py` must rewrite the import lines in every
/// file that imports it, in both `import oldmod` and `from oldmod import X`
/// form, and must NOT touch unrelated importers.
#[tokio::test]
async fn test_ws_will_rename_files_rewrites_importers() -> TestResult<()> {
    let dir = unique_temp_dir("bsk_willrename");
    std::fs::create_dir_all(&dir)?;
    // The module being renamed.
    std::fs::write(dir.join("oldmod.py"), "VALUE: int = 1\n")?;
    // An unrelated module that should NOT be edited.
    std::fs::write(dir.join("other.py"), "OTHER: int = 2\n")?;
    // Importer A: `import oldmod` (plain + aliased).
    std::fs::write(
        dir.join("a.py"),
        "import oldmod\nimport oldmod as om\nprint(oldmod.VALUE)\n",
    )?;
    // Importer B: `from oldmod import VALUE`.
    std::fs::write(dir.join("b.py"), "from oldmod import VALUE\nprint(VALUE)\n")?;

    let root_uri = format!("file://{}", dir.display());
    let old_uri = format!("file://{}", dir.join("oldmod.py").display());
    let new_uri = format!("file://{}", dir.join("newmod.py").display());
    let a_uri = format!("file://{}", dir.join("a.py").display());
    let b_uri = format!("file://{}", dir.join("b.py").display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "crossModule").await?;
    // Drain startup scan.
    for _ in 0..40 {
        let msg = tokio::time::timeout(Duration::from_millis(400), fixture.ws_read.next()).await;
        if msg.is_err() {
            break;
        }
    }
    // Open the importers so they are indexed.
    for (uri, src) in [
        (
            &a_uri,
            "import oldmod\nimport oldmod as om\nprint(oldmod.VALUE)\n",
        ),
        (&b_uri, "from oldmod import VALUE\nprint(VALUE)\n"),
    ] {
        fixture.did_open(uri, src).await?;
    }
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            600,
            "workspace/willRenameFiles",
            serde_json::json!({
                "files": [{ "oldUri": old_uri, "newUri": new_uri }]
            }),
        )
        .await?
        .ok_or("no willRenameFiles response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(
        !changes.is_null(),
        "willRenameFiles must produce changes: {resp}"
    );

    let a_edits = changes[&a_uri].as_array().ok_or("expected edits in a.py")?;
    assert!(
        a_edits.len() >= 2,
        "a.py should have at least 2 edits (import + aliased import): {resp}"
    );
    for edit in a_edits {
        let new_text = edit["newText"].as_str().unwrap_or("");
        assert!(
            new_text.contains("newmod"),
            "a.py edit should rewrite to 'newmod': {edit}"
        );
        assert!(
            !new_text.contains("oldmod"),
            "a.py edit should not retain 'oldmod': {edit}"
        );
    }

    let b_edits = changes[&b_uri].as_array().ok_or("expected edits in b.py")?;
    assert_eq!(
        b_edits.len(),
        1,
        "b.py should have exactly 1 edit (the from-import): {resp}"
    );
    assert_eq!(
        b_edits[0]["newText"].as_str(),
        Some("from newmod import VALUE"),
        "b.py from-import should rewrite to newmod: {resp}"
    );

    // other.py must NOT appear in the changes — it doesn't import oldmod.
    let other_uri = format!("file://{}", dir.join("other.py").display());
    assert!(
        changes.get(&other_uri).is_none(),
        "unrelated other.py must not be edited: {resp}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

/// Renaming a file to a path that yields the SAME module path (e.g. across a
/// case-only rename on a case-insensitive FS, or a no-op rename) returns null
/// — covers the `old_module == new_module` early-return.
#[tokio::test]
async fn test_ws_will_rename_files_same_module_returns_null() -> TestResult<()> {
    let dir = unique_temp_dir("bsk_willrename_same");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("mod.py"), "X: int = 1\n")?;
    std::fs::write(dir.join("user.py"), "import mod\n")?;

    let root_uri = format!("file://{}", dir.display());
    let old_uri = format!("file://{}", dir.join("mod.py").display());
    let new_uri = format!("file://{}", dir.join("mod.py").display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "crossModule").await?;
    for _ in 0..40 {
        let msg = tokio::time::timeout(Duration::from_millis(400), fixture.ws_read.next()).await;
        if msg.is_err() {
            break;
        }
    }
    fixture.did_open(&new_uri, "X: int = 1\n").await?;
    fixture
        .did_open(
            &format!("file://{}", dir.join("user.py").display()),
            "import mod\n",
        )
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            610,
            "workspace/willRenameFiles",
            serde_json::json!({ "files": [{ "oldUri": old_uri, "newUri": new_uri }] }),
        )
        .await?
        .ok_or("no willRenameFiles response")?;
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"].is_null(),
        "rename to the same module path should yield null: {resp}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
