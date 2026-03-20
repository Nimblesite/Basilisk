// Integration tests for BSK-E0068: Literal string enum.

use super::common::*;

#[test]
fn e0068_literal_string_enum_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from enum import StrEnum
class Status(StrEnum):
    ACTIVE = "active"
    INACTIVE = "inactive"

def func(s: Status) -> None:
    pass

func("active")
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
