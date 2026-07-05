//! Tests for [LSPARCH-FEATURES-DEFINITION]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-DEFINITION
// Tests for LSP: `ws_test_goto_definition`.

use super::ws_test_common::*;

#[tokio::test]
async fn test_ws_goto_definition_function() -> TestResult<()> {
    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n";
    let (_fixture, resp) = open_and_request(
        "file:///ws_gotodef.py",
        code,
        310,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_gotodef.py" },
            "position": { "line": 0, "character": 4 }
        }),
    )
    .await?;

    assert!(
        resp.contains("ws_gotodef.py"),
        "definition should point to same file: {resp}"
    );

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "definition result must not be null: {resp}"
    );
    let start = &parsed["result"]["range"]["start"];
    assert_eq!(start["line"], 0, "definition must be on line 0: {resp}");
    assert_eq!(
        start["character"], 4,
        "definition must start at char 4, where 'greet' begins: {resp}"
    );

    // Hardened: verify URI matches the opened file exactly
    let uri = parsed["result"]["uri"].as_str().unwrap_or("");
    assert_eq!(
        uri, "file:///ws_gotodef.py",
        "definition URI must match the opened document: {resp}"
    );

    // Hardened: verify the range is non-empty (end differs from start)
    let end = &parsed["result"]["range"]["end"];
    assert!(
        start["line"] != end["line"] || start["character"] != end["character"],
        "definition range must be non-empty (start != end): {resp}"
    );

    // Hardened: verify end character is beyond start (for a single-line range)
    if start["line"] == end["line"] {
        assert!(
            end["character"].as_u64() > start["character"].as_u64(),
            "definition end character must be > start character on same line: {resp}"
        );
    }

    // Hardened: verify JSON-RPC envelope
    assert_eq!(
        parsed["jsonrpc"], "2.0",
        "must be valid JSON-RPC 2.0: {resp}"
    );
    assert_eq!(
        parsed["id"], 310,
        "response id must match request id: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_definition_class() -> TestResult<()> {
    let code = "class Dog:\n    name: str\n    def bark(self) -> str:\n        return \"woof\"\n";
    let (_fixture, resp) = open_and_request(
        "file:///ws_gotoclass.py",
        code,
        311,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_gotoclass.py" },
            "position": { "line": 0, "character": 6 }
        }),
    )
    .await?;

    assert!(
        resp.contains("ws_gotoclass.py"),
        "definition should point to same file: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_definition_from_call_site() -> TestResult<()> {
    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n\nresult: str = greet(\"world\")\n";
    // Line 3: "result: str = greet(\"world\")" — 'g' of call "greet" at character 14.
    let (_fixture, resp) = open_and_request(
        "file:///ws_goto_call.py",
        code,
        312,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_goto_call.py" },
            "position": { "line": 3, "character": 14 }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "goto-def from call site must resolve: {resp}"
    );
    let start = &parsed["result"]["range"]["start"];
    assert_eq!(
        start["line"], 0,
        "goto-def from call should jump to line 0: {resp}"
    );
    assert_eq!(
        start["character"], 4,
        "goto-def from call should land at char 4 where 'greet' is defined: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_def_no_symbol_returns_null() -> TestResult<()> {
    let code = "x: int = 42\n";
    // Goto definition on whitespace / non-symbol position.
    let (_fixture, resp) = open_and_request(
        "file:///ws_edge_gotodef.py",
        code,
        401,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_edge_gotodef.py" },
            "position": { "line": 5, "character": 0 }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"].is_null(),
        "goto-def on non-symbol position should return null: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_definition_class_usage() -> TestResult<()> {
    // "Animal" is defined on line 0, used as a type annotation on line 3.
    let code = "\
class Animal:
    name: str

def greet(pet: Animal) -> str:
    return pet.name
";
    let (_fixture, resp) = open_and_request(
        "file:///ws_goto_class_usage.py",
        code,
        903,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_goto_class_usage.py" },
            // Goto definition on "Animal" in the type annotation on line 3.
            // "def greet(pet: Animal)" — 'A' of "Animal" is at character 15.
            "position": { "line": 3, "character": 15 }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "goto-def on class usage must resolve: {resp}"
    );

    // Should jump to line 0 where "class Animal:" is defined.
    // "class " is 6 chars, so 'Animal' starts at character 6.
    let start = &parsed["result"]["range"]["start"];
    assert_eq!(
        start["line"], 0,
        "goto-def from class usage should jump to line 0: {resp}"
    );
    assert_eq!(
        start["character"], 6,
        "goto-def from class usage should land at char 6 where 'Animal' is defined: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_definition_variable() -> TestResult<()> {
    let code = "x: int = 42\n";
    // Goto definition on "x" — line 0, character 0.
    let (_fixture, resp) = open_and_request(
        "file:///ws_goto_var.py",
        code,
        952,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_goto_var.py" },
            "position": { "line": 0, "character": 0 }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "goto-def on variable must resolve: {resp}"
    );
    let start = &parsed["result"]["range"]["start"];
    assert_eq!(
        start["line"], 0,
        "variable definition should be on line 0: {resp}"
    );
    assert_eq!(
        start["character"], 0,
        "variable definition should start at char 0: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_definition_module_var_use_inside_function() -> TestResult<()> {
    // Field report (follow-up to #199): cmd+click on a module-level constant
    // used inside a function body must jump to the module-level definition,
    // exactly like parameter and local-variable uses do.
    let code = "\"\"\"doc\"\"\"\n\nfrom typing import Final\n\nPI: Final = 3.14\n\n\ndef scaled(factor: int) -> float:\n    result = PI * factor\n    return result\n";
    let (_fixture, resp) = open_and_request(
        "file:///ws_goto_module_const_use.py",
        code,
        961,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_goto_module_const_use.py" },
            // Line 8: `    result = PI * factor` — the `PI` use at chars 13..15.
            "position": { "line": 8, "character": 13 }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "goto-def on a module-constant use must resolve: {resp}"
    );
    let start = &parsed["result"]["range"]["start"];
    assert_eq!(
        start["line"], 4,
        "PI's definition is on line 4 (`PI: Final = 3.14`): {resp}"
    );
    assert_eq!(
        start["character"], 0,
        "PI's definition starts at char 0: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_definition_parameter_use() -> TestResult<()> {
    // Regression for the goto hammer "parameter use resolves to the parameter"
    // (#200): a use of a function parameter must resolve to the parameter's
    // declaration. `find_definition_by_name` previously did not search params.
    let code = "def calculate(operand: int) -> int:\n    return operand * operand\n";
    let (_fixture, resp) = open_and_request(
        "file:///ws_goto_param.py",
        code,
        960,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_goto_param.py" },
            // Line 1: "    return operand * operand" — first `operand` at char 11.
            "position": { "line": 1, "character": 13 }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "goto-def on a parameter use must resolve: {resp}"
    );
    let start = &parsed["result"]["range"]["start"];
    assert_eq!(
        start["line"], 0,
        "parameter definition is on line 0 (the def line): {resp}"
    );
    assert_eq!(
        start["character"], 14,
        "parameter `operand` begins at char 14 in `def calculate(operand...`: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_definition_attribute_use() -> TestResult<()> {
    // Goto hammer "attribute use resolves to the attribute def": a `self.attr`
    // read must resolve to the attribute's declaration in the class body.
    let code = "\
class Widget:
    width: int = 10
    def resize(self) -> int:
        return self.width
";
    let (_fixture, resp) = open_and_request(
        "file:///ws_goto_attr.py",
        code,
        961,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_goto_attr.py" },
            // Line 3: "        return self.width" — `width` after `self.` at char 20.
            "position": { "line": 3, "character": 22 }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "goto-def on an attribute use must resolve: {resp}"
    );
    let start = &parsed["result"]["range"]["start"];
    assert_eq!(
        start["line"], 1,
        "attribute `width` is declared on line 1: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_declaration() -> TestResult<()> {
    let code = "\
def compute(x: int) -> int:
    return x * 2

result: int = compute(10)
";
    let (_fixture, resp) = open_and_request(
        "file:///decl.py",
        code,
        300,
        "textDocument/declaration",
        serde_json::json!({
            "textDocument": { "uri": "file:///decl.py" },
            "position": { "line": 3, "character": 16 }
        }),
    )
    .await?;

    assert!(
        resp.contains("\"line\":0"),
        "declaration should point to line 0 (function def): {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_type_definition_variable() -> TestResult<()> {
    let code = "\
class MyData:
    value: int

instance: MyData = MyData()
";
    let (_fixture, resp) = open_and_request(
        "file:///typedef.py",
        code,
        301,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": "file:///typedef.py" },
            "position": { "line": 3, "character": 2 }
        }),
    )
    .await?;

    assert!(
        resp.contains("\"line\":0"),
        "type definition should point to line 0 (class MyData): {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_type_definition_parameter() -> TestResult<()> {
    let code = "\
class Config:
    debug: bool

def process(cfg: Config) -> None:
    pass
";
    let (_fixture, resp) = open_and_request(
        "file:///typedef2.py",
        code,
        302,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": "file:///typedef2.py" },
            "position": { "line": 3, "character": 13 }
        }),
    )
    .await?;

    assert!(
        resp.contains("\"line\":0"),
        "type definition should point to line 0 (class Config): {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_type_definition_optional() -> TestResult<()> {
    let code = "\
class Widget:
    name: str

item: Optional[Widget] = None
";
    let (_fixture, resp) = open_and_request(
        "file:///typedef3.py",
        code,
        303,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": "file:///typedef3.py" },
            "position": { "line": 3, "character": 2 }
        }),
    )
    .await?;

    assert!(
        resp.contains("\"line\":0"),
        "type definition should unwrap Optional and point to class Widget: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_type_definition_no_annotation_returns_null() -> TestResult<()> {
    let code = "\
x = 42
";
    let (_fixture, resp) = open_and_request(
        "file:///typedef4.py",
        code,
        304,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": "file:///typedef4.py" },
            "position": { "line": 0, "character": 0 }
        }),
    )
    .await?;

    assert!(
        resp.contains("\"result\":null"),
        "type definition for unannotated variable should be null: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_definition_member_through_aliased_stub_import() -> TestResult<()> {
    // The user's exact #180 case: an aliased import of a module backed by a
    // `.pyi` STUB (`import datetime as _dt`, datetime.pyi) — clicking a member
    // (`_dt.datetime`, here `g.area`) must jump into the stub. The #180 alias
    // capture made cross-module resolution treat the aliased import as a
    // `from`-import and publish nothing, returning `result: null`; cbee4ff
    // restores it by discriminating on `kind`. Proven to fail (null) without the
    // fix and resolve with it.
    let dir = unique_temp_dir("bsk_goto_stub_alias");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("geometry.pyi"),
        "def area(r: float) -> float: ...\n",
    )?;
    let main_src = "import geometry as g\n\n\ndef use() -> float:\n    return g.area(1.0)\n";
    std::fs::write(dir.join("main.py"), main_src)?;
    let root_uri = format!("file://{}", dir.display());
    let main_uri = format!("file://{}", dir.join("main.py").display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "crossModule").await?;
    for _ in 0..20 {
        let msg = tokio::time::timeout(Duration::from_millis(500), fixture.ws_read.next()).await;
        if msg.is_err() {
            break;
        }
    }
    fixture.did_open(&main_uri, main_src).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // main.py line 4: `    return g.area(1.0)` — `area` begins at character 13.
    let resp = fixture
        .request(
            730,
            "textDocument/definition",
            serde_json::json!({
                "textDocument": { "uri": main_uri },
                "position": { "line": 4, "character": 14 }
            }),
        )
        .await?
        .ok_or("no response to member goto-def through aliased stub import")?;
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "member goto-def through an aliased .pyi-stub import must resolve: {resp}"
    );
    assert!(
        parsed["result"]["uri"]
            .as_str()
            .unwrap_or("")
            .contains("geometry.pyi"),
        "goto-def should jump into the stub geometry.pyi: {resp}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_definition_cross_file_function_whole_module() -> TestResult<()> {
    // Regression for the goto hammer cross-file failures in the SHIPPED default
    // mode: cross-file go-to-definition must work in `wholeModule` (the
    // extension's default), not only in `crossModule`. Cross-module symbol
    // population is gated to crossModule, so navigation has to resolve the
    // import on demand via its resolved_path. Same scenario as the crossModule
    // test below, but proves the default-mode user gets a working cmd+click.
    // Mirror the VS Code harness exactly: an EMPTY workspace root, with the
    // fixtures living in a SEPARATE directory that is merely opened (not under
    // any registered root). Cross-file resolution must still work via the
    // importer's own directory + the open-document index.
    let root = unique_temp_dir("bsk_goto_cross_whole_root");
    std::fs::create_dir_all(&root)?;
    let dir = unique_temp_dir("bsk_goto_cross_whole_files");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("helpers.py"),
        "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n",
    )?;
    std::fs::write(
        dir.join("main.py"),
        "from helpers import greet\n\nresult: str = greet(\"world\")\n",
    )?;

    let root_uri = format!("file://{}", root.display());
    let helpers_uri = format!("file://{}", dir.join("helpers.py").display());
    let main_uri = format!("file://{}", dir.join("main.py").display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "wholeModule").await?;
    for _ in 0..20 {
        let msg = tokio::time::timeout(Duration::from_millis(500), fixture.ws_read.next()).await;
        if msg.is_err() {
            break;
        }
    }
    // Open the target first so the importer can resolve it cross-file, mirroring
    // the VS Code goto hammer ("Helper first so the subject's import resolves").
    fixture
        .did_open(
            &helpers_uri,
            "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n",
        )
        .await?;
    fixture
        .did_open(
            &main_uri,
            "from helpers import greet\n\nresult: str = greet(\"world\")\n",
        )
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // main.py line 2: "result: str = greet("world")" — 'greet' at character 14.
    let resp = fixture
        .request(
            760,
            "textDocument/definition",
            serde_json::json!({
                "textDocument": { "uri": main_uri },
                "position": { "line": 2, "character": 14 }
            }),
        )
        .await?
        .ok_or("no response to wholeModule cross-file goto definition")?;
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "wholeModule cross-file goto-def must resolve (default-mode cmd+click): {resp}"
    );
    assert!(
        parsed["result"]["uri"]
            .as_str()
            .unwrap_or("")
            .contains("helpers.py"),
        "wholeModule cross-file goto-def should jump to helpers.py: {resp}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_definition_cross_file_function() -> TestResult<()> {
    // Set up a workspace with two files: helpers.py defines `greet`,
    // main.py imports and uses it.
    let dir = unique_temp_dir("bsk_goto_cross_file");
    std::fs::create_dir_all(&dir)?;

    std::fs::write(
        dir.join("helpers.py"),
        "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n",
    )?;
    std::fs::write(
        dir.join("main.py"),
        "from helpers import greet\n\nresult: str = greet(\"world\")\n",
    )?;

    let root_uri = format!("file://{}", dir.display());
    let main_uri = format!("file://{}", dir.join("main.py").display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "crossModule").await?;

    // Drain startup scan messages (diagnostics for both files).
    for _ in 0..20 {
        let msg = tokio::time::timeout(Duration::from_millis(500), fixture.ws_read.next()).await;
        if msg.is_err() {
            break;
        }
    }

    // Go to definition on `greet` at the call site in main.py.
    // Line 2: "result: str = greet("world")" — 'g' of "greet" at character 14.
    let resp = fixture
        .request(
            500,
            "textDocument/definition",
            serde_json::json!({
                "textDocument": { "uri": main_uri },
                "position": { "line": 2, "character": 14 }
            }),
        )
        .await?
        .ok_or("no response to cross-file goto definition")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "cross-file goto-def must resolve: {resp}"
    );

    // Should jump to helpers.py, not main.py.
    let result_uri = parsed["result"]["uri"].as_str().unwrap_or("");
    assert!(
        result_uri.contains("helpers.py"),
        "cross-file goto-def should jump to helpers.py, got: {result_uri}"
    );

    // Should land at the `greet` function definition: line 0, character 4.
    let start = &parsed["result"]["range"]["start"];
    assert_eq!(
        start["line"], 0,
        "cross-file goto-def should land on line 0 of helpers.py: {resp}"
    );
    assert_eq!(
        start["character"], 4,
        "cross-file goto-def should land at char 4 where 'greet' is defined: {resp}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_definition_member_through_dotted_aliased_import() -> TestResult<()> {
    // Regression (follow-up to #180): clicking a member accessed through a
    // *dotted, aliased* import — `import pkg.sub as ps` → `ps.helper(...)`, the
    // same shape as `import os.path as _osp` → `_osp.join(...)` — must jump to
    // the member's cross-file definition. The #180 alias capture briefly routed
    // such imports as `from`-imports and published no symbols; cbee4ff restores
    // it by discriminating on `kind`.
    let dir = unique_temp_dir("bsk_goto_dotted_alias");
    std::fs::create_dir_all(dir.join("pkg"))?;
    std::fs::write(dir.join("pkg/__init__.py"), "")?;
    std::fs::write(
        dir.join("pkg/sub.py"),
        "def helper(x: int) -> int:\n    return x\n",
    )?;
    let main_src = "import pkg.sub as ps\n\n\ndef use() -> int:\n    return ps.helper(1)\n";
    std::fs::write(dir.join("main.py"), main_src)?;
    let root_uri = format!("file://{}", dir.display());
    let main_uri = format!("file://{}", dir.join("main.py").display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "crossModule").await?;
    for _ in 0..20 {
        let msg = tokio::time::timeout(Duration::from_millis(500), fixture.ws_read.next()).await;
        if msg.is_err() {
            break;
        }
    }
    fixture.did_open(&main_uri, main_src).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // main.py line 4: `    return ps.helper(1)` — `helper` begins at character 14.
    let resp = fixture
        .request(
            720,
            "textDocument/definition",
            serde_json::json!({
                "textDocument": { "uri": main_uri },
                "position": { "line": 4, "character": 16 }
            }),
        )
        .await?
        .ok_or("no response to member goto-def through dotted aliased import")?;
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "member goto-def through a dotted aliased import must resolve: {resp}"
    );
    let result_uri = parsed["result"]["uri"].as_str().unwrap_or("");
    assert!(
        result_uri.contains("sub.py"),
        "goto-def should jump to pkg/sub.py, got: {result_uri}"
    );
    assert_eq!(
        parsed["result"]["range"]["start"]["line"], 0,
        "goto-def should land on `helper` at line 0 of pkg/sub.py: {resp}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_type_definition_cross_file_class() -> TestResult<()> {
    // Goto hammer "type-def: variable resolves to cross-file class type": a
    // variable annotated with an imported class must resolve via typeDefinition
    // to the class's cross-file declaration.
    let dir = unique_temp_dir("bsk_typedef_cross");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("models.py"), "class Dog:\n    name: str\n")?;
    let main_src = "from models import Dog\n\ninstance: Dog = Dog()\n";
    std::fs::write(dir.join("main.py"), main_src)?;
    let root_uri = format!("file://{}", dir.display());
    let main_uri = format!("file://{}", dir.join("main.py").display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "crossModule").await?;
    for _ in 0..20 {
        let msg = tokio::time::timeout(Duration::from_millis(500), fixture.ws_read.next()).await;
        if msg.is_err() {
            break;
        }
    }
    fixture.did_open(&main_uri, main_src).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // main.py line 2: "instance: Dog = Dog()" — `instance` at char 0.
    let resp = fixture
        .request(
            740,
            "textDocument/typeDefinition",
            serde_json::json!({
                "textDocument": { "uri": main_uri },
                "position": { "line": 2, "character": 2 }
            }),
        )
        .await?
        .ok_or("no response to cross-file type definition")?;
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "cross-file type-def must resolve to the imported class: {resp}"
    );
    assert!(
        parsed["result"]["uri"]
            .as_str()
            .unwrap_or("")
            .contains("models.py"),
        "cross-file type-def should jump to models.py: {resp}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_definition_cross_file_class() -> TestResult<()> {
    // Set up a workspace: models.py defines `Dog`, app.py imports and uses it.
    let dir = unique_temp_dir("bsk_goto_cross_class");
    std::fs::create_dir_all(&dir)?;

    std::fs::write(
        dir.join("models.py"),
        "class Dog:\n    name: str\n    def bark(self) -> str:\n        return \"woof\"\n",
    )?;
    std::fs::write(
        dir.join("app.py"),
        "from models import Dog\n\npet: Dog = Dog()\n",
    )?;

    let root_uri = format!("file://{}", dir.display());
    let app_uri = format!("file://{}", dir.join("app.py").display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "crossModule").await?;

    // Drain startup messages.
    for _ in 0..20 {
        let msg = tokio::time::timeout(Duration::from_millis(500), fixture.ws_read.next()).await;
        if msg.is_err() {
            break;
        }
    }

    // Go to definition on `Dog` usage at line 2: "pet: Dog = Dog()"
    // First `Dog` (type annotation) starts at character 5.
    let resp = fixture
        .request(
            501,
            "textDocument/definition",
            serde_json::json!({
                "textDocument": { "uri": app_uri },
                "position": { "line": 2, "character": 5 }
            }),
        )
        .await?
        .ok_or("no response to cross-file goto definition for class")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "cross-file goto-def for class must resolve: {resp}"
    );

    let result_uri = parsed["result"]["uri"].as_str().unwrap_or("");
    assert!(
        result_uri.contains("models.py"),
        "cross-file goto-def should jump to models.py, got: {result_uri}"
    );

    // `Dog` class name starts at character 6 ("class Dog:")
    let start = &parsed["result"]["range"]["start"];
    assert_eq!(
        start["line"], 0,
        "cross-file goto-def should land on line 0 of models.py: {resp}"
    );
    assert_eq!(
        start["character"], 6,
        "cross-file goto-def should land at char 6 where 'Dog' is defined: {resp}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
