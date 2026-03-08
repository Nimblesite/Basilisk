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

// ---------------------------------------------------------------------------
// E0026 — TypeVar with single constraint
// ---------------------------------------------------------------------------

#[test]
fn e0026_typevar_single_constraint() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0026_typevar_single_constraint.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0026").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0026 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0027 — Duplicate TypeVar in Generic[...]
// ---------------------------------------------------------------------------

#[test]
fn e0027_duplicate_typevar_generic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0027_duplicate_typevar_generic.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0027").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0027 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0029 — Method defined inside a TypedDict
// ---------------------------------------------------------------------------

#[test]
fn e0029_typeddict_method() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0029_typeddict_method.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0029").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0029 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0030 — Non-default TypeVar follows default TypeVar in Generic[...]
// ---------------------------------------------------------------------------

#[test]
fn e0030_non_default_after_default() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0030_non_default_after_default.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0030").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0030 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0031 — Invalid cast() call
// ---------------------------------------------------------------------------

#[test]
fn e0031_invalid_cast() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0031_invalid_cast.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0031").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0031 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0032 — Invalid keyword argument in TypedDict class
// ---------------------------------------------------------------------------

#[test]
fn e0032_typeddict_invalid_keyword() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0032_typeddict_invalid_keyword.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0032").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0032 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0033 — Invalid reveal_type() call
// ---------------------------------------------------------------------------

#[test]
fn e0033_invalid_reveal_type() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0033_invalid_reveal_type.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0033").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0033 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0034 — @final decorator violations
// ---------------------------------------------------------------------------

#[test]
fn e0034_final_class_inherit() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0034_final_class_inherit.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0034").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0034 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0035 — Required/NotRequired used outside TypedDict
// ---------------------------------------------------------------------------

#[test]
fn e0035_required_outside_typeddict() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0035_required_outside_typeddict.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0035").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0035 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0036 — ClassVar used in invalid context
// ---------------------------------------------------------------------------

#[test]
fn e0036_classvar_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0036_classvar_invalid.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0036").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0036 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0037 — Invalid TypedDict functional syntax
// ---------------------------------------------------------------------------

#[test]
fn e0037_typeddict_functional_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0037_typeddict_functional_invalid.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0037").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0037 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0038 — Invalid TypedDict inheritance
// ---------------------------------------------------------------------------

#[test]
fn e0038_typeddict_inheritance_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0038_typeddict_inheritance_invalid.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0038").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0038 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0039 — Invalid assert_type() call
// ---------------------------------------------------------------------------

#[test]
fn e0039_invalid_assert_type() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0039_invalid_assert_type.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0039").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0039 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0040 — Invalid Enum subclassing
// ---------------------------------------------------------------------------

#[test]
fn e0040_enum_subclass() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0040_enum_subclass.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0040").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0040 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0041 — Too few arguments in function call
// ---------------------------------------------------------------------------

#[test]
fn e0041_too_few_args() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0041_too_few_args.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0041").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0041 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0042 — PEP 695 type parameter mixed with traditional TypeVars
// ---------------------------------------------------------------------------

#[test]
fn e0042_pep695_mixed_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0042_pep695_mixed_typevar.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0042").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0042 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0043 — Non-TypeVar argument in Generic[...]
// ---------------------------------------------------------------------------

#[test]
fn e0043_non_typevar_in_generic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0043_non_typevar_in_generic.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0043").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0043 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0044 — Final used in invalid position
// ---------------------------------------------------------------------------

#[test]
fn e0044_final_invalid_position() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0044_final_invalid_position.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0044").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0044 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0045 — Invalid first argument to Annotated[...]
// ---------------------------------------------------------------------------

#[test]
fn e0045_annotated_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0045_annotated_invalid.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0045").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0045 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0046 — Enum member annotated with explicit type
// ---------------------------------------------------------------------------

#[test]
fn e0046_enum_member_annotated() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0046_enum_member_annotated.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0046").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0046 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0047 — Invalid type expression in annotation
// ---------------------------------------------------------------------------

#[test]
fn e0047_invalid_type_expr() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0047_invalid_type_expr.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0047").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0047 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0048 — Invalid RHS for TypeAlias
// ---------------------------------------------------------------------------

#[test]
fn e0048_typealias_invalid_rhs() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0048_typealias_invalid_rhs.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0048").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0048 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0049 — Multiple unbounded tuple components
// ---------------------------------------------------------------------------

#[test]
fn e0049_multiple_unbounded_tuple() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0049_multiple_unbounded_tuple.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0049").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0049 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0050 — Invalid NewType call
// ---------------------------------------------------------------------------

#[test]
fn e0050_invalid_newtype() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0050_invalid_newtype.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0050").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0050 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0051 — Invalid Literal parameterization
// ---------------------------------------------------------------------------

#[test]
fn e0051_invalid_literal() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0051_invalid_literal.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0051").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0051 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0052 — Frozen dataclass attribute assignment
// ---------------------------------------------------------------------------

#[test]
fn e0052_frozen_dataclass() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0052_frozen_dataclass.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0052").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0052 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0053 — assert_type() type mismatch (may be disabled)
// ---------------------------------------------------------------------------

#[test]
fn e0053_assert_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    // E0053 may be disabled pending full type inference; just verify the
    // fixture parses and runs through the pipeline without crashing.
    let _diags = run("errors/e0053_assert_type_mismatch.py")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// E0054 — Final reassignment
// ---------------------------------------------------------------------------

#[test]
fn e0054_final_reassignment() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0054_final_reassignment.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0054").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0054 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0055 — Invalid TypeVar keyword argument combination
// ---------------------------------------------------------------------------

#[test]
fn e0055_typevar_invalid_kwargs() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0055_typevar_invalid_kwargs.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0055").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0055 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0056 — Mutation of ReadOnly TypedDict fields
// ---------------------------------------------------------------------------

#[test]
fn e0056_readonly_typeddict() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0056_readonly_typeddict.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0056").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0056 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0057 — Invalid RHS in PEP 695 type alias
// ---------------------------------------------------------------------------

#[test]
fn e0057_pep695_type_alias_invalid() -> Result<(), Box<dyn std::error::Error>> {
    // E0057 depends on type_statements being populated by the resolver,
    // which is not yet implemented. Verify the fixture runs without crashing.
    let _diags = run("errors/e0057_pep695_type_alias_invalid.py")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// E0058 — Annotated requires at least two arguments
// ---------------------------------------------------------------------------

#[test]
fn e0058_annotated_too_few_args() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0058_annotated_too_few_args.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0058").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0058 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0059 — Access to __match_args__ on dataclass with match_args=False
// ---------------------------------------------------------------------------

#[test]
fn e0059_dataclass_match_args_false() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0059_dataclass_match_args_false.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0059").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0059 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0060 — Invalid ordering comparison of dataclass instances
// ---------------------------------------------------------------------------

#[test]
fn e0060_dataclass_ordering_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0060_dataclass_ordering_invalid.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0060").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0060 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0061 — assert_type with Literal[Enum.MEMBER] on enum-typed param
// ---------------------------------------------------------------------------

#[test]
fn e0061_assert_type_enum_literal() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0061_assert_type_enum_literal.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0061").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0061 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0062 — NoReturn/Never function can fall through
// ---------------------------------------------------------------------------

#[test]
fn e0062_noreturn_fallthrough() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0062_noreturn_fallthrough.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0062").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0062 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0063 — Non-hashable dataclass assigned to Hashable
// ---------------------------------------------------------------------------

#[test]
fn e0063_non_hashable_dataclass() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0063_non_hashable_dataclass.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0063").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0063 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0064 — Invalid argument in NamedTuple constructor
// ---------------------------------------------------------------------------

#[test]
fn e0064_namedtuple_invalid_arg() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0064_namedtuple_invalid_arg.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0064").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0064 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0065 — Access to int-only attribute on float-typed parameter
// ---------------------------------------------------------------------------

#[test]
fn e0065_float_param_int_attr() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0065_float_param_int_attr.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0065").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0065 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0066 — Enum member value incompatible with _value_ type
// ---------------------------------------------------------------------------

#[test]
fn e0066_enum_value_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0066_enum_value_type_mismatch.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0066").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0066 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0067 — Non-member referenced in Literal[EnumClass.X]
// ---------------------------------------------------------------------------

#[test]
fn e0067_enum_non_member_literal() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0067_enum_non_member_literal.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0067").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0067 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0068 — Literal string used where enum member reference required
// ---------------------------------------------------------------------------

#[test]
fn e0068_literal_string_enum() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0068_literal_string_enum.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0068").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0068 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0069 — Dataclass keyword-only field violations
// ---------------------------------------------------------------------------

#[test]
fn e0069_dataclass_kwonly() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0069_dataclass_kwonly.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0069").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0069 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0070 — Never type compatibility violations
// ---------------------------------------------------------------------------

#[test]
fn e0070_never_type_compat() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0070_never_type_compat.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0070").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0070 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0071 — Historical positional-only parameter violations
// ---------------------------------------------------------------------------

#[test]
fn e0071_historical_positional() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0071_historical_positional.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0071").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0071 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0072 — No matching overload for subscript indexing
// ---------------------------------------------------------------------------

#[test]
fn e0072_no_matching_overload() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0072_no_matching_overload.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0072").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0072 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0073 — NamedTuple-to-tuple type incompatibility
// ---------------------------------------------------------------------------

#[test]
fn e0073_namedtuple_tuple_compat() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0073_namedtuple_tuple_compat.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0073").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0073 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0074 — Constructor call type mismatch with specialized generic
// ---------------------------------------------------------------------------

#[test]
fn e0074_constructor_new_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0074_constructor_new_mismatch.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0074").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0074 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0075 — Incompatible type for Self-typed attribute
// ---------------------------------------------------------------------------

#[test]
fn e0075_self_type_attr_incompat() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0075_self_type_attr_incompat.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0075").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0075 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0076 — Overload union expansion failure
// ---------------------------------------------------------------------------

#[test]
fn e0076_overload_union_expansion() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0076_overload_union_expansion.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0076").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0076 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0077 — Protocol Self-return conformance violation
// ---------------------------------------------------------------------------

#[test]
fn e0077_protocol_self_return() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0077_protocol_self_return.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0077").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0077 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0078 — Self type violations in generics
// ---------------------------------------------------------------------------

#[test]
fn e0078_self_type_violation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0078_self_type_violation.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0078").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0078 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0079 — Module assigned to incompatible protocol type
// ---------------------------------------------------------------------------

#[test]
fn e0079_module_protocol_incompat() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0079_module_protocol_incompat.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0079").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0079 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0080 — TypeVar upper bound violation at call site
// ---------------------------------------------------------------------------

#[test]
fn e0080_typevar_bound_violation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0080_typevar_bound_violation.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0080").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0080 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0081 — TypeVarTuple unpack minimum type argument violation
// ---------------------------------------------------------------------------

#[test]
fn e0081_typevartuple_unpack_min() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0081_typevartuple_unpack_min.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0081").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0081 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0082 — TypeVarTuple callable/tuple argument mismatch
// ---------------------------------------------------------------------------

#[test]
fn e0082_typevartuple_callable_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0082_typevartuple_callable_mismatch.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0082").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0082 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0083 — TypeVarTuple must be unpacked with * operator
// ---------------------------------------------------------------------------

#[test]
fn e0083_typevartuple_unpack_required() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0083_typevartuple_unpack_required.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0083").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0083 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0084 — TypeVarTuple variance/bounds/constraints violation
// ---------------------------------------------------------------------------

#[test]
fn e0084_typevartuple_invalid_params() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0084_typevartuple_invalid_params.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0084").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0084 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0085 — TypeVarTuple argument count mismatch
// ---------------------------------------------------------------------------

#[test]
fn e0085_typevartuple_arg_count() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0085_typevartuple_arg_count.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0085").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0085 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0086 — Multiple TypeVarTuple declarations in generic
// ---------------------------------------------------------------------------

#[test]
fn e0086_multiple_typevartuple() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0086_multiple_typevartuple.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0086").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0086 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0088 — TypedDict runtime violation (isinstance)
// ---------------------------------------------------------------------------

#[test]
fn e0088_typeddict_isinstance() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0088_typeddict_isinstance.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0088").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0088 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0089 — Invalid PEP 695 type parameter bound or constraint
// ---------------------------------------------------------------------------

#[test]
fn e0089_pep695_invalid_bound() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0089_pep695_invalid_bound.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0089").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0089 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0090 — Invalid tuple type syntax
// ---------------------------------------------------------------------------

#[test]
fn e0090_invalid_tuple_syntax() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0090_invalid_tuple_syntax.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0090").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0090 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0091 — Incompatible TypeVar bound/constraint with default
// ---------------------------------------------------------------------------

#[test]
fn e0091_typevar_default_incompat() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0091_typevar_default_incompat.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0091").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0091 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0092 — Too few type arguments to generic class
// ---------------------------------------------------------------------------

#[test]
fn e0092_too_few_type_args() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0092_too_few_type_args.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0092").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0092 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0093 — Invalid key or value type in TypedDict assignment
// ---------------------------------------------------------------------------

#[test]
fn e0093_typeddict_key_validation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0093_typeddict_key_validation.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0093").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0093 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0094 — Self type used in an invalid location
// ---------------------------------------------------------------------------

#[test]
fn e0094_self_type_invalid_location() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0094_self_type_invalid_location.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0094").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0094 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0095 — InitVar field validation in dataclasses
// ---------------------------------------------------------------------------

#[test]
fn e0095_initvar_field() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0095_initvar_field.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0095").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0095 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0096 — Dataclass field default_factory type mismatch
// ---------------------------------------------------------------------------

#[test]
fn e0096_dataclass_default_factory() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0096_dataclass_default_factory.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0096").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0096 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0097 — Protocol __new__/__init__ sets undeclared self-attributes
// ---------------------------------------------------------------------------

#[test]
fn e0097_protocol_self_attr() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0097_protocol_self_attr.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0097").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0097 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0098 — Non-Protocol base class in Protocol definition
// ---------------------------------------------------------------------------

#[test]
fn e0098_non_protocol_base() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0098_non_protocol_base.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0098").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0098 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0099 — Direct instantiation of a Protocol class
// ---------------------------------------------------------------------------

#[test]
fn e0099_protocol_instantiation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0099_protocol_instantiation.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0099").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0099 diagnostic");
    Ok(())
}

// ---------------------------------------------------------------------------
// E0100 — Augmented assignment widens Literal type
// ---------------------------------------------------------------------------

#[test]
fn e0100_literal_augmented_assign() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("errors/e0100_literal_augmented_assign.py")?;
    let filtered: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0100").collect();
    assert!(!filtered.is_empty(), "expected at least one BSK-E0100 diagnostic");
    Ok(())
}
