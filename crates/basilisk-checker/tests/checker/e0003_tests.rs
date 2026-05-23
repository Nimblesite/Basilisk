//! Tests for [BSK-E0003] from [CHKARCH-DIAG-MISSING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-MISSING
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
fn e0003_non_empty_list_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("items = [1, 2, 3]\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0003"),
        "non-empty list with inferrable element types should not fire E0003"
    );
    Ok(())
}

#[test]
fn e0003_string_literal_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("name = \"hello\"\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0003"),
        "string literal should not fire E0003 — type is trivially str"
    );
    Ok(())
}

#[test]
fn e0003_int_literal_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("count = 42\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0003"),
        "int literal should not fire E0003 — type is trivially int"
    );
    Ok(())
}

#[test]
fn e0003_bool_literal_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("flag = True\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0003"),
        "bool literal should not fire E0003 — type is trivially bool"
    );
    Ok(())
}

#[test]
fn e0003_call_expr_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("result = some_function()\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0003"),
        "call expression should not fire E0003 — type resolution is deferred"
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

// ---------------------------------------------------------------------------
// Regression tests: scalar literals must NEVER produce false-positive E0003.
// These tests exercise real-world patterns that previously regressed.
// ---------------------------------------------------------------------------

/// Each scalar literal type individually — exhaustive per-type regression guard.
#[test]
fn e0003_regression_each_scalar_type_no_false_positive() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        ("int", "count = 42\n"),
        ("float", "rate = 3.14\n"),
        ("str", "name = \"hello\"\n"),
        ("bool", "flag = True\n"),
        ("bytes", "data = b\"raw\"\n"),
    ];
    for (type_name, source) in cases {
        let diags = run(source)?;
        let e0003_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.code.code == "BSK-E0003")
            .collect();
        assert!(
            e0003_diags.is_empty(),
            "{type_name} literal must not fire E0003 (regression), got: {:?}",
            e0003_diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }
    Ok(())
}

/// Multiple module-level scalar literals — zero E0003.
#[test]
fn e0003_regression_multiple_scalars_no_false_positives() -> Result<(), Box<dyn std::error::Error>>
{
    let source = "\
host = \"localhost\"
port = 5432
debug = True
version = 1.0
magic = b\"\\x00\"
";
    let diags = run(source)?;
    let e0003_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0003")
        .collect();
    assert!(
        e0003_diags.is_empty(),
        "module-level scalar literals must not fire E0003 (regression), got: {:?}",
        e0003_diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

/// Lambda assignments at module level — not unresolvable, no E0003.
#[test]
fn e0003_regression_lambda_no_false_positive() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("fn = lambda x: x * 2\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0003"),
        "lambda assignment should not fire E0003 (regression)"
    );
    Ok(())
}

/// The exact pattern from the example file: `double = 2` near lambdas.
#[test]
fn e0003_regression_example_file_pattern() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
double = 2
fn = lambda x: x * 2
";
    let diags = run(source)?;
    let e0003_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0003")
        .collect();
    assert!(
        e0003_diags.is_empty(),
        "example file pattern (scalar + lambda) must not fire E0003 (regression), got: {:?}",
        e0003_diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

/// Unresolvable types MUST still fire — ensure suppression is not over-broad.
#[test]
fn e0003_regression_unresolvable_types_still_fire() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        ("empty list", "items = []\n"),
        ("empty dict", "mapping = {}\n"),
        ("None", "result = None\n"),
    ];
    for (desc, source) in cases {
        let diags = run(source)?;
        assert!(
            codes(&diags).contains(&"BSK-E0003"),
            "{desc} must still fire E0003 — type is genuinely unresolvable, got: {:?}",
            codes(&diags)
        );
    }
    Ok(())
}
