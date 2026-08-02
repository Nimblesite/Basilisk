//! Tests for [LSPARCH-TESTING], [ANALYSIS-CROSSLSP-REFS], [ANALYSIS-CROSSLSP-RENAME].
//! See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-TESTING
// Coverage-boost tests for CROSS-FILE navigation driven from the IMPORTER side
// (the file that imports the symbol). The existing cross-file tests put the
// cursor at the defining file; these put it at the importer, exercising the
// `resolved.imported_symbols.get(name)` branches in `references`, `rename`,
// and `goto_type_definition` (navigation.rs) that search the source file.

use super::ws_test_common::*;

/// Open two files under `dir` (helpers.py + main.py) in crossModule mode and
/// drain startup diagnostics. Returns the fixture + the two URIs.
async fn cross_module_pair(
    helper_src: &str,
    main_src: &str,
) -> TestResult<(WsTestFixture, String, String)> {
    let dir = unique_temp_dir("bsk_cross_nav");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("helpers.py"), helper_src)?;
    std::fs::write(dir.join("main.py"), main_src)?;

    let root_uri = format!("file://{}", dir.display());
    let helpers_uri = format!("file://{}", dir.join("helpers.py").display());
    let main_uri = format!("file://{}", dir.join("main.py").display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "crossModule").await?;
    // Drain startup scan diagnostics for both files.
    for _ in 0..30 {
        let msg = tokio::time::timeout(Duration::from_millis(400), fixture.ws_read.next()).await;
        if msg.is_err() {
            break;
        }
    }
    // Open both so the import graph + imported_symbols populate.
    fixture.did_open(&helpers_uri, helper_src).await?;
    fixture.did_open(&main_uri, main_src).await?;
    let _ = fixture.wait_for_diagnostics().await;

    Ok((fixture, helpers_uri, main_uri))
}

/// From the IMPORTER (main.py), `textDocument/references` on the imported
/// symbol must return references in BOTH the importer and the source file
/// (helpers.py). This exercises the `ext_sym.source_path != current_path`
/// source-file search branch of `references`.
#[tokio::test]
async fn test_ws_cross_file_references_from_importer() -> TestResult<()> {
    let helper_src = "def greet(name: str) -> str:\n    return name\n";
    let main_src = "from helpers import greet\n\nresult: str = greet(\"world\")\n";

    let (mut fixture, helpers_uri, main_uri) = cross_module_pair(helper_src, main_src).await?;

    // Cursor on `greet` at the import line in main.py (line 0, char 21).
    let resp = fixture
        .request(
            400,
            "textDocument/references",
            serde_json::json!({
                "textDocument": { "uri": main_uri },
                "position": { "line": 0, "character": 21 },
                "context": { "includeDeclaration": true }
            }),
        )
        .await?
        .ok_or("no references response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let refs = parsed["result"]
        .as_array()
        .ok_or("references result should be an array")?;

    let in_main = refs
        .iter()
        .filter(|r| r["uri"].as_str().unwrap_or("").contains("main.py"))
        .count();
    let in_helpers = refs
        .iter()
        .filter(|r| r["uri"].as_str().unwrap_or("").contains("helpers.py"))
        .count();

    assert!(
        in_main >= 1,
        "should find at least one reference in main.py (importer): {resp}"
    );
    assert!(
        in_helpers >= 1,
        "should find at least one reference in helpers.py (source): {resp}"
    );

    // includeDeclaration: false should still return the usage sites.
    let no_decl = fixture
        .request(
            401,
            "textDocument/references",
            serde_json::json!({
                "textDocument": { "uri": main_uri },
                "position": { "line": 2, "character": 16 },
                "context": { "includeDeclaration": false }
            }),
        )
        .await?
        .ok_or("no references (no-decl) response")?;
    assert!(
        no_decl.contains("main.py"),
        "references without declaration should still resolve importer usages: {no_decl}"
    );

    let _ = std::fs::remove_dir_all(helpers_uri.strip_prefix("file://").unwrap_or(""));
    Ok(())
}

/// From the IMPORTER (main.py), `textDocument/rename` on the imported symbol
/// must produce edits in BOTH the importer and the source file. This exercises
/// the `ext_sym.source_path != current_path` source-file rename branch.
#[tokio::test]
async fn test_ws_cross_file_rename_from_importer() -> TestResult<()> {
    let helper_src = "def greet(name: str) -> str:\n    return name\n";
    let main_src = "from helpers import greet\n\nresult: str = greet(\"world\")\n";

    let (mut fixture, helpers_uri, main_uri) = cross_module_pair(helper_src, main_src).await?;

    // Rename `greet` at the IMPORT site in main.py (line 0, char 21).
    let resp = fixture
        .request(
            410,
            "textDocument/rename",
            serde_json::json!({
                "textDocument": { "uri": main_uri },
                "position": { "line": 0, "character": 21 },
                "newName": "say_hello"
            }),
        )
        .await?
        .ok_or("no rename response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "rename must produce changes: {resp}");

    let helpers_edits = changes[&helpers_uri]
        .as_array()
        .ok_or("expected edits in helpers.py")?;
    assert!(
        !helpers_edits.is_empty(),
        "rename from importer must edit the source file (helpers.py): {resp}"
    );
    for edit in helpers_edits {
        assert_eq!(
            edit["newText"].as_str(),
            Some("say_hello"),
            "helpers.py edit must be say_hello: {edit}"
        );
    }

    let main_edits = changes[&main_uri]
        .as_array()
        .ok_or("expected edits in main.py")?;
    assert!(
        !main_edits.is_empty(),
        "rename from importer must edit the importer (main.py): {resp}"
    );

    Ok(())
}

/// From the IMPORTER (main.py), `textDocument/typeDefinition` on a variable
/// annotated with an imported class must jump cross-file to the class
/// declaration in helpers.py. Exercises the cross-file type-def branch.
#[tokio::test]
async fn test_ws_cross_file_type_definition_from_importer() -> TestResult<()> {
    let helper_src = "class Animal:\n    name: str\n";
    let main_src = "from helpers import Animal\n\npet: Animal = Animal()\n";

    let (mut fixture, helpers_uri, main_uri) = cross_module_pair(helper_src, main_src).await?;

    // Cursor on `pet` (line 2, char 0) — annotated `Animal` (imported).
    let resp = fixture
        .request(
            420,
            "textDocument/typeDefinition",
            serde_json::json!({
                "textDocument": { "uri": main_uri },
                "position": { "line": 2, "character": 0 }
            }),
        )
        .await?
        .ok_or("no cross-file typeDefinition response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "cross-file type-def must resolve: {resp}"
    );
    let result_uri = parsed["result"]["uri"].as_str().unwrap_or("");
    assert!(
        result_uri.contains("helpers.py"),
        "cross-file type-def should jump to helpers.py, got {result_uri}: {resp}"
    );

    let _ = helpers_uri;
    Ok(())
}

/// `goto_definition` from the IMPORTER on an imported symbol resolves
/// cross-file in wholeModule mode (on-demand resolution via `resolved_path`).
/// This re-covers the on-demand path for a `from … import` binding.
#[tokio::test]
async fn test_ws_cross_file_goto_definition_whole_module_from_importer() -> TestResult<()> {
    let dir = unique_temp_dir("bsk_cross_nav_whole");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("shapes.py"), "class Circle:\n    radius: float\n")?;
    std::fs::write(
        dir.join("app.py"),
        "from shapes import Circle\n\nc: Circle = Circle()\n",
    )?;

    let root_uri = format!("file://{}", dir.display());
    let shapes_uri = format!("file://{}", dir.join("shapes.py").display());
    let app_uri = format!("file://{}", dir.join("app.py").display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "wholeModule").await?;
    for _ in 0..20 {
        let msg = tokio::time::timeout(Duration::from_millis(400), fixture.ws_read.next()).await;
        if msg.is_err() {
            break;
        }
    }
    fixture
        .did_open(&shapes_uri, "class Circle:\n    radius: float\n")
        .await?;
    fixture
        .did_open(
            &app_uri,
            "from shapes import Circle\n\nc: Circle = Circle()\n",
        )
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Cursor on `Circle` at the import line in app.py (line 0, char 19).
    let resp = fixture
        .request(
            430,
            "textDocument/definition",
            serde_json::json!({
                "textDocument": { "uri": app_uri },
                "position": { "line": 0, "character": 19 }
            }),
        )
        .await?
        .ok_or("no wholeModule cross-file goto-def response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "wholeModule cross-file goto-def must resolve: {resp}"
    );
    assert!(
        parsed["result"]["uri"]
            .as_str()
            .unwrap_or("")
            .contains("shapes.py"),
        "wholeModule goto-def should jump to shapes.py: {resp}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
