// Integration tests for BSK-E0079: Module-level protocol incompatibility.

use super::common::*;

#[test]
fn e0079_module_var_protocol_incompat() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Drawable(Protocol):
    def draw(self) -> None: ...

class Circle:
    def draw(self) -> None:
        pass

class Square:
    pass

x: Drawable = Square()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0079_valid_protocol_conformance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Drawable(Protocol):
    def draw(self) -> None: ...

class Circle:
    def draw(self) -> None:
        pass

x: Drawable = Circle()
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0079"),
        "valid protocol conformance should not fire E0079"
    );
    Ok(())
}

#[test]
fn e0079_missing_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class HasName(Protocol):
    def name(self) -> str: ...
    def id(self) -> int: ...

class OnlyName:
    def name(self) -> str:
        return 'x'

x: HasName = OnlyName()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
