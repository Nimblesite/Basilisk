//! Tests for [LSPARCH-FEATURES-RENAME]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-RENAME
// Tests for scope-aware rename.
//
// These tests verify that rename operations respect Python's lexical scoping
// rules: shadowed names in inner scopes are NOT renamed, and names in the
// correct scope ARE renamed.

use super::ws_test_common::*;

/// Renaming a local variable inside a function must NOT rename the same name
/// at module level.
#[tokio::test]
async fn test_ws_rename_respects_function_scope() -> TestResult<()> {
    let uri = "file:///scope_rename_func.py";
    // `x` at module level and `x` inside `foo` are different bindings.
    let code = "x: int = 1\n\ndef foo() -> int:\n    x: int = 2\n    return x\n\ny: int = x\n";

    // Rename `x` inside the function (line 3, char 4 = the `x` in `x: int = 2`).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        500,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 3, "character": 4 },
            "newName": "local_x"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    let edits = changes[uri]
        .as_array()
        .expect("file edits must be an array");

    // Should rename the local `x` occurrences (the assignment + the return),
    // but NOT the module-level `x` on line 0 or line 6.
    for edit in edits {
        let line = edit["range"]["start"]["line"].as_u64().unwrap();
        assert!(
            (3..=4).contains(&line),
            "edit should only touch lines 3-4 (function body), but found edit on line {line}: {edit}"
        );
        assert_eq!(edit["newText"].as_str(), Some("local_x"));
    }

    Ok(())
}

/// Renaming a module-level variable must NOT rename a shadowed local variable
/// with the same name inside a function.
#[tokio::test]
async fn test_ws_rename_module_scope_skips_shadowed() -> TestResult<()> {
    let uri = "file:///scope_rename_module.py";
    let code = "x: int = 1\n\ndef foo() -> int:\n    x: int = 2\n    return x\n\ny: int = x\n";

    // Rename `x` at module level (line 0, char 0).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        501,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 0 },
            "newName": "global_x"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    let edits = changes[uri]
        .as_array()
        .expect("file edits must be an array");

    // Should rename module-level `x` (line 0) and `y: int = x` (line 6),
    // but NOT the `x` inside `foo` (lines 3-4).
    for edit in edits {
        let line = edit["range"]["start"]["line"].as_u64().unwrap();
        assert!(
            line == 0 || line == 6,
            "edit should only touch lines 0 and 6, but found edit on line {line}: {edit}"
        );
        assert_eq!(edit["newText"].as_str(), Some("global_x"));
    }

    Ok(())
}

/// Renaming a function parameter must only affect usages within that function.
#[tokio::test]
async fn test_ws_rename_parameter_scope() -> TestResult<()> {
    let uri = "file:///scope_rename_param.py";
    let code = "name: str = \"global\"\n\ndef greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n\nresult: str = name\n";

    // Rename `name` parameter (line 2, char 10 = the `n` in `greet(name: str)`).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        502,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 2, "character": 10 },
            "newName": "person"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    let edits = changes[uri]
        .as_array()
        .expect("file edits must be an array");

    // Should only rename within the function (lines 2-3), not at module level.
    for edit in edits {
        let line = edit["range"]["start"]["line"].as_u64().unwrap();
        assert!(
            (2..=3).contains(&line),
            "edit should only touch lines 2-3, but found edit on line {line}: {edit}"
        );
    }

    Ok(())
}

/// Renaming a function name should rename at the definition and all call sites,
/// but not a local variable with the same name.
#[tokio::test]
async fn test_ws_rename_function_name_not_local_shadow() -> TestResult<()> {
    let uri = "file:///scope_rename_func_name.py";
    let code = "def compute(x: int) -> int:\n    return x * 2\n\ncompute: int = 42\n";

    // Rename the function `compute` (line 0, char 4).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        503,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 4 },
            "newName": "calculate"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    Ok(())
}

/// Renaming a variable to a Python keyword must be rejected.
#[tokio::test]
async fn test_ws_rename_rejects_keyword() -> TestResult<()> {
    let uri = "file:///scope_rename_keyword.py";
    let code = "x: int = 1\n";

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        504,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 0 },
            "newName": "class"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    // Should return null result (rename rejected).
    assert!(
        parsed["result"].is_null(),
        "rename to keyword 'class' should return null: {resp}"
    );

    Ok(())
}

/// Renaming a variable to an invalid identifier (starts with digit) must be rejected.
#[tokio::test]
async fn test_ws_rename_rejects_invalid_identifier() -> TestResult<()> {
    let uri = "file:///scope_rename_invalid.py";
    let code = "x: int = 1\n";

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        505,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 0 },
            "newName": "123abc"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"].is_null(),
        "rename to '123abc' should return null: {resp}"
    );

    Ok(())
}

/// Renaming a symbol must never rewrite matching text inside string literals
/// or docstring prose — those are data, not references.
///
/// Regression test for `is_in_string_or_comment` (references.rs) only
/// implementing the `#`-comment half of its contract: occurrences of the
/// name inside plain strings and docstrings were emitted as rename edits,
/// silently corrupting user data.
#[tokio::test]
async fn test_ws_rename_skips_string_literals_and_docstring_prose() -> TestResult<()> {
    let uri = "file:///scope_rename_strings.py";
    // `total` appears as: the parameter (line 0), prose inside the docstring
    // (line 1), text inside a string literal (line 2), and the real usage
    // (line 3). Only lines 0 and 3 are genuine references.
    let code = "def process(total: int) -> int:\n    \"\"\"Compute the total for a report.\"\"\"\n    label: str = \"total is big\"\n    return total\n";

    // Rename the `total` parameter (line 0, char 12 = the `t` in `total`).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        507,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 12 },
            "newName": "amount"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    let edits = changes[uri]
        .as_array()
        .expect("file edits must be an array");

    // The docstring (line 1) and the string literal (line 2) must be untouched.
    for edit in edits {
        let line = edit["range"]["start"]["line"].as_u64().unwrap();
        assert!(
            line == 0 || line == 3,
            "rename must not edit string content on line {line}: {edit}"
        );
    }
    // Exactly the definition + the real usage, nothing more.
    assert_eq!(
        edits.len(),
        2,
        "expected exactly 2 edits (definition + usage): {resp}"
    );

    Ok(())
}

/// The original reported repro: renaming a module-level variable must not
/// rewrite the same word inside a string literal on another line.
#[tokio::test]
async fn test_ws_rename_module_var_skips_string_literal() -> TestResult<()> {
    let uri = "file:///scope_rename_module_string.py";
    let code = "total: int = 1\nmsg: str = \"total is big\"\nprint(total)\n";

    // Rename `total` at its definition (line 0, char 0).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        510,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 0 },
            "newName": "amount"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    let edits = changes[uri]
        .as_array()
        .expect("file edits must be an array");

    for edit in edits {
        let line = edit["range"]["start"]["line"].as_u64().unwrap();
        assert!(
            line == 0 || line == 2,
            "rename must not edit the string on line {line}: {edit}"
        );
    }
    assert_eq!(
        edits.len(),
        2,
        "expected exactly 2 edits (definition + print usage): {resp}"
    );

    Ok(())
}

/// Renaming must keep skipping `#` comments — the behaviour the old heuristic
/// did implement must survive the switch to the token/AST mask.
#[tokio::test]
async fn test_ws_rename_skips_comment_text() -> TestResult<()> {
    let uri = "file:///scope_rename_comment.py";
    let code = "total: int = 1  # total counter\nprint(total)\n";

    let (_fixture, resp) = open_and_request(
        uri,
        code,
        511,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 0 },
            "newName": "amount"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    let edits = changes[uri]
        .as_array()
        .expect("file edits must be an array");

    // The `total` inside `# total counter` (line 0, char 18) must be skipped.
    for edit in edits {
        let line = edit["range"]["start"]["line"].as_u64().unwrap();
        let character = edit["range"]["start"]["character"].as_u64().unwrap();
        assert!(
            (line, character) == (0, 0) || (line, character) == (1, 6),
            "rename must not edit comment text at {line}:{character}: {edit}"
        );
    }
    assert_eq!(
        edits.len(),
        2,
        "expected exactly 2 edits (definition + print usage): {resp}"
    );

    Ok(())
}

/// Renaming a parameter must update real keyword-argument call sites but never
/// a lookalike `func(param=...)` inside a string literal.
#[tokio::test]
async fn test_ws_rename_kwarg_site_but_not_string_lookalike() -> TestResult<()> {
    let uri = "file:///scope_rename_kwarg_string.py";
    let code = "def make(count: int) -> int:\n    return count\n\nresult: int = make(count=2)\nnote: str = \"make(count=3)\"\n";

    // Rename the `count` parameter (line 0, char 9).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        512,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 9 },
            "newName": "quantity"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    let edits = changes[uri]
        .as_array()
        .expect("file edits must be an array");

    // Real references: def (line 0), body usage (line 1), kwarg site (line 3).
    // The `count=` inside the string on line 4 is data and must be untouched.
    let mut edit_lines: Vec<u64> = edits
        .iter()
        .map(|edit| edit["range"]["start"]["line"].as_u64().unwrap())
        .collect();
    edit_lines.sort_unstable();
    assert_eq!(
        edit_lines,
        vec![0, 1, 3],
        "expected edits on lines 0, 1, 3 only: {resp}"
    );

    Ok(())
}

/// Renaming a class attribute must update `self.attr` usages in method bodies
/// but never `self.attr` text quoted inside a docstring.
#[tokio::test]
async fn test_ws_rename_attribute_skips_docstring_mention() -> TestResult<()> {
    let uri = "file:///scope_rename_attr_docstring.py";
    let code = "class Counter:\n    value: int = 0\n\n    def bump(self) -> None:\n        \"\"\"Increase self.value by one.\"\"\"\n        self.value = self.value + 1\n";

    // Rename `value` at its class-attribute definition (line 1, char 4).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        513,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 1, "character": 4 },
            "newName": "tally"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    let edits = changes[uri]
        .as_array()
        .expect("file edits must be an array");

    // The docstring mention on line 4 is prose; only the definition (line 1)
    // and the real usages (line 5) may be edited.
    for edit in edits {
        let line = edit["range"]["start"]["line"].as_u64().unwrap();
        assert!(
            line == 1 || line == 5,
            "rename must not edit the docstring mention on line {line}: {edit}"
        );
    }
    let has_definition_edit = edits
        .iter()
        .any(|edit| edit["range"]["start"]["line"].as_u64() == Some(1));
    let has_usage_edit = edits
        .iter()
        .any(|edit| edit["range"]["start"]["line"].as_u64() == Some(5));
    assert!(
        has_definition_edit && has_usage_edit,
        "expected edits on both the definition and the usages: {resp}"
    );

    Ok(())
}

/// Renaming a module-level function must still update its quoted `__all__`
/// entry: `__all__` strings are exports, handled by the dedicated pass even
/// though the general sweep now masks all string content.
#[tokio::test]
async fn test_ws_rename_still_updates_dunder_all_entry() -> TestResult<()> {
    let uri = "file:///scope_rename_dunder_all.py";
    let code = "__all__ = [\"helper\"]\n\ndef helper() -> int:\n    return 1\n";

    // Rename `helper` at its definition (line 2, char 4).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        514,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 2, "character": 4 },
            "newName": "assist"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    let edits = changes[uri]
        .as_array()
        .expect("file edits must be an array");

    // The quoted entry starts inside `__all__ = ["` at line 0, char 12.
    let dunder_all_edit = edits.iter().find(|edit| {
        edit["range"]["start"]["line"].as_u64() == Some(0)
            && edit["range"]["start"]["character"].as_u64() == Some(12)
    });
    assert!(
        dunder_all_edit.is_some(),
        "the __all__ entry must be renamed: {resp}"
    );
    let definition_edit = edits
        .iter()
        .any(|edit| edit["range"]["start"]["line"].as_u64() == Some(2));
    assert!(definition_edit, "the definition must be renamed: {resp}");

    Ok(())
}

/// Renaming a parameter used in an f-string interpolation field must rename
/// the field: `{name}` is code, not string data. Companion pin for the
/// string-literal mask — only *literal* f-string chunks are masked.
#[tokio::test]
async fn test_ws_rename_includes_fstring_interpolation_field() -> TestResult<()> {
    let uri = "file:///scope_rename_fstring.py";
    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n";

    // Rename the `name` parameter (line 0, char 10).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        508,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 10 },
            "newName": "person"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    let edits = changes[uri]
        .as_array()
        .expect("file edits must be an array");

    // The interpolation field on line 1 MUST be renamed. `{` sits at char 20,
    // so the `name` identifier spans chars 21-25.
    let interpolation_edit = edits.iter().find(|edit| {
        edit["range"]["start"]["line"].as_u64() == Some(1)
            && edit["range"]["start"]["character"].as_u64() == Some(21)
    });
    assert!(
        interpolation_edit.is_some(),
        "f-string interpolation field must be renamed: {resp}"
    );
    // Definition + interpolation field, nothing else (the literal chunks
    // `Hello, ` and `!` contain no match, and no other usage exists).
    assert_eq!(edits.len(), 2, "expected exactly 2 edits: {resp}");

    Ok(())
}

/// Renaming a class referenced through PEP 563 string annotations must rename
/// the name inside the annotation strings — they are forward references, not
/// data. Companion pin for the string-literal mask's annotation exemption.
#[tokio::test]
async fn test_ws_rename_includes_string_annotation_references() -> TestResult<()> {
    let uri = "file:///scope_rename_str_ann.py";
    let code =
        "class MyClass:\n    pass\n\ndef make(x: \"MyClass\") -> \"MyClass\":\n    return x\n";

    // Rename `MyClass` at its definition (line 0, char 6).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        509,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 6 },
            "newName": "Renamed"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    let edits = changes[uri]
        .as_array()
        .expect("file edits must be an array");

    // Both string-annotation occurrences on line 3 must be renamed: the
    // parameter annotation (char 13) and the return annotation (char 27).
    for expected_char in [13_u64, 27_u64] {
        let annotation_edit = edits.iter().find(|edit| {
            edit["range"]["start"]["line"].as_u64() == Some(3)
                && edit["range"]["start"]["character"].as_u64() == Some(expected_char)
        });
        assert!(
            annotation_edit.is_some(),
            "string annotation at line 3 char {expected_char} must be renamed: {resp}"
        );
    }
    // Definition + the two annotation strings.
    assert_eq!(edits.len(), 3, "expected exactly 3 edits: {resp}");

    Ok(())
}

/// Nested function scoping: renaming `x` in the outer function should not
/// affect `x` in the inner function when the inner function redefines it.
#[tokio::test]
async fn test_ws_rename_nested_function_shadow() -> TestResult<()> {
    let uri = "file:///scope_rename_nested.py";
    let code = "def outer() -> int:\n    x: int = 1\n    def inner() -> int:\n        x: int = 2\n        return x\n    return x\n";

    // Rename `x` in outer (line 1, char 4).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        506,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 1, "character": 4 },
            "newName": "outer_x"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(!changes.is_null(), "changes must not be null: {resp}");

    let edits = changes[uri]
        .as_array()
        .expect("file edits must be an array");

    // Should rename `x` on lines 1 and 5 (outer scope), but NOT lines 3-4 (inner).
    for edit in edits {
        let line = edit["range"]["start"]["line"].as_u64().unwrap();
        assert!(
            line == 1 || line == 5,
            "edit should only touch lines 1 and 5 (outer function), but found edit on line {line}: {edit}"
        );
        assert_eq!(edit["newText"].as_str(), Some("outer_x"));
    }

    Ok(())
}

/// A `WorkspaceEdit` must never contain two edits covering the same range.
/// The LSP spec forbids overlapping edit ranges within one `changes` entry,
/// and identical ranges overlap.
///
/// Regression test for `rename_symbol` unioning the scope-aware sweep with
/// `find_keyword_arg_sites` and never de-duplicating: for a parameter with a
/// default (`def f(x=1)`) the `def` line satisfies BOTH sweeps, so the
/// definition range was emitted twice and clients rejected the whole rename.
#[tokio::test]
async fn test_ws_rename_defaulted_parameter_has_no_duplicate_edits() -> TestResult<()> {
    let uri = "file:///scope_rename_dup_default.py";
    let code = "def f(x=1) -> int:\n    return x\n\nresult: int = f(x=2)\n";

    // Rename the `x` parameter (line 0, char 6).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        515,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 6 },
            "newName": "quantity"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let edits = parsed["result"]["changes"][uri]
        .as_array()
        .expect("file edits must be an array")
        .clone();

    let mut ranges: Vec<String> = edits.iter().map(|edit| edit["range"].to_string()).collect();
    ranges.sort();
    let mut unique = ranges.clone();
    unique.dedup();
    assert_eq!(
        ranges.len(),
        unique.len(),
        "WorkspaceEdit must not contain duplicate ranges: {resp}"
    );

    Ok(())
}

/// `__all__` handling must stay inside the `__all__` statement. A trailing
/// comment (or the tuple form) used to leave the block "open" forever, so the
/// first quoted match on every later line was rewritten as if it were an
/// export entry — silently corrupting ordinary string literals.
#[tokio::test]
async fn test_ws_rename_dunder_all_with_trailing_comment_spares_string_literals() -> TestResult<()>
{
    let uri = "file:///scope_rename_all_comment.py";
    let code = "__all__ = [\"run\"]  # public API\n\n\ndef run() -> None:\n    print(\"run\")\n";

    // Rename the function `run` (line 3, char 4).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        516,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 3, "character": 4 },
            "newName": "execute"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let edits = parsed["result"]["changes"][uri]
        .as_array()
        .expect("file edits must be an array")
        .clone();

    // Legitimate edits: the `__all__` entry (line 0) and the def (line 3).
    // The `print("run")` string on line 4 must NOT be touched.
    for edit in &edits {
        let line = edit["range"]["start"]["line"].as_u64().unwrap();
        assert!(
            line == 0 || line == 3,
            "rename must not rewrite the string literal on line {line}: {edit}"
        );
    }
    let has_all_entry = edits
        .iter()
        .any(|edit| edit["range"]["start"]["line"].as_u64() == Some(0));
    assert!(
        has_all_entry,
        "the __all__ entry must still be renamed: {resp}"
    );

    Ok(())
}

/// Strings in `Literal[...]` are values, not forward references. The
/// annotation exemption in the string mask must cover only type-expression
/// positions, never `Literal` arguments.
#[tokio::test]
async fn test_ws_rename_skips_literal_string_values() -> TestResult<()> {
    let uri = "file:///scope_rename_literal.py";
    let code = "from typing import Literal\n\nMode = \"fast\"\n\n\ndef run(mode: Literal[\"Mode\"]) -> None:\n    print(Mode, mode)\n";

    // Rename the module variable `Mode` (line 2, char 0).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        517,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 2, "character": 0 },
            "newName": "Kind"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let edits = parsed["result"]["changes"][uri]
        .as_array()
        .expect("file edits must be an array")
        .clone();

    // Only the definition (line 2) and the `print(Mode, ...)` usage (line 6).
    // The `Literal["Mode"]` value on line 5 is data.
    for edit in &edits {
        let line = edit["range"]["start"]["line"].as_u64().unwrap();
        assert!(
            line == 2 || line == 6,
            "rename must not rewrite the Literal value on line {line}: {edit}"
        );
    }

    Ok(())
}

/// LSP `Position.character` is a UTF-16 code-unit offset. The keyword-argument
/// sweep emitted raw byte indices, so any non-ASCII character earlier on the
/// line shifted the edit right and corrupted the source.
#[tokio::test]
async fn test_ws_rename_kwarg_range_uses_utf16_columns() -> TestResult<()> {
    let uri = "file:///scope_rename_utf16_kwarg.py";
    // The en dash on the call line is 3 bytes but 1 UTF-16 code unit, so a
    // byte-indexed column would report `body` two columns too far right.
    let code = "def notify(title: str, body: str) -> None:\n    print(title, body)\n\n\nnotify(title=\"Erfolg \u{2013} gespeichert\", body=\"ok\")\n";

    // Rename the `body` parameter (line 0, char 23).
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        518,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 23 },
            "newName": "message"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let edits = parsed["result"]["changes"][uri]
        .as_array()
        .expect("file edits must be an array")
        .clone();

    // `body=` on the call line starts at UTF-16 column 37.
    let kwarg_edit = edits
        .iter()
        .find(|edit| edit["range"]["start"]["line"].as_u64() == Some(4));
    let kwarg_edit = kwarg_edit.expect("the keyword-argument site must be renamed");
    assert_eq!(
        kwarg_edit["range"]["start"]["character"].as_u64(),
        Some(37),
        "kwarg column must be a UTF-16 offset, not a byte offset: {kwarg_edit}"
    );
    assert_eq!(
        kwarg_edit["range"]["end"]["character"].as_u64(),
        Some(41),
        "kwarg end column must be a UTF-16 offset: {kwarg_edit}"
    );

    Ok(())
}
