// Integration tests for BSK-E0025: Missing @override decorator.

use super::common::*;

#[test]
fn e0025_missing_override_decorator_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Base:
    def process(self) -> None:
        pass

class Child(Base):
    def process(self) -> None:
        pass
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0025"),
        "overriding method without @override should fire E0025, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0025_with_override_decorator_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import override

class Base:
    def process(self) -> None:
        pass

class Child(Base):
    @override
    def process(self) -> None:
        pass
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0025"),
        "method with @override should not fire E0025"
    );
    Ok(())
}

#[test]
fn e0025_different_method_name_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Base:
    def process(self) -> None:
        pass

class Child(Base):
    def other_method(self) -> None:
        pass
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0025"),
        "different method name should not fire E0025"
    );
    Ok(())
}

#[test]
fn e0025_protocol_class_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class MyProto(Protocol):
    def method(self) -> None: ...

class Impl(MyProto):
    def method(self) -> None:
        pass
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0025"),
        "Protocol implementation should be exempt from E0025"
    );
    Ok(())
}
