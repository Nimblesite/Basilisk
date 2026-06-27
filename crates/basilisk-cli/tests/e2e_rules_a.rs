//! Tests for [CHKARCH-CLI]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! E2E tests for error codes BSK-E0001 through BSK-E0005.
//!
//! Missing parameter type annotation
//! Missing return type annotation
//! Unannotated module-level variable
//! Unannotated *args/**kwargs
//! Unannotated class attribute

mod common;

use basilisk_test_utils::{assert_diagnostics, Expected};
use common::{fixture, run};

// ---------------------------------------------------------------------------
// Missing parameter type annotation
// ---------------------------------------------------------------------------

/// ```python
/// def process(data) -> None:   # `data` at col 13, line 1
///     pass
/// ```
#[test]
fn single_unannotated_param() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0001_single_param.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0001_single_param.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[Expected::error("BSK-E0001", "`data`", 1, 13)],
    );
    Ok(())
}

/// ```python
/// def compute(x, y, z) -> int:   # x→col 13, y→col 16, z→col 19
///     return 0
/// ```
#[test]
fn three_unannotated_params() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0001_multi_param.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0001_multi_param.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0001", "`x`", 1, 13),
            Expected::error("BSK-E0001", "`y`", 1, 16),
            Expected::error("BSK-E0001", "`z`", 1, 19),
        ],
    );
    Ok(())
}

/// ```python
/// def log(*messages, level: str) -> None:   # *messages unannotated → BSK-E0004, col 10
///     pass
/// ```
#[test]
fn unannotated_vararg() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0001_varargs.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0001_varargs.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[Expected::error("BSK-E0004", "`messages`", 1, 10)],
    );
    Ok(())
}

/// ```python
/// def configure(**options) -> None:   # **options unannotated → BSK-E0004, col 17
///     pass
/// ```
#[test]
fn unannotated_kwarg() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0001_kwargs.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0001_kwargs.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[Expected::error("BSK-E0004", "`options`", 1, 17)],
    );
    Ok(())
}

/// ```python
/// def outer(x: int) -> int:
///     def inner(y) -> int:   # `y` unannotated, line 2, col 15
///         return x + y
///
///     return inner(1)
/// ```
#[test]
fn unannotated_param_in_nested_function() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0001_nested.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0001_nested.py"))?;
    assert_diagnostics(&src, &diags, &[Expected::error("BSK-E0001", "`y`", 2, 15)]);
    Ok(())
}

// ---------------------------------------------------------------------------
// Missing return type annotation
// ---------------------------------------------------------------------------

/// ```python
/// def fetch(url: str):   # `fetch` at col 5, line 1
///     pass
/// ```
#[test]
fn single_function_missing_return() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0002_single_func.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0002_single_func.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[Expected::error("BSK-E0002", "`fetch`", 1, 5)],
    );
    Ok(())
}

/// ```python
/// def fetch(url: str):       # line 1, col 5
///     pass
///
///
/// def compute(x: int, y: int):   # line 5, col 5
///     return x + y
///
///
/// def noop():   # line 9, col 5
///     pass
/// ```
#[test]
fn three_functions_all_missing_return() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0002_multiple_funcs.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0002_multiple_funcs.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0002", "`fetch`", 1, 5),
            Expected::error("BSK-E0002", "`compute`", 5, 5),
            Expected::error("BSK-E0002", "`noop`", 9, 5),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// BSK-E0001 + BSK-E0002 — Mixed, class methods
// ---------------------------------------------------------------------------

#[test]
fn and_e0002_class_methods() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0001_and_e0002_class_methods.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0001_and_e0002_class_methods.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0002", "`connect`", 2, 9),
            Expected::error("BSK-E0001", "`host`", 2, 23),
            Expected::error("BSK-E0001", "`port`", 2, 29),
            Expected::error("BSK-E0002", "`send`", 8, 9),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// dunder methods without return annotations
// ---------------------------------------------------------------------------

#[test]
fn dunder_methods_all_missing_return() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0002_dunder_methods.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0002_dunder_methods.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0002", "`__init__`", 2, 9),
            Expected::error("BSK-E0002", "`__repr__`", 6, 9),
            Expected::error("BSK-E0002", "`__add__`", 9, 9),
            Expected::error("BSK-E0002", "`__len__`", 12, 9),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// mixed: some params annotated, some not
// ---------------------------------------------------------------------------

#[test]
fn only_unannotated_params_flagged_in_mixed_signature() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0001_mixed_annotated.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0001_mixed_annotated.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0001", "`destination`", 1, 27),
            Expected::error("BSK-E0001", "`currency`", 1, 55),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// positional-only parameters without annotations
// ---------------------------------------------------------------------------

#[test]
fn positional_only_params_flagged() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0001_posonly_params.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0001_posonly_params.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0001", "`numerator`", 1, 12),
            Expected::error("BSK-E0001", "`denominator`", 1, 23),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// missing return on innermost nested function only
// ---------------------------------------------------------------------------

#[test]
fn only_innermost_nested_function_missing_return() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0002_deeply_nested.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0002_deeply_nested.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[Expected::error("BSK-E0002", "`inner`", 3, 13)],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// keyword-only params without annotations
// ---------------------------------------------------------------------------

#[test]
fn unannotated_keyword_only_params_flagged() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0001_kwonly_params.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0001_kwonly_params.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0001", "`height`", 1, 27),
            Expected::error("BSK-E0001", "`background`", 1, 35),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// BSK-E0001 + BSK-E0002 — four module-level functions, all completely untyped
// ---------------------------------------------------------------------------

#[test]
fn and_e0002_four_completely_untyped_functions() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0001_and_e0002_module_level.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0001_and_e0002_module_level.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0002", "`parse`", 1, 5),
            Expected::error("BSK-E0001", "`raw`", 1, 11),
            Expected::error("BSK-E0002", "`validate`", 5, 5),
            Expected::error("BSK-E0001", "`value`", 5, 14),
            Expected::error("BSK-E0002", "`transform`", 9, 5),
            Expected::error("BSK-E0001", "`data`", 9, 15),
            Expected::error("BSK-E0002", "`serialize`", 13, 5),
            Expected::error("BSK-E0001", "`obj`", 13, 15),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// function inside else branch of version guard
// ---------------------------------------------------------------------------

#[test]
fn function_in_else_branch_of_version_guard() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0002_in_if_block.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0002_in_if_block.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[Expected::error("BSK-E0002", "`new_feature`", 7, 9)],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// BSK-E0001 + BSK-E0002 — subclass overrides with missing annotations
// ---------------------------------------------------------------------------

#[test]
fn and_e0002_subclass_override_missing_annotations() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0001_and_e0002_inheritance.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0001_and_e0002_inheritance.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0002", "`process`", 7, 9),
            Expected::error("BSK-E0025", "`process`", 7, 9),
            Expected::error("BSK-E0001", "`data`", 7, 23),
            Expected::error("BSK-E0002", "`extra`", 10, 9),
            Expected::error("BSK-E0001", "`value`", 10, 21),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// BSK-E0001 + BSK-E0002 — untyped functions inside try/except blocks
// ---------------------------------------------------------------------------

#[test]
fn and_e0002_functions_inside_try_except() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0001_and_e0002_try_except.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0001_and_e0002_try_except.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0002", "`risky`", 1, 5),
            Expected::error("BSK-E0001", "`value`", 1, 11),
            Expected::error("BSK-E0002", "`also_risky`", 8, 5),
            Expected::error("BSK-E0001", "`a`", 8, 16),
            Expected::error("BSK-E0001", "`b`", 8, 19),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// BSK-E0001 + BSK-E0002 — untyped functions inside while/for blocks
// ---------------------------------------------------------------------------

#[test]
fn and_e0002_functions_inside_while_for() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0001_and_e0002_while_for.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0001_and_e0002_while_for.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0002", "`count`", 1, 5),
            Expected::error("BSK-E0001", "`limit`", 1, 11),
            Expected::error("BSK-E0002", "`search`", 8, 5),
            Expected::error("BSK-E0001", "`items`", 8, 12),
            Expected::error("BSK-E0001", "`target`", 8, 19),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// zero-param functions without return annotation
// ---------------------------------------------------------------------------

#[test]
fn zero_param_functions_all_missing_return() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0002_no_params.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0002_no_params.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0002", "`get_version`", 1, 5),
            Expected::error("BSK-E0002", "`get_timestamp`", 5, 5),
            Expected::error("BSK-E0002", "`noop`", 9, 5),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// unannotated params in doubly-nested class methods
// ---------------------------------------------------------------------------

#[test]
fn params_in_doubly_nested_class_methods() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0001_deeply_nested_class.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0001_deeply_nested_class.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0001", "`value`", 3, 26),
            Expected::error("BSK-E0001", "`x`", 6, 28),
            Expected::error("BSK-E0001", "`y`", 6, 31),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// BSK-E0001 + BSK-E0004 — every parameter kind in one signature
// ---------------------------------------------------------------------------

#[test]
fn and_e0004_all_parameter_kinds_flagged() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0001_all_param_kinds.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0001_all_param_kinds.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0001", "`pos_only`", 1, 16),
            Expected::error("BSK-E0001", "`normal`", 1, 29),
            Expected::error("BSK-E0004", "`args`", 1, 38),
            Expected::error("BSK-E0001", "`kw_only`", 1, 44),
            Expected::error("BSK-E0004", "`kwargs`", 1, 55),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// module-level variables with unresolvable inference
// ---------------------------------------------------------------------------

#[test]
fn unannotated_module_vars() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0003_module_vars.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0003_module_vars.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0003", "`items`", 1, 1),
            Expected::error("BSK-E0003", "`data`", 2, 1),
            Expected::error("BSK-E0003", "`empty`", 3, 1),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// class attributes without type annotations
// ---------------------------------------------------------------------------

#[test]
fn unannotated_class_attributes() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0005_class_attrs.py")?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"BSK-E0005"),
        "should emit BSK-E0005 for unannotated class attributes, got: {diags:#?}"
    );
    Ok(())
}
