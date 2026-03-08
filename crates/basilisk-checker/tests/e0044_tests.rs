//! Integration tests for BSK-E0044: Final used in invalid position.
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

fn e0044_messages(diags: &[basilisk_checker::Diagnostic]) -> Vec<String> {
    diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0044")
        .map(|d| d.message.clone())
        .collect()
}

#[test]
fn e0044_valid_module_final_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final
X: Final[int] = 42
"#;
    let msgs = e0044_messages(&run(source)?);
    assert!(
        msgs.is_empty(),
        "valid module-level Final should not fire E0044, got: {:?}",
        msgs
    );
    Ok(())
}

#[test]
fn e0044_final_in_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final
def f(x: Final[int]) -> None:
    pass
"#;
    let msgs = e0044_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "Final in function parameter should fire E0044, got: {:?}",
        msgs
    );
    Ok(())
}

#[test]
fn e0044_final_nested_in_list_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final
x: list[Final[int]] = []
"#;
    let msgs = e0044_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "Final nested in list should fire E0044, got: {:?}",
        msgs
    );
    Ok(())
}

#[test]
fn e0044_classvar_with_final_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar, Final

class MyClass:
    VALUE: ClassVar[Final[int]] = 1
"#;
    let msgs = e0044_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "ClassVar[Final[...]] should fire E0044, got: {:?}",
        msgs
    );
    Ok(())
}

#[test]
fn e0044_bare_final_no_assignment_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final
BAD: Final
"#;
    let msgs = e0044_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "bare Final with no assignment should fire E0044, got: {:?}",
        msgs
    );
    Ok(())
}

#[test]
fn e0044_final_too_many_type_args_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final
BAD: Final[str, int] = ""
"#;
    let msgs = e0044_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "Final[str, int] should fire E0044, got: {:?}",
        msgs
    );
    Ok(())
}

#[test]
fn e0044_final_in_class_body_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final

class MyClass:
    VALUE: Final[int] = 42
"#;
    let msgs = e0044_messages(&run(source)?);
    assert!(
        msgs.is_empty(),
        "Final in class body should not fire E0044, got: {:?}",
        msgs
    );
    Ok(())
}
