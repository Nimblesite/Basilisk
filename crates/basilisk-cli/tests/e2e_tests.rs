//! End-to-end tests for the Basilisk analysis pipeline.
//!
//! Every test uses a real `.py` fixture file and asserts the exact set of
//! diagnostics produced: error code, symbol name, byte span, line, column,
//! and message. No hand-wavy count assertions — if a diagnostic appears at
//! the wrong location or with the wrong message, the test fails.
//!
//! Pipeline under test: parse_file → resolve → check
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

fn run(rel: &str) -> Vec<Diagnostic> {
    let path = fixture(rel);
    let parsed = parse_file(&path).unwrap_or_else(|e| panic!("parse failed for {rel}: {e}"));
    let resolved = resolve(&parsed).unwrap_or_else(|e| panic!("resolve failed for {rel}: {e}"));
    check(&resolved)
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
}

/// Assert that `diags` matches `expected` exactly — same count, same order
/// (sorted by span start), same code/severity/location/message.
fn assert_diagnostics(source: &str, diags: &[Diagnostic], expected: &[Expected]) {
    let mut sorted = diags.to_vec();
    sorted.sort_by_key(|d| d.span.start);

    assert_eq!(
        sorted.len(),
        expected.len(),
        "wrong number of diagnostics.\n  got:\n{}\n  want {} diagnostics",
        sorted
            .iter()
            .map(|d| {
                let (l, c) = line_col(source, d.span.start);
                format!("    {}[{}] at {l}:{c} — {}", d.severity, d.code.code, d.message)
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
fn clean_fully_typed_module_is_silent() {
    let diags = run("clean/fully_typed_module.py");
    assert!(
        diags.is_empty(),
        "fully_typed_module.py must produce no diagnostics, got:\n{diags:#?}"
    );
}

#[test]
fn clean_typed_with_varargs_is_silent() {
    let diags = run("clean/typed_with_varargs.py");
    assert!(
        diags.is_empty(),
        "typed_with_varargs.py must produce no diagnostics, got:\n{diags:#?}"
    );
}

#[test]
fn clean_nested_functions_is_silent() {
    let diags = run("clean/nested_functions.py");
    assert!(
        diags.is_empty(),
        "nested_functions.py must produce no diagnostics, got:\n{diags:#?}"
    );
}

// ---------------------------------------------------------------------------
// E0001 — Missing parameter type annotation
// ---------------------------------------------------------------------------

/// ```python
/// def process(data) -> None:   # `data` at col 13, line 1
///     pass
/// ```
#[test]
fn e0001_single_unannotated_param() {
    let diags = run("errors/e0001_single_param.py");
    let src = std::fs::read_to_string(fixture("errors/e0001_single_param.py")).unwrap();
    assert_diagnostics(
        &src,
        &diags,
        &[Expected::error("BSK-E0001", "`data`", 1, 13)],
    );
}

/// ```python
/// def compute(x, y, z) -> int:   # x→col 13, y→col 16, z→col 19
///     return 0
/// ```
#[test]
fn e0001_three_unannotated_params() {
    let diags = run("errors/e0001_multi_param.py");
    let src = std::fs::read_to_string(fixture("errors/e0001_multi_param.py")).unwrap();
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0001", "`x`", 1, 13),
            Expected::error("BSK-E0001", "`y`", 1, 16),
            Expected::error("BSK-E0001", "`z`", 1, 19),
        ],
    );
}

/// ```python
/// def log(*messages, level: str) -> None:   # *messages unannotated, col 10
///     pass
/// ```
#[test]
fn e0001_unannotated_vararg() {
    let diags = run("errors/e0001_varargs.py");
    let src = std::fs::read_to_string(fixture("errors/e0001_varargs.py")).unwrap();
    assert_diagnostics(
        &src,
        &diags,
        &[Expected::error("BSK-E0001", "`messages`", 1, 10)],
    );
}

/// ```python
/// def configure(**options) -> None:   # **options unannotated, col 17
///     pass
/// ```
#[test]
fn e0001_unannotated_kwarg() {
    let diags = run("errors/e0001_kwargs.py");
    let src = std::fs::read_to_string(fixture("errors/e0001_kwargs.py")).unwrap();
    assert_diagnostics(
        &src,
        &diags,
        &[Expected::error("BSK-E0001", "`options`", 1, 17)],
    );
}

/// ```python
/// def outer(x: int) -> int:
///     def inner(y) -> int:   # `y` unannotated, line 2, col 15
///         return x + y
///
///     return inner(1)
/// ```
#[test]
fn e0001_unannotated_param_in_nested_function() {
    let diags = run("errors/e0001_nested.py");
    let src = std::fs::read_to_string(fixture("errors/e0001_nested.py")).unwrap();
    assert_diagnostics(
        &src,
        &diags,
        &[Expected::error("BSK-E0001", "`y`", 2, 15)],
    );
}

// ---------------------------------------------------------------------------
// E0002 — Missing return type annotation
// ---------------------------------------------------------------------------

/// ```python
/// def fetch(url: str):   # `fetch` at col 5, line 1
///     pass
/// ```
#[test]
fn e0002_single_function_missing_return() {
    let diags = run("errors/e0002_single_func.py");
    let src = std::fs::read_to_string(fixture("errors/e0002_single_func.py")).unwrap();
    assert_diagnostics(
        &src,
        &diags,
        &[Expected::error("BSK-E0002", "`fetch`", 1, 5)],
    );
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
fn e0002_three_functions_all_missing_return() {
    let diags = run("errors/e0002_multiple_funcs.py");
    let src = std::fs::read_to_string(fixture("errors/e0002_multiple_funcs.py")).unwrap();
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0002", "`fetch`", 1, 5),
            Expected::error("BSK-E0002", "`compute`", 5, 5),
            Expected::error("BSK-E0002", "`noop`", 9, 5),
        ],
    );
}

// ---------------------------------------------------------------------------
// E0001 + E0002 — Mixed, class methods
// ---------------------------------------------------------------------------

/// ```python
/// class Service:
///     def connect(self, host, port):    # self/host/port unannotated + missing return
///         pass
///
///     def disconnect(self: Service) -> None:   # OK
///         pass
///
///     def send(self: Service, payload: str):   # missing return
///         pass
/// ```
///
/// Expected (sorted by span start):
///   E0002 `connect` line 2, col 9   (func name span starts before params)
///   E0001 `self`    line 2, col 17
///   E0001 `host`    line 2, col 23
///   E0001 `port`    line 2, col 29
///   E0002 `send`    line 8, col 9
#[test]
fn e0001_and_e0002_class_methods() {
    let diags = run("errors/e0001_and_e0002_class_methods.py");
    let src =
        std::fs::read_to_string(fixture("errors/e0001_and_e0002_class_methods.py")).unwrap();
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0002", "`connect`", 2, 9),
            Expected::error("BSK-E0001", "`self`", 2, 17),
            Expected::error("BSK-E0001", "`host`", 2, 23),
            Expected::error("BSK-E0001", "`port`", 2, 29),
            Expected::error("BSK-E0002", "`send`", 8, 9),
        ],
    );
}

// ---------------------------------------------------------------------------
// Clean fixtures — additional patterns, zero diagnostics expected
// ---------------------------------------------------------------------------

#[test]
fn clean_typed_generics_is_silent() {
    let diags = run("clean/typed_generics.py");
    assert!(diags.is_empty(), "typed_generics.py must produce no diagnostics, got:\n{diags:#?}");
}

#[test]
fn clean_typed_optional_is_silent() {
    let diags = run("clean/typed_optional.py");
    assert!(diags.is_empty(), "typed_optional.py must produce no diagnostics, got:\n{diags:#?}");
}

#[test]
fn clean_typed_inheritance_is_silent() {
    let diags = run("clean/typed_inheritance.py");
    assert!(diags.is_empty(), "typed_inheritance.py must produce no diagnostics, got:\n{diags:#?}");
}

#[test]
fn clean_typed_dataclass_style_is_silent() {
    let diags = run("clean/typed_dataclass_style.py");
    assert!(diags.is_empty(), "typed_dataclass_style.py must produce no diagnostics, got:\n{diags:#?}");
}

#[test]
fn clean_typed_control_flow_is_silent() {
    let diags = run("clean/typed_control_flow.py");
    assert!(diags.is_empty(), "typed_control_flow.py must produce no diagnostics, got:\n{diags:#?}");
}

// ---------------------------------------------------------------------------
// E0002 — dunder methods without return annotations
//
// class Vector:
//     def __init__(self: Vector, x: float, y: float):   # line 2, col 9
//     def __repr__(self: Vector):                        # line 6, col 9
//     def __add__(self: Vector, other: Vector):          # line 9, col 9
//     def __len__(self: Vector):                         # line 12, col 9
// ---------------------------------------------------------------------------

#[test]
fn e0002_dunder_methods_all_missing_return() {
    let diags = run("errors/e0002_dunder_methods.py");
    let src = std::fs::read_to_string(fixture("errors/e0002_dunder_methods.py")).unwrap();
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0002", "`__init__`", 2, 9),
            Expected::error("BSK-E0002", "`__repr__`", 6, 9),
            Expected::error("BSK-E0002", "`__add__`",  9, 9),
            Expected::error("BSK-E0002", "`__len__`",  12, 9),
        ],
    );
}

// ---------------------------------------------------------------------------
// E0001 — mixed: some params annotated, some not
//
// def transfer(source: str, destination, amount: float, currency) -> bool:
//                           ^^^^^^^^^^^                 ^^^^^^^^
//                           col 27                      col 55
// ---------------------------------------------------------------------------

#[test]
fn e0001_only_unannotated_params_flagged_in_mixed_signature() {
    let diags = run("errors/e0001_mixed_annotated.py");
    let src = std::fs::read_to_string(fixture("errors/e0001_mixed_annotated.py")).unwrap();
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0001", "`destination`", 1, 27),
            Expected::error("BSK-E0001", "`currency`",    1, 55),
        ],
    );
}

// ---------------------------------------------------------------------------
// E0001 — positional-only parameters (before /) without annotations
//
// def divide(numerator, denominator, /) -> float:
//            ^^^^^^^^^  ^^^^^^^^^^^
//            col 12     col 23
// ---------------------------------------------------------------------------

#[test]
fn e0001_positional_only_params_flagged() {
    let diags = run("errors/e0001_posonly_params.py");
    let src = std::fs::read_to_string(fixture("errors/e0001_posonly_params.py")).unwrap();
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0001", "`numerator`",   1, 12),
            Expected::error("BSK-E0001", "`denominator`", 1, 23),
        ],
    );
}

// ---------------------------------------------------------------------------
// E0002 — missing return on innermost nested function only
//
// def outer(x: int) -> int:
//     def middle(y: int) -> int:
//         def inner(z: int):    # line 3, col 13  ← only this is broken
//             return x + y + z
// ---------------------------------------------------------------------------

#[test]
fn e0002_only_innermost_nested_function_missing_return() {
    let diags = run("errors/e0002_deeply_nested.py");
    let src = std::fs::read_to_string(fixture("errors/e0002_deeply_nested.py")).unwrap();
    assert_diagnostics(
        &src,
        &diags,
        &[Expected::error("BSK-E0002", "`inner`", 3, 13)],
    );
}

// ---------------------------------------------------------------------------
// E0001 — keyword-only params (after *) without annotations
//
// def render(*, width: int, height, background, scale: float) -> str:
//                           ^^^^^^  ^^^^^^^^^^
//                           col 27  col 35
// ---------------------------------------------------------------------------

#[test]
fn e0001_unannotated_keyword_only_params_flagged() {
    let diags = run("errors/e0001_kwonly_params.py");
    let src = std::fs::read_to_string(fixture("errors/e0001_kwonly_params.py")).unwrap();
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0001", "`height`",     1, 27),
            Expected::error("BSK-E0001", "`background`", 1, 35),
        ],
    );
}

// ---------------------------------------------------------------------------
// E0001 + E0002 — four module-level functions, all completely untyped
//
// def parse(raw):       line 1  param col 11  func col 5
// def validate(value):  line 5  param col 14  func col 5
// def transform(data):  line 9  param col 15  func col 5
// def serialize(obj):   line 13 param col 15  func col 5
// ---------------------------------------------------------------------------

#[test]
fn e0001_and_e0002_four_completely_untyped_functions() {
    let diags = run("errors/e0001_and_e0002_module_level.py");
    let src = std::fs::read_to_string(fixture("errors/e0001_and_e0002_module_level.py")).unwrap();
    assert_diagnostics(
        &src,
        &diags,
        &[
            // E0001s come from params; E0002s from func names — sorted by span start
            Expected::error("BSK-E0002", "`parse`",     1,  5),
            Expected::error("BSK-E0001", "`raw`",       1, 11),
            Expected::error("BSK-E0002", "`validate`",  5,  5),
            Expected::error("BSK-E0001", "`value`",     5, 14),
            Expected::error("BSK-E0002", "`transform`", 9,  5),
            Expected::error("BSK-E0001", "`data`",      9, 15),
            Expected::error("BSK-E0002", "`serialize`", 13, 5),
            Expected::error("BSK-E0001", "`obj`",       13, 15),
        ],
    );
}

// ---------------------------------------------------------------------------
// E0002 — function inside else branch of version guard
//
// if sys.version_info >= (3, 11):
//     def new_feature(x: int) -> int:   # OK
// else:
//     def new_feature(x: int):          # line 7, col 9 — missing return
// ---------------------------------------------------------------------------

#[test]
fn e0002_function_in_else_branch_of_version_guard() {
    let diags = run("errors/e0002_in_if_block.py");
    let src = std::fs::read_to_string(fixture("errors/e0002_in_if_block.py")).unwrap();
    assert_diagnostics(
        &src,
        &diags,
        &[Expected::error("BSK-E0002", "`new_feature`", 7, 9)],
    );
}

// ---------------------------------------------------------------------------
// E0001 + E0002 — subclass overrides with missing annotations
//
// class Child(Base):
//     def process(self, data):    # self col 17, data col 23, func col 9  — line 7
//     def extra(self, value):     # self col 15, value col 21, func col 9 — line 10
// ---------------------------------------------------------------------------

#[test]
fn e0001_and_e0002_subclass_override_missing_annotations() {
    let diags = run("errors/e0001_and_e0002_inheritance.py");
    let src = std::fs::read_to_string(fixture("errors/e0001_and_e0002_inheritance.py")).unwrap();
    assert_diagnostics(
        &src,
        &diags,
        &[
            Expected::error("BSK-E0002", "`process`", 7,  9),
            Expected::error("BSK-E0001", "`self`",    7, 17),
            Expected::error("BSK-E0001", "`data`",    7, 23),
            Expected::error("BSK-E0002", "`extra`",   10, 9),
            Expected::error("BSK-E0001", "`self`",    10, 15),
            Expected::error("BSK-E0001", "`value`",   10, 21),
        ],
    );
}
