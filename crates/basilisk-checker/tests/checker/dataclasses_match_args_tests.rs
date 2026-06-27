//! Tests for [`dataclasses_match_args`] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
// Integration tests for dataclasses_match_args: dataclass `match_args=False`.

use super::common::*;

#[test]
fn match_args_false_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
fn match_args_true_ok() -> Result<(), Box<dyn std::error::Error>> {
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
        !codes(&diags).contains(&"dataclasses_match_args"),
        "default match_args=True should not fire E0059"
    );
    Ok(())
}
