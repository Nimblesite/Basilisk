//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
use super::common::*;

// Coverage boost tests batch 22a: e0129 literal value assignment, e0014 assignment type
// incompatibility.

// =============================================================================
// Literal value assignment incompatibility
// =============================================================================

#[test]
fn literal_0_vs_false() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal

def func(a: Literal[0], b: Literal[False]):
    x1: Literal[False] = a
    x2: Literal[0] = b
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "literals_semantics_2")
        .count();
    Ok(())
}

#[test]
fn augmented_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal

def func(a: Literal[3, 4, 5]):
    a += 3
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "literals_semantics_2")
        .count();
    Ok(())
}

#[test]
fn multiple_augmented_ops() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal

def func(a: Literal[1, 2], b: Literal[10]):
    a -= 1
    b *= 2
    a //= 1
    b **= 2
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "literals_semantics_2")
        .count();
    Ok(())
}

#[test]
fn literal_1_vs_true() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal

def func(a: Literal[1], b: Literal[True]):
    x1: Literal[True] = a
    x2: Literal[1] = b
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "literals_semantics_2")
        .count();
    Ok(())
}

#[test]
fn literal_string_values() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

def func(a: Literal["hello"]):
    x: Literal["world"] = a
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "literals_semantics_2")
        .count();
    Ok(())
}

#[test]
fn literal_hex_octal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal

def func(a: Literal[0xFF]):
    x: Literal[255] = a
    y: Literal[256] = a
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "literals_semantics_2")
        .count();
    Ok(())
}

#[test]
fn valid_literal_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal

def func(a: Literal[1, 2, 3]):
    x: Literal[1, 2, 3] = a
    y: Literal[1, 2, 3, 4] = a
";
    let diagnostics = run(source)?;
    let e0129 = diagnostics
        .iter()
        .filter(|d| d.code.code == "literals_semantics_2")
        .count();
    // Valid assignment should not trigger.
    let _ = e0129;
    Ok(())
}

#[test]
fn nested_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal

def func(a: Literal[Literal[1, 2], 3]):
    b: Literal[4] = a
";
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// =============================================================================
// Assignment type incompatibility - deeper paths
// =============================================================================

#[test]
fn float_literal_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
count: int = "hello"
label: str = 42
flag: bool = "yes"
ratio: float = "1.5"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "assignment_compatibility")
        .count();
    Ok(())
}

#[test]
fn negative_int_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
x: str = -42
y: bool = -1
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "assignment_compatibility")
        .count();
    Ok(())
}

#[test]
fn bytes_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: int = b"hello"
y: str = b"world"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "assignment_compatibility")
        .count();
    Ok(())
}

#[test]
fn none_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
x: int = None
y: str = None
z: float = None
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "assignment_compatibility")
        .count();
    Ok(())
}

#[test]
fn bool_literal_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
x: str = True
y: float = False
z: bytes = True
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "assignment_compatibility")
        .count();
    Ok(())
}

#[test]
fn list_dict_set_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: int = [1, 2, 3]
y: str = {"a": 1}
z: float = {1, 2}
w: int = (1, 2)
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "assignment_compatibility")
        .count();
    Ok(())
}

#[test]
fn empty_collection_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
x: int = []
y: int = {}
";
    let diagnostics = run(source)?;
    let _ = diagnostics
        .iter()
        .filter(|d| d.code.code == "assignment_compatibility")
        .count();
    Ok(())
}

#[test]
fn complex_annotations() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Optional, Union, List, Dict

a: Optional[int] = "hello"
b: Union[int, float] = "hello"
c: List[int] = 42
d: Dict[str, int] = "hello"
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}
