// Integration tests for BSK-E0003: Missing variable type (unresolvable inference).

use super::common::*;

#[test]
fn e0003_empty_list_fires() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("items = []\n")?;
    assert!(
        codes(&diags).contains(&"BSK-E0003"),
        "unannotated empty list should fire E0003, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0003_empty_dict_fires() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("mapping = {}\n")?;
    assert!(
        codes(&diags).contains(&"BSK-E0003"),
        "unannotated empty dict should fire E0003, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0003_none_value_fires() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("result = None\n")?;
    assert!(
        codes(&diags).contains(&"BSK-E0003"),
        "unannotated None should fire E0003, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0003_annotated_empty_list_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("items: list[int] = []\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0003"),
        "annotated empty list should not fire E0003"
    );
    Ok(())
}

#[test]
fn e0003_annotated_none_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("result: int | None = None\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0003"),
        "annotated None should not fire E0003"
    );
    Ok(())
}

#[test]
fn e0003_non_empty_list_fires_strict() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("items = [1, 2, 3]\n")?;
    assert!(
        codes(&diags).contains(&"BSK-E0003"),
        "unannotated non-empty list should fire E0003 in strict mode, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0003_string_literal_fires_strict() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("name = \"hello\"\n")?;
    assert!(
        codes(&diags).contains(&"BSK-E0003"),
        "unannotated string literal should fire E0003 in strict mode, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0003_int_literal_fires() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("count = 42\n")?;
    assert!(
        codes(&diags).contains(&"BSK-E0003"),
        "unannotated int literal should fire E0003 (strict mode requires annotation), got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0003_string_literal_fires() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("label = \"hello\"\n")?;
    assert!(
        codes(&diags).contains(&"BSK-E0003"),
        "unannotated string literal should fire E0003 (strict mode requires annotation), got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0003_bool_literal_fires() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("flag = True\n")?;
    assert!(
        codes(&diags).contains(&"BSK-E0003"),
        "unannotated bool literal should fire E0003 (strict mode requires annotation), got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0003_non_empty_list_fires() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("items = [1, 2, 3]\n")?;
    assert!(
        codes(&diags).contains(&"BSK-E0003"),
        "unannotated non-empty list should fire E0003 (strict mode requires annotation), got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0003_call_expr_fires() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("result = some_function()\n")?;
    assert!(
        codes(&diags).contains(&"BSK-E0003"),
        "unannotated call expression should fire E0003 (strict mode requires annotation), got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0003_diagnostic_has_help() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("items = []\n")?;
    let e0003 = diags.iter().find(|d| d.code.code == "BSK-E0003");
    assert!(e0003.is_some(), "should fire E0003");
    let Some(diag) = e0003 else {
        return Err("E0003 diagnostic missing after assertion".into());
    };
    assert!(diag.help.is_some(), "E0003 should have help text");
    assert!(diag.note.is_some(), "E0003 should have note text");
    Ok(())
}
