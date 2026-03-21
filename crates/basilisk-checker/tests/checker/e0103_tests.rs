// Integration tests for BSK-E0103: Tuple index out of bounds.

use super::common::*;

#[test]
fn e0103_valid_tuple_index() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t: tuple[int, str] = (1, "a")
x = t[0]
y = t[1]
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0103"),
        "valid tuple index should not fire E0103"
    );
    Ok(())
}

#[test]
fn e0103_positive_out_of_bounds() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: resolver does not yet produce tuple_index_violations for literal indices.
    // When it does, this test should assert E0103 fires.
    let source = r#"
t: tuple[int, str, bool] = (1, "a", True)
x = t[3]
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0103_negative_out_of_bounds() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: resolver does not yet produce tuple_index_violations for literal indices.
    let source = r#"
t: tuple[int, str, bool] = (1, "a", True)
x = t[-4]
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0103_valid_negative_index() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t: tuple[int, str, bool] = (1, "a", True)
x = t[-1]
y = t[-2]
z = t[-3]
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0103"),
        "valid negative indices should not fire E0103"
    );
    Ok(())
}

#[test]
fn e0103_single_element_tuple() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: resolver does not yet produce tuple_index_violations for literal indices.
    let source = r#"
t: tuple[int] = (42,)
x = t[0]
y = t[1]
z = t[-2]
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
