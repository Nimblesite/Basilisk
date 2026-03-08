//! Integration tests for BSK-E0040 (enum subclassing) and BSK-E0046 (enum member annotated).
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

fn messages_for(diags: &[basilisk_checker::Diagnostic], code: &str) -> Vec<String> {
    diags
        .iter()
        .filter(|d| d.code.code == code)
        .map(|d| d.message.clone())
        .collect()
}

// --- E0040: Enum with members cannot be subclassed ---

#[test]
fn e0040_subclass_enum_with_members_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum

class Color(Enum):
    RED = 1
    GREEN = 2

class ExtendedColor(Color):
    BLUE = 3
";
    let msgs = messages_for(&run(source)?, "BSK-E0040");
    assert!(
        msgs.iter()
            .any(|m| m.contains("Cannot subclass") && m.contains("Color")),
        "subclassing enum with members should fire E0040, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0040_subclass_memberless_enum_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum

class BaseEnum(Enum):
    pass

class Child(BaseEnum):
    VALUE = 1
";
    let msgs = messages_for(&run(source)?, "BSK-E0040");
    assert!(
        msgs.is_empty(),
        "subclassing memberless enum should not fire E0040, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0040_non_enum_class_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Base:
    x = 1

class Child(Base):
    pass
";
    let msgs = messages_for(&run(source)?, "BSK-E0040");
    assert!(
        msgs.is_empty(),
        "non-enum subclassing should not fire E0040"
    );
    Ok(())
}

// --- E0046: Enum member annotated ---

#[test]
fn e0046_annotated_enum_member_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum

class Pet(Enum):
    DOG: int = 2
";
    let msgs = messages_for(&run(source)?, "BSK-E0046");
    assert!(
        msgs.iter()
            .any(|m| m.contains("should not have an explicit type annotation")),
        "annotated enum member should fire E0046, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0046_unannotated_enum_member_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from enum import Enum

class Pet(Enum):
    CAT = "felis"
"#;
    let msgs = messages_for(&run(source)?, "BSK-E0046");
    assert!(
        msgs.is_empty(),
        "unannotated enum member should not fire E0046"
    );
    Ok(())
}

#[test]
fn e0046_annotation_only_non_member_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from enum import Enum

class Pet(Enum):
    genus: str
";
    let msgs = messages_for(&run(source)?, "BSK-E0046");
    assert!(
        msgs.is_empty(),
        "annotation-only non-member should not fire E0046"
    );
    Ok(())
}

#[test]
fn e0046_non_enum_class_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Regular:
    x: int = 5
";
    let msgs = messages_for(&run(source)?, "BSK-E0046");
    assert!(
        msgs.is_empty(),
        "annotated attribute in non-enum class should not fire E0046"
    );
    Ok(())
}
