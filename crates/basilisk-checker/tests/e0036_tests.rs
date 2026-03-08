//! Integration tests for BSK-E0036: ClassVar used in invalid context.
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

fn e0036_messages(diags: &[basilisk_checker::Diagnostic]) -> Vec<String> {
    diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0036")
        .map(|d| d.message.clone())
        .collect()
}

#[test]
fn e0036_classvar_in_class_body_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

class MyClass:
    count: ClassVar[int] = 0
"#;
    let msgs = e0036_messages(&run(source)?);
    assert!(
        msgs.is_empty(),
        "ClassVar in class body should not fire E0036, got: {:?}",
        msgs
    );
    Ok(())
}

#[test]
fn e0036_classvar_in_module_var_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar
bad: ClassVar[int] = 3
"#;
    let msgs = e0036_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "ClassVar at module level should fire E0036, got: {:?}",
        msgs
    );
    Ok(())
}

#[test]
fn e0036_classvar_in_function_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

class MyClass:
    def method(self, a: ClassVar[int]) -> None:
        pass
"#;
    let msgs = e0036_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "ClassVar in function param should fire E0036, got: {:?}",
        msgs
    );
    Ok(())
}

#[test]
fn e0036_classvar_in_return_type_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

class MyClass:
    def method(self) -> ClassVar[int]:
        return 0
"#;
    let msgs = e0036_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "ClassVar in return type should fire E0036, got: {:?}",
        msgs
    );
    Ok(())
}

#[test]
fn e0036_classvar_in_local_var_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

class MyClass:
    def method(self) -> None:
        x: ClassVar[str] = ""
"#;
    let msgs = e0036_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "ClassVar in local variable should fire E0036, got: {:?}",
        msgs
    );
    Ok(())
}

#[test]
fn e0036_nested_classvar_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar, Final

class MyClass:
    bad: Final[ClassVar[int]] = 3
"#;
    let msgs = e0036_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "nested ClassVar should fire E0036, got: {:?}",
        msgs
    );
    Ok(())
}

#[test]
fn e0036_classvar_in_list_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

class MyClass:
    bad: list[ClassVar[int]] = []
"#;
    let msgs = e0036_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "ClassVar nested in list should fire E0036, got: {:?}",
        msgs
    );
    Ok(())
}
