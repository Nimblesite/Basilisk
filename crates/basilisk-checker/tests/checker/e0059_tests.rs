// Integration tests for BSK-E0059: dataclass `match_args=False`.

use super::common::*;

#[test]
fn e0059_match_args_false_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(match_args=False)
class Point:
    x: int
    y: int

match Point(1, 2):
    case Point(x, y):
        pass
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0059_match_args_true_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass
class Point:
    x: int
    y: int

match Point(1, 2):
    case Point(x, y):
        pass
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0059"),
        "default match_args=True should not fire E0059"
    );
    Ok(())
}
