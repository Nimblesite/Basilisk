//! End-to-end tests for the Basilisk analysis pipeline.
//!
//! Every test uses a real `.py` fixture file and asserts the exact set of
//! diagnostics produced: error code, symbol name, byte span, line, column,
//! and message. No hand-wavy count assertions — if a diagnostic appears at
//! the wrong location or with the wrong message, the test fails.
//!
//! Pipeline under test: `parse_file` → resolve → check
//!
//! Fixture layout:
//!   tests/fixtures/clean/   — fully typed Python; must produce zero diagnostics
//!   tests/fixtures/errors/  — deliberately broken Python; exact diagnostics asserted

use std::path::Path;

use basilisk_checker::{check, Diagnostic, Severity};
use basilisk_parser::parse_file;
use basilisk_resolver::resolve;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fixture(rel: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(rel)
        .to_string_lossy()
        .into_owned()
}

fn run(rel: &str) -> Result<Vec<Diagnostic>, Box<dyn std::error::Error>> {
    let path = fixture(rel);
    let parsed = parse_file(&path)?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

/// Convert a byte offset in `source` into a 1-based (line, col) pair.
fn line_col(source: &str, offset: u32) -> (usize, usize) {
    let clamped = (offset as usize).min(source.len());
    let before = &source[..clamped];
    let line = before.chars().filter(|&c| c == '\n').count() + 1;
    let col = before.rfind('\n').map_or(clamped, |pos| clamped - pos - 1) + 1;
    (line, col)
}

/// A concise expected-diagnostic value constructed in tests.
#[derive(Debug)]
struct Expected {
    code: &'static str,
    severity: Severity,
    /// Substring that must appear in the message (usually the symbol name).
    message_contains: &'static str,
    line: usize,
    col: usize,
}

impl Expected {
    fn error(code: &'static str, message_contains: &'static str, line: usize, col: usize) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message_contains,
            line,
            col,
        }
    }

    #[allow(dead_code)]
    fn warning(
        code: &'static str,
        message_contains: &'static str,
        line: usize,
        col: usize,
    ) -> Self {
        Self {
            code,
            severity: Severity::Warning,
            message_contains,
            line,
            col,
        }
    }
}

/// Assert that `diags` matches `expected` exactly — same count, same order
/// (sorted by span start), same code/severity/location/message.
fn assert_diagnostics(source: &str, diags: &[Diagnostic], expected: &[Expected]) {
    let mut sorted = diags.to_vec();
    // Sort by span start, then by code for a stable order when two diagnostics
    // share the same position (e.g. E0025 and E0002 on the same method line).
    sorted.sort_by(|a, b| {
        a.span
            .start
            .cmp(&b.span.start)
            .then(a.code.code.cmp(b.code.code))
    });

    assert_eq!(
        sorted.len(),
        expected.len(),
        "wrong number of diagnostics.\n  got:\n{}\n  want {} diagnostics",
        sorted
            .iter()
            .map(|d| {
                let (l, c) = line_col(source, d.span.start);
                format!(
                    "    {}[{}] at {l}:{c} — {}",
                    d.severity, d.code.code, d.message
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        expected.len(),
    );

    for (i, (got, want)) in sorted.iter().zip(expected.iter()).enumerate() {
        let (got_line, got_col) = line_col(source, got.span.start);

        assert_eq!(
            got.code.code, want.code,
            "diagnostic[{i}]: wrong code (got {}, want {})",
            got.code.code, want.code
        );
        assert_eq!(
            got.severity, want.severity,
            "diagnostic[{i}]: wrong severity"
        );
        assert_eq!(
            got_line, want.line,
            "diagnostic[{i}]: wrong line (got {got_line}, want {})",
            want.line
        );
        assert_eq!(
            got_col, want.col,
            "diagnostic[{i}]: wrong column (got {got_col}, want {})",
            want.col
        );
        assert!(
            got.message.contains(want.message_contains),
            "diagnostic[{i}]: message {:#?} does not contain {:#?}",
            got.message,
            want.message_contains,
        );
    }
}

// ---------------------------------------------------------------------------
// Clean fixtures — zero diagnostics expected
// ---------------------------------------------------------------------------

#[test]
fn clean_fully_typed_module_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/fully_typed_module.py")?;
    assert!(
        diags.is_empty(),
        "fully_typed_module.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_typed_with_varargs_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_with_varargs.py")?;
    assert!(
        diags.is_empty(),
        "typed_with_varargs.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_nested_functions_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/nested_functions.py")?;
    assert!(
        diags.is_empty(),
        "nested_functions.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0001 — Missing parameter type annotation
// ---------------------------------------------------------------------------

/// ```python
/// def process(data) -> None:   # `data` at col 13, line 1
///     pass
/// ```
#[test]
fn e0001_single_unannotated_param() -> Result<(), Box<dyn std::error::Error>> {
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
fn e0001_three_unannotated_params() -> Result<(), Box<dyn std::error::Error>> {
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
/// def log(*messages, level: str) -> None:   # *messages unannotated → E0004, col 10
///     pass
/// ```
#[test]
fn e0004_unannotated_vararg() -> Result<(), Box<dyn std::error::Error>> {
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
/// def configure(**options) -> None:   # **options unannotated → E0004, col 17
///     pass
/// ```
#[test]
fn e0004_unannotated_kwarg() -> Result<(), Box<dyn std::error::Error>> {
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
fn e0001_unannotated_param_in_nested_function() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0001_nested.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0001_nested.py"))?;
    assert_diagnostics(&src, &diags, &[Expected::error("BSK-E0001", "`y`", 2, 15)]);
    Ok(())
}

// ---------------------------------------------------------------------------
// E0002 — Missing return type annotation
// ---------------------------------------------------------------------------

/// ```python
/// def fetch(url: str):   # `fetch` at col 5, line 1
///     pass
/// ```
#[test]
fn e0002_single_function_missing_return() -> Result<(), Box<dyn std::error::Error>> {
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
fn e0002_three_functions_all_missing_return() -> Result<(), Box<dyn std::error::Error>> {
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
// E0001 + E0002 — Mixed, class methods
// ---------------------------------------------------------------------------

#[test]
fn e0001_and_e0002_class_methods() -> Result<(), Box<dyn std::error::Error>> {
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
// Clean fixtures — additional patterns, zero diagnostics expected
// ---------------------------------------------------------------------------

#[test]
fn clean_typed_generics_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_generics.py")?;
    assert!(
        diags.is_empty(),
        "typed_generics.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_typed_optional_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_optional.py")?;
    assert!(
        diags.is_empty(),
        "typed_optional.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_typed_inheritance_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_inheritance.py")?;
    assert!(
        diags.is_empty(),
        "typed_inheritance.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_typed_dataclass_style_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_dataclass_style.py")?;
    assert!(
        diags.is_empty(),
        "typed_dataclass_style.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_typed_control_flow_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_control_flow.py")?;
    assert!(
        diags.is_empty(),
        "typed_control_flow.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0002 — dunder methods without return annotations
// ---------------------------------------------------------------------------

#[test]
fn e0002_dunder_methods_all_missing_return() -> Result<(), Box<dyn std::error::Error>> {
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
// E0001 — mixed: some params annotated, some not
// ---------------------------------------------------------------------------

#[test]
fn e0001_only_unannotated_params_flagged_in_mixed_signature(
) -> Result<(), Box<dyn std::error::Error>> {
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
// E0001 — positional-only parameters without annotations
// ---------------------------------------------------------------------------

#[test]
fn e0001_positional_only_params_flagged() -> Result<(), Box<dyn std::error::Error>> {
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
// E0002 — missing return on innermost nested function only
// ---------------------------------------------------------------------------

#[test]
fn e0002_only_innermost_nested_function_missing_return() -> Result<(), Box<dyn std::error::Error>> {
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
// E0001 — keyword-only params without annotations
// ---------------------------------------------------------------------------

#[test]
fn e0001_unannotated_keyword_only_params_flagged() -> Result<(), Box<dyn std::error::Error>> {
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
// E0001 + E0002 — four module-level functions, all completely untyped
// ---------------------------------------------------------------------------

#[test]
fn e0001_and_e0002_four_completely_untyped_functions() -> Result<(), Box<dyn std::error::Error>> {
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
// E0002 — function inside else branch of version guard
// ---------------------------------------------------------------------------

#[test]
fn e0002_function_in_else_branch_of_version_guard() -> Result<(), Box<dyn std::error::Error>> {
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
// E0001 + E0002 — subclass overrides with missing annotations
// ---------------------------------------------------------------------------

#[test]
fn e0001_and_e0002_subclass_override_missing_annotations() -> Result<(), Box<dyn std::error::Error>>
{
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
// Clean fixtures — control flow and exception handling
// ---------------------------------------------------------------------------

#[test]
fn clean_typed_try_except_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_try_except.py")?;
    assert!(
        diags.is_empty(),
        "typed_try_except.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_typed_while_for_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_while_for.py")?;
    assert!(
        diags.is_empty(),
        "typed_while_for.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_typed_with_statement_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_with_statement.py")?;
    assert!(
        diags.is_empty(),
        "typed_with_statement.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0001 + E0002 — untyped functions inside try/except blocks
// ---------------------------------------------------------------------------

#[test]
fn e0001_and_e0002_functions_inside_try_except() -> Result<(), Box<dyn std::error::Error>> {
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
// E0001 + E0002 — untyped functions inside while/for blocks
// ---------------------------------------------------------------------------

#[test]
fn e0001_and_e0002_functions_inside_while_for() -> Result<(), Box<dyn std::error::Error>> {
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
// E0002 — zero-param functions without return annotation
// ---------------------------------------------------------------------------

#[test]
fn e0002_zero_param_functions_all_missing_return() -> Result<(), Box<dyn std::error::Error>> {
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
// E0001 — unannotated params in doubly-nested class methods
// ---------------------------------------------------------------------------

#[test]
fn e0001_params_in_doubly_nested_class_methods() -> Result<(), Box<dyn std::error::Error>> {
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
// E0001 + E0004 — every parameter kind in one signature
//
// def everything(pos_only, /, normal, *args, kw_only, **kwargs) -> None:
//                ^^^^^^^^^      ^^^^^^  ^^^^  ^^^^^^^   ^^^^^^
// pos_only (E0001), normal (E0001), args (E0004), kw_only (E0001), kwargs (E0004)
// ---------------------------------------------------------------------------

#[test]
fn e0001_and_e0004_all_parameter_kinds_flagged() -> Result<(), Box<dyn std::error::Error>> {
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
// E0003 — module-level variables with unresolvable inference
// ---------------------------------------------------------------------------

#[test]
fn e0003_unannotated_module_vars() -> Result<(), Box<dyn std::error::Error>> {
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
// E0005 — class attributes without type annotations
// ---------------------------------------------------------------------------

#[test]
fn e0005_unannotated_class_attributes() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0005_class_attrs.py")?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"BSK-E0005"),
        "should emit E0005 for unannotated class attributes, got: {diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0010 — import from untyped module
// ---------------------------------------------------------------------------

#[test]
fn e0010_import_from_untyped_module() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0010_untyped_import.py")?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"BSK-E0010"),
        "should emit E0010 for untyped imports, got: {diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0011 — explicit Any without justification
// ---------------------------------------------------------------------------

#[test]
fn e0011_explicit_any_in_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0011_explicit_any.py")?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"BSK-E0011"),
        "should emit E0011 for explicit Any annotations, got: {diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0013 — return type mismatch (-> None returning value)
// ---------------------------------------------------------------------------

#[test]
fn e0013_none_annotated_returning_value() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0013_return_mismatch.py")?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"BSK-E0013"),
        "should emit E0013 when -> None function returns a value, got: {diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0014 — assignment type incompatibility (literal mismatches)
// ---------------------------------------------------------------------------

#[test]
fn e0014_literal_assigned_to_incompatible_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0014_assignment_incompatible.py")?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"BSK-E0014"),
        "should emit E0014 for literal type mismatches, got: {diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0015 — invalid type argument count
// ---------------------------------------------------------------------------

#[test]
fn e0015_invalid_type_arg_count() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0015_invalid_type_arg.py")?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"BSK-E0015"),
        "should emit E0015 for invalid generic arg count, got: {diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0020 — @overload without implementation
// ---------------------------------------------------------------------------

#[test]
fn e0020_overload_missing_implementation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0020_missing_overload_impl.py")?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"BSK-E0020"),
        "should emit E0020 when @overload has no implementation, got: {diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0021 — overlapping @overload signatures
// ---------------------------------------------------------------------------

#[test]
fn e0021_overlapping_overload_signatures() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0021_overlapping_overloads.py")?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"BSK-E0021"),
        "should emit E0021 for overlapping overload signatures, got: {diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0023 — non-exhaustive match (no wildcard case)
// ---------------------------------------------------------------------------

#[test]
fn e0023_match_without_wildcard() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0023_nonexhaustive_match.py")?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"BSK-E0023"),
        "should emit E0023 for match without wildcard, got: {diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0024 — invalid type form in annotation
// ---------------------------------------------------------------------------

#[test]
fn e0024_numeric_literal_as_type_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0024_invalid_type_form.py")?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"BSK-E0024"),
        "should emit E0024 for numeric literal used as type, got: {diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0025 — method override without @override decorator
// ---------------------------------------------------------------------------

#[test]
fn e0025_override_without_decorator() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0025_missing_override.py")?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"BSK-E0025"),
        "should emit E0025 for override without @override, got: {diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0011 — Any on vararg, kwarg, and return annotation
// ---------------------------------------------------------------------------

#[test]
fn e0011_any_on_vararg_kwarg_and_return() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0011_vararg_kwarg_any.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0011_vararg_kwarg_any.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::warning("BSK-E0011", "return annotation", 4, 5),
            Expected::warning("BSK-E0011", "`args`", 4, 14),
            Expected::warning("BSK-E0011", "`kwargs`", 4, 27),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0024 — numeric literal on vararg, kwarg, and return annotation
// ---------------------------------------------------------------------------

#[test]
fn e0024_numeric_literal_on_vararg_kwarg_and_return() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0024_vararg_kwarg_return_literal.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0024_vararg_kwarg_return_literal.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0024", "return type", 1, 5),
            Expected::error("BSK-E0024", "`args`", 1, 14),
            Expected::error("BSK-E0024", "`kwargs`", 1, 26),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0014 — bytes literal, float literal, and int-to-bytes mismatches
// ---------------------------------------------------------------------------

#[test]
fn e0014_bytes_and_float_mismatches() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0014_bytes_float_mismatches.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0014_bytes_float_mismatches.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0014", "`ratio`", 1, 1),
            Expected::error("BSK-E0014", "`name`", 2, 1),
            Expected::error("BSK-E0014", "`raw`", 3, 1),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0015 — set, frozenset, and dict with wrong type argument counts
// ---------------------------------------------------------------------------

#[test]
fn e0015_set_frozenset_and_dict_wrong_arg_count() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0015_more_generics.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0015_more_generics.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0015", "`set[", 1, 11),
            Expected::error("BSK-E0015", "`frozenset[", 5, 17),
            Expected::error("BSK-E0015", "`data`", 9, 18),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0020 — exact diagnostic: two @overload variants with no implementation
// ---------------------------------------------------------------------------

#[test]
fn e0020_exact_diagnostic_for_double() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0020_missing_overload_impl.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0020_missing_overload_impl.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[Expected::error("BSK-E0020", "`double`", 5, 5)],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0021 — exact diagnostics: overlapping overloads also trigger E0001
// ---------------------------------------------------------------------------

#[test]
fn e0021_exact_diagnostics_for_overlapping_overloads() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0021_overlapping_overloads.py")?;
    let src = std::fs::read_to_string(fixture("errors/e0021_overlapping_overloads.py"))?;
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0001", "`x`", 5, 13),
            Expected::error("BSK-E0021", "`process`", 9, 5),
            Expected::error("BSK-E0001", "`x`", 9, 13),
        ],
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Clean — overloads with different arities must not trigger E0020 or E0021
// ---------------------------------------------------------------------------

#[test]
fn clean_overloads_different_arity_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_overloads_multi_arity.py")?;
    assert!(
        diags.is_empty(),
        "overloads with different arities must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// FAILING TESTS — rules not yet implemented (Phase 1 limitations)
// These tests document desired behavior and fail to mark missing functionality.
// ---------------------------------------------------------------------------

/// E0012: Argument type mismatch.
/// Requires a type inference engine — not implemented in Phase 1.
#[test]
fn e0012_argument_type_mismatch_not_yet_implemented() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0012_wrong_arg_type.py")?;
    assert!(
        diags.iter().any(|d| d.code.code == "BSK-E0012"),
        "E0012 (argument type mismatch) not yet implemented — Phase 1 limitation"
    );
    Ok(())
}

/// E0016: Incompatible method override (type-level).
/// Requires class hierarchy + type inference — not implemented in Phase 1.
#[test]
fn e0016_incompatible_method_override_not_yet_implemented() -> Result<(), Box<dyn std::error::Error>>
{
    let diags = run("errors/e0016_incompatible_override.py")?;
    assert!(
        diags.iter().any(|d| d.code.code == "BSK-E0016"),
        "E0016 (incompatible override) not yet implemented — Phase 1 limitation"
    );
    Ok(())
}

/// E0017: Incompatible variable override.
/// Requires type inference for variable types — not implemented in Phase 1.
#[test]
fn e0017_incompatible_variable_override_not_yet_implemented(
) -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0017_variable_override.py")?;
    assert!(
        diags.iter().any(|d| d.code.code == "BSK-E0017"),
        "E0017 (incompatible variable override) not yet implemented — Phase 1 limitation"
    );
    Ok(())
}

/// E0018: Undefined variable.
/// Requires full scope analysis of expressions — not implemented in Phase 1.
#[test]
fn e0018_undefined_variable_not_yet_implemented() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0018_undefined_variable.py")?;
    assert!(
        diags.iter().any(|d| d.code.code == "BSK-E0018"),
        "E0018 (undefined variable) not yet implemented — Phase 1 limitation"
    );
    Ok(())
}

/// E0019: Unbound variable on some code paths.
/// Requires full flow analysis — not implemented in Phase 1.
#[test]
fn e0019_unbound_variable_not_yet_implemented() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0019_unbound_variable.py")?;
    assert!(
        diags.iter().any(|d| d.code.code == "BSK-E0019"),
        "E0019 (unbound variable) not yet implemented — Phase 1 limitation"
    );
    Ok(())
}

/// E0022: Unhashable type in hash-requiring context.
/// Requires type inference — not implemented in Phase 1.
#[test]
fn e0022_unhashable_type_not_yet_implemented() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0022_unhashable_type.py")?;
    assert!(
        diags.iter().any(|d| d.code.code == "BSK-E0022"),
        "E0022 (unhashable type) not yet implemented — Phase 1 limitation"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Clean fixtures for new rules — must produce zero diagnostics
// ---------------------------------------------------------------------------

#[test]
fn clean_typed_module_vars_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_module_vars.py")?;
    assert!(
        diags.is_empty(),
        "typed_module_vars.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_typed_class_attrs_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_class_attrs.py")?;
    assert!(
        diags.is_empty(),
        "typed_class_attrs.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_typed_overloads_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_overloads.py")?;
    assert!(
        diags.is_empty(),
        "typed_overloads.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_typed_override_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_override.py")?;
    assert!(
        diags.is_empty(),
        "typed_override.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

#[test]
fn clean_typed_match_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_match.py")?;
    assert!(
        diags.is_empty(),
        "typed_match.py must produce no diagnostics, got:\n{diags:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0010 — stdlib imports must NOT trigger E0010
// ---------------------------------------------------------------------------

#[test]
fn clean_stdlib_imports_are_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_stdlib_imports.py")?;
    let e0010: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0010")
        .collect();
    assert!(
        e0010.is_empty(),
        "stdlib imports must not produce E0010, got:\n{e0010:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0011 — typing.Any import itself must NOT trigger E0010/E0011
// ---------------------------------------------------------------------------

#[test]
fn clean_any_with_comment_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_any_justified.py")?;
    let e0011: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0011")
        .collect();
    assert!(
        e0011.is_empty(),
        "justified Any must not produce E0011, got:\n{e0011:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0023 — match with wildcard must NOT trigger E0023
// ---------------------------------------------------------------------------

#[test]
fn clean_match_with_wildcard_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_match.py")?;
    let e0023: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0023")
        .collect();
    assert!(
        e0023.is_empty(),
        "match with wildcard must not produce E0023, got:\n{e0023:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0025 — override WITH @override must NOT trigger E0025
// ---------------------------------------------------------------------------

#[test]
fn clean_override_with_decorator_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_override.py")?;
    let e0025: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0025")
        .collect();
    assert!(
        e0025.is_empty(),
        "override with @override must not produce E0025, got:\n{e0025:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// E0020 — proper @overload with implementation must NOT trigger E0020
// ---------------------------------------------------------------------------

#[test]
fn clean_overloads_with_implementation_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("clean/typed_overloads.py")?;
    let e0020: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0020")
        .collect();
    assert!(
        e0020.is_empty(),
        "properly implemented overloads must not produce E0020, got:\n{e0020:#?}"
    );
    Ok(())
}
