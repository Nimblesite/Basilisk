// Integration tests for BSK-E0005: Missing class attribute type annotation.

use super::common::*;

#[test]
fn e0005_unannotated_class_attr_with_literal_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = "class Foo:\n    value = 42\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0005"),
        "scalar literal class attr should NOT fire E0005 (type is inferrable), got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0005_annotated_class_attr_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "class Foo:\n    value: int = 42\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0005"),
        "annotated class attr should not fire E0005"
    );
    Ok(())
}

#[test]
fn e0005_enum_class_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from enum import Enum\n\nclass Color(Enum):\n    RED = 1\n    GREEN = 2\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0005"),
        "Enum class should be exempt from E0005"
    );
    Ok(())
}

#[test]
fn e0005_protocol_class_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source =
        "from typing import Protocol\n\nclass MyProto(Protocol):\n    name = \"default\"\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0005"),
        "Protocol class should be exempt from E0005"
    );
    Ok(())
}

#[test]
fn e0005_namedtuple_class_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import NamedTuple\n\nclass Point(NamedTuple):\n    x = 0\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0005"),
        "NamedTuple class should be exempt from E0005"
    );
    Ok(())
}

#[test]
fn e0005_subclass_overriding_annotated_parent_attr_exempt() -> Result<(), Box<dyn std::error::Error>>
{
    let source = "\
class BaseRoute:
    priority: int = 10

class AdminRoute(BaseRoute):
    priority = 100
";
    let diags = run(source)?;
    let e0005_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0005")
        .collect();
    assert!(
        e0005_diags.is_empty(),
        "subclass overriding an annotated parent attribute should not fire E0005, got: {:?}",
        e0005_diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn e0005_subclass_unannotated_new_attr_literal_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class BaseRoute:
    priority: int = 10

class AdminRoute(BaseRoute):
    priority = 100
    new_attr = 42
";
    let diags = run(source)?;
    let e0005_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0005")
        .collect();
    assert!(
        e0005_diags.is_empty(),
        "literal attrs should not fire E0005 even in subclasses, got: {:?}",
        e0005_diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn e0005_multiple_unannotated_attrs() -> Result<(), Box<dyn std::error::Error>> {
    let source = "class Foo:\n    a = 1\n    b = 2\n    c = 3\n";
    let diags = run(source)?;
    let count = diags.iter().filter(|d| d.code.code == "BSK-E0005").count();
    assert_eq!(
        count, 0,
        "scalar literal attrs should NOT fire E0005 (type is inferrable)"
    );
    Ok(())
}

#[test]
fn e0005_scalar_literal_attrs_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class Config:
    retries = 3
    label = \"default\"
    threshold = 0.5
    verbose = True
    magic = b\"\\x00\"
    nothing = None
";
    let diags = run(source)?;
    let e0005_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0005")
        .collect();
    assert!(
        e0005_diags.is_empty(),
        "class attrs with scalar literal values should not fire E0005 (type is inferrable), got: {:?}",
        e0005_diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn e0005_non_inferrable_rhs_still_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class Foo:
    result = some_function()
    computed = x + y
";
    let diags = run(source)?;
    let e0005_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0005")
        .collect();
    assert!(
        !e0005_diags.is_empty(),
        "non-inferrable RHS attrs should still fire E0005"
    );
    Ok(())
}
