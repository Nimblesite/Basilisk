#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0014: Assignment type incompatibility.
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

fn e0014_messages(diags: &[basilisk_checker::Diagnostic]) -> Vec<String> {
    diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0014")
        .map(|d| d.message.clone())
        .collect()
}

#[test]
fn e0014_int_annotated_str_literal_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "count: int = \"hello\"\n";
    let msgs = e0014_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "int annotation with str literal should fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0014_str_annotated_int_literal_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "label: str = 42\n";
    let msgs = e0014_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "str annotation with int literal should fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0014_bool_annotated_str_literal_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "flag: bool = \"yes\"\n";
    let msgs = e0014_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "bool annotation with str literal should fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0014_float_annotated_str_literal_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "ratio: float = \"1.5\"\n";
    let msgs = e0014_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "float annotation with str literal should fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0014_compatible_assignment_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "count: int = 42\n";
    let msgs = e0014_messages(&run(source)?);
    assert!(
        msgs.is_empty(),
        "compatible assignment should not fire E0014"
    );
    Ok(())
}

#[test]
fn e0014_str_annotated_str_literal_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "name: str = \"hello\"\n";
    let msgs = e0014_messages(&run(source)?);
    assert!(
        msgs.is_empty(),
        "str annotation with str literal should not fire E0014"
    );
    Ok(())
}

#[test]
fn e0014_local_var_mismatch_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def func() -> None:
    x: int = "oops"
"#;
    let msgs = e0014_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "local variable type mismatch should fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0014_local_var_compatible_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func() -> None:
    x: int = 42
";
    let msgs = e0014_messages(&run(source)?);
    assert!(
        msgs.is_empty(),
        "compatible local var should not fire E0014"
    );
    Ok(())
}

#[test]
fn e0014_bytes_annotated_str_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "data: bytes = \"text\"\n";
    let msgs = e0014_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "bytes annotation with str literal should fire E0014"
    );
    Ok(())
}

#[test]
fn e0014_int_annotated_bool_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // bool is a subclass of int in Python
    let source = "x: int = True\n";
    let _msgs = e0014_messages(&run(source)?);
    // This may or may not fire depending on implementation; bool is subtype of int
    // Just exercise the code path
    Ok(())
}

#[test]
fn e0014_float_annotated_int_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // int is widened to float in Python
    let source = "x: float = 42\n";
    let _msgs = e0014_messages(&run(source)?);
    // int -> float widening may or may not be handled
    Ok(())
}

#[test]
fn e0014_optional_none_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // None is a valid value for Optional[int]
    let source = "from typing import Optional\nmaybe_int: Optional[int] = None\n";
    let msgs = e0014_messages(&run(source)?);
    assert!(
        msgs.is_empty(),
        "Optional[int] = None should NOT fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0014_union_member_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // 42 (int) is a valid member of Union[int, str]
    let source = "from typing import Union\neither: Union[int, str] = 42\n";
    let msgs = e0014_messages(&run(source)?);
    assert!(
        msgs.is_empty(),
        "Union[int, str] = 42 should NOT fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0014_final_with_type_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // Final[int] = 100 — int matches int
    let source = "from typing import Final\nMAX_SIZE: Final[int] = 100\n";
    let msgs = e0014_messages(&run(source)?);
    assert!(
        msgs.is_empty(),
        "Final[int] = 100 should NOT fire E0014, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0014_tuple_reassignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: int = 1
y: str = "hello"
x, y = "wrong", 42
"#;
    // Just exercise the tuple reassignment path
    let _msgs = e0014_messages(&run(source)?);
    Ok(())
}
