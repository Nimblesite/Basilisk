// Integration tests for BSK-E0104: Cyclical type alias.

use super::common::*;

#[test]
fn e0104_non_cyclical_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias

IntList: TypeAlias = list[int]
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0104"),
        "non-cyclical alias should not fire E0104"
    );
    Ok(())
}
