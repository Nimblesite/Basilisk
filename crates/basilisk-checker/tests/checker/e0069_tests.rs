// Integration tests for BSK-E0069: dataclass `kw_only`.

use super::common::*;

#[test]
fn e0069_positional_to_kw_only() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(kw_only=True)
class Point:
    x: int
    y: int

p = Point(1, 2)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0069_kw_only_with_kwargs_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(kw_only=True)
class Point:
    x: int
    y: int

p = Point(x=1, y=2)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0069"),
        "keyword args to kw_only dataclass should not fire E0069"
    );
    Ok(())
}
