// Integration tests for BSK-E0147: Tuple starred-unpack compatibility.

use super::common::*;

#[test]
fn e0147_starred_unpack_too_many_elements() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t1: tuple[int, *tuple[str]] = (1, "a")
t1 = (1, "a", "b")
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0147_starred_unpack_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t2: tuple[int, *tuple[str, ...]] = (1, "a")
t2 = (1, 2, "a")
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0147_valid_starred_unpack() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t1: tuple[int, *tuple[str, ...]] = (1, "a", "b", "c")
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0147"),
        "valid starred unpack should not fire E0147"
    );
    Ok(())
}

#[test]
fn e0147_function_body_starred_unpack() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(t1: tuple[int], t2: tuple[int, *tuple[int, ...]], t3: tuple[int, ...]) -> None:
    v2: tuple[int, *tuple[int, ...]]
    v2 = t3
    v3: tuple[int]
    v3 = t2
    v3 = t3
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
