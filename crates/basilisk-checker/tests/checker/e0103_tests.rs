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
