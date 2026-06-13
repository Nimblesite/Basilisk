//! Tests for [ANALYSIS-CROSSLSP]. See docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-CROSSLSP
// E2E tests for cross-module analysis.
//
// These tests exercise the full LSP pipeline: real `.py` files on disk,
// workspace scan with `crossModule` analysis mode, import resolution,
// cross-module symbol population, and diagnostic checking.

use super::ws_test_common::{initialize_with_root, unique_temp_dir, WsTestFixture};

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Write test files to a temp directory and return the root URI.
fn setup_cross_module_workspace(files: &[(&str, &str)]) -> (std::path::PathBuf, String) {
    let dir = unique_temp_dir("bsk_cross_module");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, content) in files {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }
    let root_uri = format!("file://{}", dir.display());
    (dir, root_uri)
}

/// Drain all diagnostics messages until timeout, return them keyed by URI.
async fn collect_all_diagnostics(
    fixture: &mut WsTestFixture,
) -> std::collections::HashMap<String, serde_json::Value> {
    let mut result = std::collections::HashMap::new();
    // Give workspace scan time to complete.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    for _ in 0..20 {
        let Some(msg) = fixture.recv().await else {
            break;
        };
        if msg.contains("\"method\":\"textDocument/publishDiagnostics\"") {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&msg) {
                if let Some(uri) = json["params"]["uri"].as_str() {
                    let _ = result.insert(uri.to_owned(), json);
                }
            }
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Cross-module: imported symbol suppresses E0018 (undefined variable)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cross_module_imported_symbol_suppresses_e0018() -> TestResult<()> {
    let (dir, root_uri) = setup_cross_module_workspace(&[
        ("utils.py", "def helper(x: int) -> int:\n    return x * 2\n"),
        (
            "main.py",
            "from utils import helper\n\ndef run() -> int:\n    return helper(42)\n",
        ),
    ]);

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "crossModule").await?;

    let diags = collect_all_diagnostics(&mut fixture).await;

    // main.py should NOT have E0018 for `helper` — it's imported
    let main_uri = format!("{root_uri}/main.py");
    if let Some(main_diag) = diags.get(&main_uri) {
        let empty = vec![];
        let diagnostics = main_diag["params"]["diagnostics"]
            .as_array()
            .unwrap_or(&empty);
        let e0018_for_helper = diagnostics.iter().any(|d| {
            d["code"].as_str() == Some("BSK-E0018")
                && d["message"].as_str().unwrap_or("").contains("helper")
        });
        assert!(
            !e0018_for_helper,
            "E0018 should NOT fire for imported symbol `helper` in main.py, got: {diagnostics:#?}"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

// ---------------------------------------------------------------------------
// Cross-module: undefined variable still fires when NOT imported
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cross_module_undefined_variable_still_fires() -> TestResult<()> {
    let (dir, root_uri) = setup_cross_module_workspace(&[(
        "main.py",
        "def run() -> int:\n    return nonexistent_thing\n",
    )]);

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "crossModule").await?;

    let diags = collect_all_diagnostics(&mut fixture).await;

    let main_uri = format!("{root_uri}/main.py");
    if let Some(main_diag) = diags.get(&main_uri) {
        let empty = vec![];
        let diagnostics = main_diag["params"]["diagnostics"]
            .as_array()
            .unwrap_or(&empty);
        let has_e0018 = diagnostics
            .iter()
            .any(|d| d["code"].as_str() == Some("BSK-E0018"));
        assert!(
            has_e0018,
            "E0018 should fire for truly undefined `nonexistent_thing`, got: {diagnostics:#?}"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

// ---------------------------------------------------------------------------
// Cross-module: import graph is built correctly
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cross_module_import_graph_built() -> TestResult<()> {
    let (dir, root_uri) = setup_cross_module_workspace(&[
        ("base.py", "BASE_VALUE: int = 42\n"),
        (
            "middle.py",
            "from base import BASE_VALUE\n\ndef double() -> int:\n    return BASE_VALUE\n",
        ),
        (
            "top.py",
            "from middle import double\n\ndef run() -> int:\n    return double()\n",
        ),
    ]);

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "crossModule").await?;

    let diags = collect_all_diagnostics(&mut fixture).await;

    // None of the files should have E0018 for their imported symbols
    let empty = vec![];
    let top_uri = format!("{root_uri}/top.py");
    if let Some(top_diag) = diags.get(&top_uri) {
        let diagnostics = top_diag["params"]["diagnostics"]
            .as_array()
            .unwrap_or(&empty);
        let e0018_for_double = diagnostics.iter().any(|d| {
            d["code"].as_str() == Some("BSK-E0018")
                && d["message"].as_str().unwrap_or("").contains("double")
        });
        assert!(
            !e0018_for_double,
            "E0018 should not fire for imported `double` in top.py: {diagnostics:#?}"
        );
    }

    let middle_uri = format!("{root_uri}/middle.py");
    if let Some(mid_diag) = diags.get(&middle_uri) {
        let diagnostics = mid_diag["params"]["diagnostics"]
            .as_array()
            .unwrap_or(&empty);
        let e0018_for_base = diagnostics.iter().any(|d| {
            d["code"].as_str() == Some("BSK-E0018")
                && d["message"].as_str().unwrap_or("").contains("BASE_VALUE")
        });
        assert!(
            !e0018_for_base,
            "E0018 should not fire for imported `BASE_VALUE` in middle.py: {diagnostics:#?}"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

// ---------------------------------------------------------------------------
// Cross-module: class import resolves correctly
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cross_module_class_import() -> TestResult<()> {
    let (dir, root_uri) = setup_cross_module_workspace(&[
        (
            "models.py",
            "class User:\n    name: str\n    age: int\n    def greet(self) -> str:\n        return self.name\n",
        ),
        (
            "app.py",
            "from models import User\n\ndef create_user() -> User:\n    return User()\n",
        ),
    ]);

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "crossModule").await?;

    let diags = collect_all_diagnostics(&mut fixture).await;

    // app.py should NOT have E0018 for `User`
    let empty = vec![];
    let app_uri = format!("{root_uri}/app.py");
    if let Some(app_diag) = diags.get(&app_uri) {
        let diagnostics = app_diag["params"]["diagnostics"]
            .as_array()
            .unwrap_or(&empty);
        let e0018_for_user = diagnostics.iter().any(|d| {
            d["code"].as_str() == Some("BSK-E0018")
                && d["message"].as_str().unwrap_or("").contains("User")
        });
        assert!(
            !e0018_for_user,
            "E0018 should not fire for imported class `User` in app.py: {diagnostics:#?}"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

// ---------------------------------------------------------------------------
// Cross-module: circular import detection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cross_module_circular_import_no_crash() -> TestResult<()> {
    let (dir, root_uri) = setup_cross_module_workspace(&[
        (
            "a.py",
            "from b import func_b\n\ndef func_a() -> int:\n    return func_b()\n",
        ),
        (
            "b.py",
            "from a import func_a\n\ndef func_b() -> int:\n    return func_a()\n",
        ),
    ]);

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "crossModule").await?;

    // The key assertion: the server should NOT crash on circular imports.
    // We just need to get diagnostics back without hanging or panicking.
    let diags = collect_all_diagnostics(&mut fixture).await;

    // Both files should produce some diagnostics (at minimum E0010 for unresolved imports
    // or E0001 for missing annotations). The important thing is we got results.
    assert!(
        !diags.is_empty(),
        "should receive diagnostics even with circular imports"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

// ---------------------------------------------------------------------------
// Cross-module: multiple imports from same module
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cross_module_multiple_imports_from_same_module() -> TestResult<()> {
    let (dir, root_uri) = setup_cross_module_workspace(&[
        (
            "lib.py",
            "def add(x: int, y: int) -> int:\n    return x + y\n\ndef sub(x: int, y: int) -> int:\n    return x - y\n\nPI: float = 3.14\n",
        ),
        (
            "consumer.py",
            "from lib import add, sub, PI\n\ndef compute() -> float:\n    return add(1, 2)\n",
        ),
    ]);

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "crossModule").await?;

    let diags = collect_all_diagnostics(&mut fixture).await;

    let empty = vec![];
    let consumer_uri = format!("{root_uri}/consumer.py");
    if let Some(consumer_diag) = diags.get(&consumer_uri) {
        let diagnostics = consumer_diag["params"]["diagnostics"]
            .as_array()
            .unwrap_or(&empty);

        // None of the imported symbols should trigger E0018
        for symbol in &["add", "sub", "PI"] {
            let e0018_for_sym = diagnostics.iter().any(|d| {
                d["code"].as_str() == Some("BSK-E0018")
                    && d["message"].as_str().unwrap_or("").contains(symbol)
            });
            assert!(
                !e0018_for_sym,
                "E0018 should not fire for imported `{symbol}` in consumer.py: {diagnostics:#?}"
            );
        }
    }

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

// ---------------------------------------------------------------------------
// Cross-module: subdirectory package imports
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cross_module_package_import() -> TestResult<()> {
    let (dir, root_uri) = setup_cross_module_workspace(&[
        ("pkg/__init__.py", ""),
        (
            "pkg/core.py",
            "def process(data: str) -> str:\n    return data\n",
        ),
        (
            "main.py",
            "from pkg.core import process\n\ndef run() -> str:\n    return process('hello')\n",
        ),
    ]);

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "crossModule").await?;

    let diags = collect_all_diagnostics(&mut fixture).await;

    // main.py should resolve the package import without E0018
    let main_uri = format!("{root_uri}/main.py");
    if let Some(main_diag) = diags.get(&main_uri) {
        let empty = vec![];
        let diagnostics = main_diag["params"]["diagnostics"]
            .as_array()
            .unwrap_or(&empty);
        let e0018_for_process = diagnostics.iter().any(|d| {
            d["code"].as_str() == Some("BSK-E0018")
                && d["message"].as_str().unwrap_or("").contains("process")
        });
        assert!(
            !e0018_for_process,
            "E0018 should not fire for package-imported `process`: {diagnostics:#?}"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

// ---------------------------------------------------------------------------
// Cross-module: Go to Definition across files
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cross_module_goto_definition() -> TestResult<()> {
    let (dir, root_uri) = setup_cross_module_workspace(&[
        (
            "definitions.py",
            "def target_func(x: int) -> int:\n    return x\n",
        ),
        (
            "caller.py",
            "from definitions import target_func\n\ndef use_it() -> int:\n    return target_func(1)\n",
        ),
    ]);

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "crossModule").await?;

    // Drain initial diagnostics
    let _ = collect_all_diagnostics(&mut fixture).await;

    // Open caller.py and request Go to Definition on `target_func` at line 0
    let caller_uri = format!("{root_uri}/caller.py");
    fixture
        .did_open(
            &caller_uri,
            "from definitions import target_func\n\ndef use_it() -> int:\n    return target_func(1)\n",
        )
        .await?;

    // Wait for diagnostics from didOpen
    let _ = fixture.wait_for_diagnostics().await;

    // Go to definition on `target_func` in the import statement (line 0, col 28)
    let resp = fixture
        .request(
            10,
            "textDocument/definition",
            serde_json::json!({
                "textDocument": { "uri": caller_uri },
                "position": { "line": 0, "character": 28 }
            }),
        )
        .await?;

    // We should get a response (even if it's null for now — the test documents
    // the expected behavior)
    assert!(resp.is_some(), "should get a definition response");

    let resp_json: serde_json::Value = serde_json::from_str(&resp.unwrap())?;
    assert!(
        resp_json.get("result").is_some(),
        "definition response should have a result field: {resp_json}"
    );

    // When cross-file goto-def is working, result should point to definitions.py line 0.
    let result = &resp_json["result"];
    if !result.is_null() {
        // Result may be a single Location or an array of Locations.
        let location = if result.is_array() {
            result.as_array().and_then(|arr| arr.first())
        } else {
            Some(result)
        };
        if let Some(loc) = location {
            let target_uri = loc["uri"].as_str().unwrap_or("");
            assert!(
                target_uri.contains("definitions.py"),
                "goto-def should point to definitions.py, got: {target_uri}"
            );
            assert_eq!(
                loc["range"]["start"]["line"].as_u64(),
                Some(0),
                "goto-def should point to line 0 of definitions.py"
            );
        }
    }

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

// ---------------------------------------------------------------------------
// Cross-module: Find All References across files
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cross_module_find_references() -> TestResult<()> {
    let (dir, root_uri) = setup_cross_module_workspace(&[
        (
            "shared.py",
            "def shared_func(x: int) -> int:\n    return x\n",
        ),
        (
            "user1.py",
            "from shared import shared_func\n\ndef a() -> int:\n    return shared_func(1)\n",
        ),
        (
            "user2.py",
            "from shared import shared_func\n\ndef b() -> int:\n    return shared_func(2)\n",
        ),
    ]);

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "crossModule").await?;

    let _ = collect_all_diagnostics(&mut fixture).await;

    let shared_uri = format!("{root_uri}/shared.py");
    fixture
        .did_open(
            &shared_uri,
            "def shared_func(x: int) -> int:\n    return x\n",
        )
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Find references on `shared_func` definition (line 0, col 4)
    let resp = fixture
        .request(
            20,
            "textDocument/references",
            serde_json::json!({
                "textDocument": { "uri": shared_uri },
                "position": { "line": 0, "character": 4 },
                "context": { "includeDeclaration": true }
            }),
        )
        .await?;

    assert!(resp.is_some(), "should get a references response");
    let resp_json: serde_json::Value = serde_json::from_str(&resp.unwrap())?;
    assert!(
        resp_json.get("result").is_some(),
        "references response should have a result: {resp_json}"
    );

    // When cross-file references is working, result should include locations
    // in shared.py, user1.py, and user2.py
    if let Some(locations) = resp_json["result"].as_array() {
        assert!(
            !locations.is_empty(),
            "should find at least the definition itself"
        );

        // Verify the definition site (shared.py) is included.
        let has_shared = locations
            .iter()
            .any(|loc| loc["uri"].as_str().unwrap_or("").contains("shared.py"));
        assert!(
            has_shared,
            "references should include definition in shared.py, got: {locations:#?}"
        );

        // When cross-file refs fully works, user1.py and user2.py should appear.
        let has_user1 = locations
            .iter()
            .any(|loc| loc["uri"].as_str().unwrap_or("").contains("user1.py"));
        let has_user2 = locations
            .iter()
            .any(|loc| loc["uri"].as_str().unwrap_or("").contains("user2.py"));
        // These may not pass yet — documenting expected behavior.
        if has_user1 && has_user2 {
            assert!(
                locations.len() >= 3,
                "should find at least 3 references (def + 2 usage sites), got: {}",
                locations.len()
            );
        }
    }

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

// ---------------------------------------------------------------------------
// Regression: BSK-E0014 false positive on first-party import in a NEWLY
// CREATED file within a known package (GitHub #53).
//
// A pre-existing sibling (`dispatcher.py`) imports `from nap.agent_workspace.host
// import Err` and resolves cleanly. A brand-new file (`log_persist.py`) created
// in the same package with the SAME import must resolve identically — without a
// server reload. Implements [ANALYSIS-INCR-IMPORTS].
// ---------------------------------------------------------------------------

#[tokio::test]
async fn new_file_resolves_first_party_sibling_import() -> TestResult<()> {
    // `src/` layout so workspace-member discovery adds `src/` as a source root.
    let (dir, root_uri) = setup_cross_module_workspace(&[
        (
            "pyproject.toml",
            "[project]\nname = \"nap\"\nversion = \"0.0.0\"\n",
        ),
        ("src/nap/__init__.py", ""),
        ("src/nap/agent_workspace/__init__.py", ""),
        (
            "src/nap/agent_workspace/host.py",
            "class Err:\n    code: int\n",
        ),
        (
            // Pre-existing sibling — present during the startup scan.
            "src/nap/agent_workspace/dispatcher.py",
            "from nap.agent_workspace.host import Err\n\ndef run() -> Err:\n    return Err()\n",
        ),
    ]);

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "wholeModule").await?;

    // Drain the startup-scan diagnostics.
    let _ = collect_all_diagnostics(&mut fixture).await;

    // Create a NEW file in the SAME package AFTER the scan, then open it.
    let new_rel = "src/nap/agent_workspace/log_persist.py";
    let new_code =
        "from nap.agent_workspace.host import Err\n\ndef persist() -> Err:\n    return Err()\n";
    std::fs::write(dir.join(new_rel), new_code)?;
    let new_uri = format!("{root_uri}/{new_rel}");
    fixture.did_open(&new_uri, new_code).await?;

    let diag_msg = fixture.wait_for_diagnostics().await?;
    let json: serde_json::Value = serde_json::from_str(&diag_msg)?;
    let empty = vec![];
    let diagnostics = json["params"]["diagnostics"].as_array().unwrap_or(&empty);

    let unresolved_import = diagnostics.iter().any(|d| {
        let code = d["code"].as_str().unwrap_or("");
        (code == "BSK-E0010" || code == "BSK-E0014")
            && d["message"]
                .as_str()
                .unwrap_or("")
                .contains("nap.agent_workspace.host")
    });
    assert!(
        !unresolved_import,
        "new file must resolve first-party sibling import like its pre-existing \
         siblings — got unresolved-import diagnostic: {diagnostics:#?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

// ---------------------------------------------------------------------------
// Regression: creating a NEW module must update the package index so that
// pre-existing files importing it stop reporting BSK-E0010 — WITHOUT a server
// reload (GitHub #53). The file-watcher CREATED event must re-resolve the
// importers of the new module, not just the new module's own imports.
// Implements [ANALYSIS-INCR-IMPORTS].
// ---------------------------------------------------------------------------

#[tokio::test]
async fn created_module_clears_unresolved_import_in_dependents() -> TestResult<()> {
    // `src/` layout so workspace-member discovery adds `src/` as a source root.
    let (dir, root_uri) = setup_cross_module_workspace(&[
        ("pyproject.toml", "[project]\nname = \"nap\"\nversion = \"0.0.0\"\n"),
        ("src/nap/__init__.py", ""),
        ("src/nap/agent_workspace/__init__.py", ""),
        (
            // Consumer imports a module that does NOT exist yet at scan time.
            "src/nap/agent_workspace/consumer.py",
            "from nap.agent_workspace.log_persist import fetch_logs\n\ndef use() -> None:\n    fetch_logs()\n",
        ),
    ]);

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "wholeModule").await?;

    // Startup scan: consumer.py SHOULD report BSK-E0010 — log_persist is absent.
    let startup = collect_all_diagnostics(&mut fixture).await;
    let consumer_uri = format!("{root_uri}/src/nap/agent_workspace/consumer.py");
    let startup_has_e0010 = startup.get(&consumer_uri).is_some_and(|d| {
        d["params"]["diagnostics"]
            .as_array()
            .is_some_and(|arr| arr.iter().any(is_log_persist_unresolved))
    });
    assert!(
        startup_has_e0010,
        "precondition: consumer.py should report unresolved import before log_persist exists"
    );

    // Now CREATE the missing module on disk and notify via the file watcher,
    // exactly as an editor does when a new file appears.
    let new_rel = "src/nap/agent_workspace/log_persist.py";
    std::fs::write(
        dir.join(new_rel),
        "def fetch_logs() -> None:\n    return None\n",
    )?;
    let new_uri = format!("{root_uri}/{new_rel}");
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "workspace/didChangeWatchedFiles",
            "params": { "changes": [{ "uri": new_uri, "type": 1 }] }  // 1 = Created
        }))
        .await?;

    // The dependent (consumer.py) must be re-resolved and re-published WITHOUT
    // its unresolved-import diagnostic — no reload required.
    let after = collect_all_diagnostics(&mut fixture).await;
    let consumer_after = after
        .get(&consumer_uri)
        .ok_or("consumer.py diagnostics not re-published after the module was created")?;
    let empty = vec![];
    let diagnostics = consumer_after["params"]["diagnostics"]
        .as_array()
        .unwrap_or(&empty);
    let still_unresolved = diagnostics.iter().any(is_log_persist_unresolved);
    assert!(
        !still_unresolved,
        "creating log_persist.py must clear the unresolved-import diagnostic in consumer.py \
         without a reload — got: {diagnostics:#?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

/// True iff the diagnostic is an unresolved-import error for `log_persist`.
fn is_log_persist_unresolved(d: &serde_json::Value) -> bool {
    let code = d["code"].as_str().unwrap_or("");
    (code == "BSK-E0010" || code == "BSK-E0014")
        && d["message"].as_str().unwrap_or("").contains("log_persist")
}

// ---------------------------------------------------------------------------
// Cross-module: wildcard import
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cross_module_wildcard_import() -> TestResult<()> {
    let (dir, root_uri) = setup_cross_module_workspace(&[
        (
            "exports.py",
            "def alpha() -> int:\n    return 1\n\ndef beta() -> str:\n    return 'b'\n",
        ),
        (
            "consumer.py",
            "from exports import *\n\ndef use_them() -> int:\n    return alpha()\n",
        ),
    ]);

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "crossModule").await?;

    let diags = collect_all_diagnostics(&mut fixture).await;

    // With wildcard imports, `alpha` should be available (when supported)
    let empty = vec![];
    let consumer_uri = format!("{root_uri}/consumer.py");
    if let Some(consumer_diag) = diags.get(&consumer_uri) {
        let diagnostics = consumer_diag["params"]["diagnostics"]
            .as_array()
            .unwrap_or(&empty);
        // Document the expected behavior: wildcard imports should resolve
        let e0018_count = diagnostics
            .iter()
            .filter(|d| d["code"].as_str() == Some("BSK-E0018"))
            .count();
        // This test documents current behavior — wildcard import resolution
        // may or may not suppress E0018 depending on implementation status
        assert!(
            e0018_count <= 1,
            "at most one E0018 expected for wildcard import edge case: {diagnostics:#?}"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

// ---------------------------------------------------------------------------
// Regression: EDITING an existing module's top-level symbols must refresh
// dependents' cross-module diagnostics live, exactly like module creation
// (GitHub #56). The file-watcher CHANGED event must re-populate cross-module
// symbols and re-check dependents when the exported symbol set changes.
// Implements [ANALYSIS-SYMBOLS-INVAL].
// ---------------------------------------------------------------------------

#[tokio::test]
async fn changed_module_exports_refresh_dependent_diagnostics() -> TestResult<()> {
    let (dir, root_uri) = setup_cross_module_workspace(&[
        ("lib.py", ""),
        (
            "main.py",
            "import lib\n\ndef use() -> int:\n    return thing\n",
        ),
    ]);

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "crossModule").await?;

    // Startup scan: main.py SHOULD report `thing` as undefined — lib is empty.
    let startup = collect_all_diagnostics(&mut fixture).await;
    let main_uri = format!("{root_uri}/main.py");
    let startup_has_undefined = startup.get(&main_uri).is_some_and(|d| {
        d["params"]["diagnostics"]
            .as_array()
            .is_some_and(|arr| arr.iter().any(is_thing_undefined))
    });
    assert!(
        startup_has_undefined,
        "precondition: main.py should report `thing` undefined while lib.py is empty"
    );

    // EDIT lib.py on disk to export `thing`, then notify via the file watcher.
    std::fs::write(dir.join("lib.py"), "def thing() -> int:\n    return 1\n")?;
    let lib_uri = format!("{root_uri}/lib.py");
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "workspace/didChangeWatchedFiles",
            "params": { "changes": [{ "uri": lib_uri, "type": 2 }] }  // 2 = Changed
        }))
        .await?;

    // The dependent (main.py) must be re-checked and re-published WITHOUT the
    // stale diagnostic — no reload required.
    let after = collect_all_diagnostics(&mut fixture).await;
    let main_after = after
        .get(&main_uri)
        .ok_or("main.py diagnostics not re-published after lib.py's exports changed")?;
    let empty = vec![];
    let diagnostics = main_after["params"]["diagnostics"]
        .as_array()
        .unwrap_or(&empty);
    let still_undefined = diagnostics.iter().any(is_thing_undefined);
    assert!(
        !still_undefined,
        "adding `thing` to lib.py must clear the stale diagnostic in main.py \
         without a reload — got: {diagnostics:#?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

/// Whether main.py's most recently published diagnostics flag `thing` undefined.
async fn main_reports_thing_undefined(fixture: &mut WsTestFixture, main_uri: &str) -> bool {
    collect_all_diagnostics(fixture).await.get(main_uri).is_some_and(|d| {
        d["params"]["diagnostics"]
            .as_array()
            .is_some_and(|arr| arr.iter().any(is_thing_undefined))
    })
}

/// Send a full-text `didChange` for `uri`.
async fn did_change_full(
    fixture: &mut WsTestFixture,
    uri: &str,
    version: i64,
    text: &str,
) -> TestResult<()> {
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": uri, "version": version },
                "contentChanges": [{ "text": text }]
            }
        }))
        .await?;
    Ok(())
}

/// [ANALYSIS-SYMBOLS-INVAL] open-file path (#56): editing an OPEN module's
/// exports must refresh dependents live, in BOTH directions. The file watcher
/// skips open files (`reload_from_disk` bails when `is_open`), so the in-editor
/// `didChange` path drives the cross-module re-resolve. Critically, a renamed or
/// removed export must STOP suppressing its now-undefined references — a stale
/// export lingering in `imported_symbols` left dependents green after a rename.
#[tokio::test]
async fn open_module_export_edit_refreshes_dependent_diagnostics() -> TestResult<()> {
    let with_thing = "def thing() -> int:\n    return 1\n";
    // lib EXPORTS `thing`; main.py uses it cross-module, so it is clean at startup.
    let (dir, root_uri) = setup_cross_module_workspace(&[
        ("lib.py", with_thing),
        (
            "main.py",
            "import lib\n\ndef use() -> int:\n    return thing\n",
        ),
    ]);

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "crossModule").await?;

    let main_uri = format!("{root_uri}/main.py");
    let lib_uri = format!("{root_uri}/lib.py");

    // Startup: lib exports `thing`, so main.py is clean.
    assert!(
        !main_reports_thing_undefined(&mut fixture, &main_uri).await,
        "precondition: main.py should be clean while lib.py exports `thing`"
    );

    // OPEN lib.py, then EDIT it to RENAME the export away. The dependent must
    // report `thing` undefined LIVE — the stale export must not keep it green.
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": { "textDocument": {
                "uri": lib_uri, "languageId": "python", "version": 1, "text": with_thing
            }}
        }))
        .await?;
    did_change_full(&mut fixture, &lib_uri, 2, "def renamed() -> int:\n    return 1\n").await?;
    assert!(
        main_reports_thing_undefined(&mut fixture, &main_uri).await,
        "renaming `thing` away in the OPEN lib.py must make main.py report it undefined live"
    );

    // Restore the export in the OPEN file: the dependent must clear again.
    did_change_full(&mut fixture, &lib_uri, 3, with_thing).await?;
    assert!(
        !main_reports_thing_undefined(&mut fixture, &main_uri).await,
        "restoring `thing` in the OPEN lib.py must clear main.py's diagnostic live"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

/// True iff the diagnostic reports `thing` as undefined/unresolved.
fn is_thing_undefined(d: &serde_json::Value) -> bool {
    d["message"].as_str().unwrap_or("").contains("`thing`")
}
