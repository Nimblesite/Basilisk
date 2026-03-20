// Integration tests for BSK-E0040 (enum subclassing) and BSK-E0046 (enum member annotated).

use super::common::*;

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
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0040");
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
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0040");
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
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0040");
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
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0046");
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
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0046");
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
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0046");
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
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0046");
    assert!(
        msgs.is_empty(),
        "annotated attribute in non-enum class should not fire E0046"
    );
    Ok(())
}
