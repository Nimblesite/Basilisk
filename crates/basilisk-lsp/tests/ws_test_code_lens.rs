#![allow(dead_code)]

mod ws_test_common;
use ws_test_common::*;

#[tokio::test]
async fn test_ws_code_lens() -> TestResult<()> {
    let code = "\
def greet(name: str) -> str:
    return name

x = greet(\"hello\")
y = greet(\"world\")
";
    let (_fixture, resp) = open_and_request(
        "file:///ws_code_lens.py",
        code,
        400,
        "textDocument/codeLens",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_code_lens.py" }
        }),
    )
    .await?;

    // The function `greet` is called twice (line 4 + line 5), so 2 references.
    assert!(
        resp.contains("2 references"),
        "codeLens should show '2 references' for greet: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_code_lens_class_references() -> TestResult<()> {
    let code = "\
class Animal:
    name: str

class Dog(Animal):
    breed: str

def make_animal() -> Animal:
    return Animal()

x: Animal = make_animal()
";
    let (_fixture, resp) = open_and_request(
        "file:///ws_code_lens_class_refs.py",
        code,
        401,
        "textDocument/codeLens",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_code_lens_class_refs.py" }
        }),
    )
    .await?;

    // `Animal` appears in: definition, Dog(Animal), -> Animal, Animal(), x: Animal = 5 total => 4 references.
    assert!(
        resp.contains("4 references"),
        "codeLens should show '4 references' for Animal: {resp}"
    );
    // `Dog` is defined but never used elsewhere => 0 references.
    assert!(
        resp.contains("0 references"),
        "codeLens should show '0 references' for Dog: {resp}"
    );
    // `make_animal` is called once => 1 reference.
    assert!(
        resp.contains("1 reference"),
        "codeLens should show '1 reference' for make_animal: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_code_lens_single_reference() -> TestResult<()> {
    let code = "\
def helper(x: int) -> int:
    return x

result: int = helper(42)
";
    let (_fixture, resp) = open_and_request(
        "file:///ws_code_lens_single_ref.py",
        code,
        402,
        "textDocument/codeLens",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_code_lens_single_ref.py" }
        }),
    )
    .await?;

    // `helper` is called once (line 4), so singular "1 reference".
    assert!(
        resp.contains("1 reference"),
        "codeLens should show singular '1 reference' for helper: {resp}"
    );
    // Must NOT show "1 references" (plural).
    assert!(
        !resp.contains("1 references"),
        "codeLens must use singular form '1 reference', not '1 references': {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_code_lens_no_references() -> TestResult<()> {
    let code = "\
def unused_func(x: int) -> int:
    return x
";
    let (_fixture, resp) = open_and_request(
        "file:///ws_code_lens_no_refs.py",
        code,
        403,
        "textDocument/codeLens",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_code_lens_no_refs.py" }
        }),
    )
    .await?;

    // `unused_func` is never called, so 0 references.
    assert!(
        resp.contains("0 references"),
        "codeLens should show '0 references' for unused function: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_code_lens_methods_excluded() -> TestResult<()> {
    let code = "\
class MyClass:
    def method_one(self) -> None:
        pass

    def method_two(self) -> None:
        self.method_one()
";
    let (_fixture, resp) = open_and_request(
        "file:///ws_code_lens_methods.py",
        code,
        404,
        "textDocument/codeLens",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_code_lens_methods.py" }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let lenses = parsed["result"]
        .as_array()
        .ok_or("codeLens result should be an array")?;

    // Only `MyClass` should get a lens; methods should be excluded.
    // method_one and method_two are inside a class, so they must not appear.
    assert_eq!(
        lenses.len(),
        1,
        "only the class should get a code lens, not methods: {resp}"
    );

    // The single lens should be for MyClass.
    let title = lenses[0]["command"]["title"]
        .as_str()
        .ok_or("lens should have a title")?;
    assert!(
        title.contains("references"),
        "the single lens should be a reference count for MyClass: {title}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_code_lens_multiple_functions() -> TestResult<()> {
    let code = "\
def alpha(x: int) -> int:
    return x

def beta(y: int) -> int:
    return alpha(y)

def gamma(z: int) -> int:
    return beta(alpha(z))
";
    let (_fixture, resp) = open_and_request(
        "file:///ws_code_lens_multi.py",
        code,
        405,
        "textDocument/codeLens",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_code_lens_multi.py" }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let lenses = parsed["result"]
        .as_array()
        .ok_or("codeLens result should be an array")?;

    // Three top-level functions => three lenses.
    assert_eq!(
        lenses.len(),
        3,
        "each top-level function should get its own code lens: {resp}"
    );

    // Collect titles in order (alpha, beta, gamma).
    let titles: Vec<&str> = lenses
        .iter()
        .filter_map(|lens| lens["command"]["title"].as_str())
        .collect();

    assert_eq!(titles.len(), 3, "all three lenses should have titles");

    // alpha is called in beta (line 5) and gamma (line 8) => 2 references.
    assert_eq!(
        titles[0], "2 references",
        "alpha should have 2 references: {resp}"
    );
    // beta is called in gamma (line 8) => 1 reference.
    assert_eq!(
        titles[1], "1 reference",
        "beta should have 1 reference: {resp}"
    );
    // gamma is never called => 0 references.
    assert_eq!(
        titles[2], "0 references",
        "gamma should have 0 references: {resp}"
    );

    Ok(())
}
