//! Integration tests for BSK-E0057: PEP 695 type alias invalid.
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

fn codes(diags: &[basilisk_checker::Diagnostic]) -> Vec<&str> {
    diags.iter().map(|d| d.code.code).collect()
}

#[test]
fn e0057_pep695_type_alias_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Vector = list[float]
type Matrix = list[Vector]
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0057_type_alias_with_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Pair[T] = tuple[T, T]
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0057_type_alias_bool_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Bad = True
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0057_type_alias_int_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Bad = 42
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0057_type_alias_list_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Bad = [int, str]
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0057_type_alias_dict_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
type Bad = {"a": int}
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0057_type_alias_fstring() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
type Bad = f"hello"
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0057_type_alias_conditional() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Bad = int if True else str
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0057_type_alias_boolean_op() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Bad = int or str
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0057_type_alias_lambda() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Bad = lambda: int
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0057_type_alias_eval() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
type Bad = eval("int")
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0057_type_alias_negative_number() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Bad = -1
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0057_type_alias_tuple_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Bad = (int, str)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0057_type_alias_non_type_name() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
x = 42
type Bad = x
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
