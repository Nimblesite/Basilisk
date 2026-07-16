//! Tests for [BSK-0003] from [CHKARCH-DIAG-MISSING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-MISSING
// Integration tests for BSK-0003: Missing variable type (unresolvable inference).

use super::common::*;

#[test]
fn empty_list_fires() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_with_config("items = []\n", &annotation_rules_config())?;
    assert!(
        codes(&diags).contains(&"BSK-0003"),
        "unannotated empty list should fire BSK-0003, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn empty_dict_fires() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_with_config("mapping = {}\n", &annotation_rules_config())?;
    assert!(
        codes(&diags).contains(&"BSK-0003"),
        "unannotated empty dict should fire BSK-0003, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn none_value_fires() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_with_config("result = None\n", &annotation_rules_config())?;
    assert!(
        codes(&diags).contains(&"BSK-0003"),
        "unannotated None should fire BSK-0003, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn annotated_empty_list_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_with_config("items: list[int] = []\n", &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-0003"),
        "annotated empty list should not fire BSK-0003"
    );
    Ok(())
}

#[test]
fn annotated_none_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_with_config("result: int | None = None\n", &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-0003"),
        "annotated None should not fire BSK-0003"
    );
    Ok(())
}

#[test]
fn non_empty_list_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_with_config("items = [1, 2, 3]\n", &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-0003"),
        "non-empty list with inferrable element types should not fire BSK-0003"
    );
    Ok(())
}

#[test]
fn string_literal_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_with_config("name = \"hello\"\n", &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-0003"),
        "string literal should not fire BSK-0003 — type is trivially str"
    );
    Ok(())
}

#[test]
fn int_literal_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_with_config("count = 42\n", &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-0003"),
        "int literal should not fire BSK-0003 — type is trivially int"
    );
    Ok(())
}

#[test]
fn bool_literal_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_with_config("flag = True\n", &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-0003"),
        "bool literal should not fire BSK-0003 — type is trivially bool"
    );
    Ok(())
}

#[test]
fn call_expr_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_with_config("result = some_function()\n", &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-0003"),
        "call expression should not fire BSK-0003 — type resolution is deferred"
    );
    Ok(())
}

#[test]
fn diagnostic_has_help() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_with_config("items = []\n", &annotation_rules_config())?;
    let e0003 = diags.iter().find(|d| d.code.code == "BSK-0003");
    assert!(e0003.is_some(), "should fire BSK-0003");
    let Some(diag) = e0003 else {
        return Err("BSK-0003 diagnostic missing after assertion".into());
    };
    assert!(diag.help.is_some(), "BSK-0003 should have help text");
    assert!(diag.note.is_some(), "BSK-0003 should have note text");
    Ok(())
}

// ---------------------------------------------------------------------------
// Regression tests: scalar literals must NEVER produce false-positive BSK-0003.
// These tests exercise real-world patterns that previously regressed.
// ---------------------------------------------------------------------------

/// Each scalar literal type individually — exhaustive per-type regression guard.
#[test]
fn regression_each_scalar_type_no_false_positive() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        ("int", "count = 42\n"),
        ("float", "rate = 3.14\n"),
        ("str", "name = \"hello\"\n"),
        ("bool", "flag = True\n"),
        ("bytes", "data = b\"raw\"\n"),
    ];
    for (type_name, source) in cases {
        let diags = run_with_config(source, &annotation_rules_config())?;
        let e0003_diags: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0003").collect();
        assert!(
            e0003_diags.is_empty(),
            "{type_name} literal must not fire BSK-0003 (regression), got: {:?}",
            e0003_diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }
    Ok(())
}

/// Multiple module-level scalar literals — zero BSK-0003.
#[test]
fn regression_multiple_scalars_no_false_positives() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
host = \"localhost\"
port = 5432
debug = True
version = 1.0
magic = b\"\\x00\"
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    let e0003_diags: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0003").collect();
    assert!(
        e0003_diags.is_empty(),
        "module-level scalar literals must not fire BSK-0003 (regression), got: {:?}",
        e0003_diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

/// Lambda assignments at module level — not unresolvable, no BSK-0003.
#[test]
fn regression_lambda_no_false_positive() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_with_config("fn = lambda x: x * 2\n", &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-0003"),
        "lambda assignment should not fire BSK-0003 (regression)"
    );
    Ok(())
}

/// The exact pattern from the example file: `double = 2` near lambdas.
#[test]
fn regression_example_file_pattern() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
double = 2
fn = lambda x: x * 2
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    let e0003_diags: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0003").collect();
    assert!(
        e0003_diags.is_empty(),
        "example file pattern (scalar + lambda) must not fire BSK-0003 (regression), got: {:?}",
        e0003_diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

/// Unresolvable types MUST still fire — ensure suppression is not over-broad.
#[test]
fn regression_unresolvable_types_still_fire() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        ("empty list", "items = []\n"),
        ("empty dict", "mapping = {}\n"),
        ("None", "result = None\n"),
    ];
    for (desc, source) in cases {
        let diags = run_with_config(source, &annotation_rules_config())?;
        assert!(
            codes(&diags).contains(&"BSK-0003"),
            "{desc} must still fire BSK-0003 — type is genuinely unresolvable, got: {:?}",
            codes(&diags)
        );
    }
    Ok(())
}
