//! Tests for [`aliases_type_statement`] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
// Integration tests for aliases_type_statement: PEP 695 type alias invalid.

use super::common::*;

#[test]
fn pep695_type_alias_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Vector = list[float]
type Matrix = list[Vector]
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn type_alias_with_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Pair[T] = tuple[T, T]
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn type_alias_bool_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Bad = True
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn type_alias_int_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Bad = 42
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn type_alias_list_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Bad = [int, str]
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn type_alias_dict_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
type Bad = {"a": int}
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn type_alias_fstring() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
type Bad = f"hello"
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn type_alias_conditional() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Bad = int if True else str
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn type_alias_boolean_op() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Bad = int or str
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn type_alias_lambda() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Bad = lambda: int
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn type_alias_eval() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
type Bad = eval("int")
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn type_alias_negative_number() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Bad = -1
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn type_alias_tuple_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Bad = (int, str)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn type_alias_non_type_name() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
x = 42
type Bad = x
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
