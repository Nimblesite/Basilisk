#![doc = "Tests for BSK-E0061: Invalid literal string vs enum member mismatch."]
//! Tests for BSK-E0061: Invalid literal string vs enum member mismatch.
//!
//! This rule detects when a `Literal["X.Y"]` annotation (string) is used
//! where `Literal[X.Y]` (enum member) is expected, or vice versa.

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run_e2e(src: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(src.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

#[test]
fn test_e0061_literal_string_vs_enum_member() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
from enum import Enum

class Color(Enum):
    RED = 1
    GREEN = 2
    BLUE = 3

def process_color(c: Literal[Color.RED]) -> None:
    pass

x: Literal['Color.RED'] = Color.RED
";
    let diags = run_e2e(src)?;
    let e0061: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0061")
        .collect();
    assert!(!e0061.is_empty(), "literal string vs enum member mismatch should fire E0061");
    Ok(())
}

#[test]
fn test_e0061_valid_literal_enum_member() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
from enum import Enum

class Color(Enum):
    RED = 1
    GREEN = 2
    BLUE = 3

def process_color(c: Literal[Color.RED]) -> None:
    pass

x: Literal[Color.RED] = Color.RED
";
    let diags = run_e2e(src)?;
    let e0061: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0061")
        .collect();
    assert!(e0061.is_empty(), "valid literal enum member should not fire E0061");
    Ok(())
}

#[test]
fn test_e0061_literal_string_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
from enum import Enum

class Color(Enum):
    RED = 1
    GREEN = 2
    BLUE = 3

def get_color() -> Literal[Color.RED]:
    return Color.RED

result: Literal['Color.RED'] = get_color()
";
    let diags = run_e2e(src)?;
    let e0061: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0061")
        .collect();
    assert!(!e0061.is_empty(), "literal string assignment mismatch should fire E0061");
    Ok(())
}

#[test]
fn test_e0061_nested_enum_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
from enum import Enum

class Status(Enum):
    PENDING = 'pending'
    APPROVED = 'approved'
    REJECTED = 'rejected'

def handle_status(s: Literal[Status.PENDING]) -> None:
    pass

# This should trigger E0061
status_var: Literal['Status.PENDING'] = Status.PENDING
";
    let diags = run_e2e(src)?;
    let e0061: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0061")
        .collect();
    assert!(!e0061.is_empty(), "nested enum mismatch should fire E0061");
    Ok(())
}

#[test]
fn test_e0061_function_param_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
from enum import Enum

class Direction(Enum):
    NORTH = 'n'
    SOUTH = 's'
    EAST = 'e'
    WEST = 'w'

def navigate(d: Literal[Direction.NORTH]) -> None:
    pass

# This call should trigger E0061
navigate(Direction.NORTH)  # Should be fine
";
    let diags = run_e2e(src)?;
    let e0061: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0061")
        .collect();
    // This test expects no error since the function call uses the actual enum member
    assert!(e0061.is_empty(), "function call with enum member should not fire E0061");
    Ok(())
}

#[test]
fn test_e0061_complex_enum_hierarchy() -> Result<(), Box<dyn std::error::Error>> {
    let src = "
from enum import Enum

class VehicleType(Enum):
    CAR = 'car'
    TRUCK = 'truck'
    MOTORCYCLE = 'motorcycle'

class Color(Enum):
    RED = 'red'
    BLUE = 'blue'
    GREEN = 'green'

def process_vehicle(vtype: Literal[VehicleType.CAR], color: Literal[Color.RED]) -> None:
    pass

# Mixed usage that should trigger E0061
vehicle: Literal['VehicleType.CAR'] = VehicleType.CAR
paint: Literal[Color.RED] = Color.RED
";
    let diags = run_e2e(src)?;
    let e0061: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0061")
        .collect();
    assert!(!e0061.is_empty(), "complex enum hierarchy mismatch should fire E0061");
    Ok(())
}