//! Integration tests for BSK-E0054: Final type qualifier violations.
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

fn e0054_messages(diags: &[basilisk_checker::Diagnostic]) -> Vec<String> {
    diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0054")
        .map(|d| d.message.clone())
        .collect()
}

#[test]
fn e0054_module_final_reassignment_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Final
RATE: Final = 3000
RATE = 300
";
    let msgs = e0054_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "reassignment to module-level Final should fire E0054, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0054_module_final_no_reassignment_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Final
RATE: Final = 3000
";
    let msgs = e0054_messages(&run(source)?);
    assert!(
        msgs.is_empty(),
        "module Final with no reassignment should not fire E0054"
    );
    Ok(())
}

#[test]
fn e0054_class_final_attr_reassigned_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Final

class Config:
    DEFAULT_ID: Final[int] = 0

Config.DEFAULT_ID = 42
";
    let msgs = e0054_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "class Final attr reassignment should fire E0054, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0054_subclass_final_override_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Final

class Base:
    BORDER: Final[float] = 2.5

class Child(Base):
    BORDER = 3.0
";
    let msgs = e0054_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "subclass overriding Final attr should fire E0054, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0054_local_final_modification_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Final

def func() -> None:
    x: Final = 3
    x = 4
";
    let msgs = e0054_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "local Final modification should fire E0054, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0054_instance_final_outside_init_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Final

class MyClass:
    def other_method(self) -> None:
        self.x: Final = 1
";
    let msgs = e0054_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "instance Final outside __init__ should fire E0054, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0054_instance_final_in_init_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Final

class MyClass:
    def __init__(self) -> None:
        self.x: Final = 1
";
    let msgs = e0054_messages(&run(source)?);
    assert!(
        msgs.is_empty(),
        "instance Final in __init__ should not fire E0054, got: {msgs:?}"
    );
    Ok(())
}
