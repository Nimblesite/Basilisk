#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0051: Invalid Literal parameterization.
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

fn e0051_messages(diags: &[basilisk_checker::Diagnostic]) -> Vec<String> {
    diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0051")
        .map(|d| d.message.clone())
        .collect()
}

#[test]
fn e0051_valid_int_literal_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
x: Literal[1] = 1
";
    let msgs = e0051_messages(&run(source)?);
    assert!(msgs.is_empty(), "valid Literal[1] should not fire E0051");
    Ok(())
}

#[test]
fn e0051_valid_str_literal_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal
x: Literal["hello"] = "hello"
"#;
    let msgs = e0051_messages(&run(source)?);
    assert!(
        msgs.is_empty(),
        "valid Literal[\"hello\"] should not fire E0051"
    );
    Ok(())
}

#[test]
fn e0051_valid_bool_literal_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
x: Literal[True] = True
";
    let msgs = e0051_messages(&run(source)?);
    assert!(msgs.is_empty(), "valid Literal[True] should not fire E0051");
    Ok(())
}

#[test]
fn e0051_valid_none_literal_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
x: Literal[None] = None
";
    let msgs = e0051_messages(&run(source)?);
    assert!(msgs.is_empty(), "valid Literal[None] should not fire E0051");
    Ok(())
}

#[test]
fn e0051_float_literal_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
x: Literal[3.14] = 3.14
";
    let msgs = e0051_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "Literal[3.14] should fire E0051, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0051_bare_literal_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
x: Literal
";
    let msgs = e0051_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "bare Literal should fire E0051, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0051_type_object_in_literal_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
x: Literal[int]
";
    let msgs = e0051_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "Literal[int] should fire E0051, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0051_ellipsis_in_literal_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
x: Literal[...]
";
    let msgs = e0051_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "Literal[...] should fire E0051, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0051_valid_negative_int_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
x: Literal[-1] = -1
";
    let msgs = e0051_messages(&run(source)?);
    assert!(msgs.is_empty(), "Literal[-1] should not fire E0051");
    Ok(())
}

#[test]
fn e0051_valid_enum_member_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
from enum import Enum

class Color(Enum):
    RED = 1

x: Literal[Color.RED]
";
    let msgs = e0051_messages(&run(source)?);
    assert!(msgs.is_empty(), "Literal[Color.RED] should not fire E0051");
    Ok(())
}
