//! Tests for [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for basilisk-parser.

use basilisk_parser::{parse_file, parse_source, ParseError};

#[test]
fn parses_valid_empty_module() {
    let result = parse_source(String::new(), "empty.py".to_owned());
    assert!(result.is_ok(), "empty source should parse successfully");
}

#[test]
fn parses_simple_annotated_function() {
    let source = "def greet(name: str) -> str:\n    return name\n".to_owned();
    let result = parse_source(source, "test.py".to_owned());
    assert!(result.is_ok(), "simple annotated function should parse");
}

#[test]
fn preserves_source_and_path() -> Result<(), Box<dyn std::error::Error>> {
    let source = "x: int = 1\n".to_owned();
    let parsed = parse_source(source.clone(), "myfile.py".to_owned())?;
    assert_eq!(parsed.source, source);
    assert_eq!(parsed.path, "myfile.py");
    Ok(())
}

#[test]
fn returns_syntax_error_for_bad_source() {
    let source = "def (broken:".to_owned();
    let result = parse_source(source, "bad.py".to_owned());
    assert!(
        matches!(result, Err(ParseError::Syntax { .. })),
        "malformed syntax should return ParseError::Syntax"
    );
}

#[test]
fn returns_io_error_for_missing_file() {
    let result = parse_file("/nonexistent/path/does_not_exist.py");
    assert!(
        matches!(result, Err(ParseError::Io { .. })),
        "missing file should return ParseError::Io"
    );
}

// Tests for [CHKARCH-ARCH-PARSEDEPTH]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PARSEDEPTH
//
// The recursive-descent parser (and our recursive AST visitors) overflow the
// stack on deeply nested input — a ~4000-deep bracket file aborts the process
// with SIGABRT. parse_source must reject pathologically nested source as a
// `ParseError::Syntax` (measured by the linear lexer, never the recursive
// parser), matching CPython's tokenizer limits, instead of crashing.

/// `n` nested `if True:` blocks ending in `pass` — a non-bracket nesting vector.
fn nested_if_blocks(depth: usize) -> String {
    let mut source = String::new();
    for level in 0..depth {
        source.push_str(&"    ".repeat(level));
        source.push_str("if True:\n");
    }
    source.push_str(&"    ".repeat(depth));
    source.push_str("pass\n");
    source
}

#[test]
fn deeply_nested_brackets_are_rejected_not_crashed() {
    // 5000-deep brackets overflow ruff's recursive parser; the guard must turn
    // this into a clean syntax error rather than a stack-overflow abort.
    let source = format!("x = {}1{}\n", "(".repeat(5000), ")".repeat(5000));
    let result = parse_source(source, "deep.py".to_owned());
    assert!(
        matches!(result, Err(ParseError::Syntax { .. })),
        "deeply nested brackets must be a syntax error, not a crash"
    );
}

// --- Bracket-depth boundary (pins MAX_BRACKET_DEPTH = 200 and the `>` test) ---

#[test]
fn brackets_at_limit_parse() {
    // Exactly 200 simultaneously-open brackets is accepted (CPython MAXLEVEL).
    let source = format!("x = {}1{}\n", "(".repeat(200), ")".repeat(200));
    assert!(
        parse_source(source, "ok.py".to_owned()).is_ok(),
        "200-deep bracket nesting is at the limit and must parse"
    );
}

#[test]
fn brackets_one_over_limit_report_cpython_message() {
    // The 201st simultaneously-open bracket is rejected, matching CPython.
    let source = format!("x = {}1{}\n", "(".repeat(201), ")".repeat(201));
    match parse_source(source, "deep.py".to_owned()) {
        Err(ParseError::Syntax { message, .. }) => assert!(
            message.contains("too many nested parentheses"),
            "bracket-depth rejection should match CPython's message, got: {message}"
        ),
        other => panic!("expected ParseError::Syntax at depth 201, got {other:?}"),
    }
}

#[test]
fn mixed_bracket_kinds_share_one_depth_counter() {
    // 67 of each kind opened simultaneously = 201 cumulative depth (one past the
    // limit). Pins cross-kind counting and that all three openers increment.
    let opens = "([{".repeat(67);
    let closes = "}])".repeat(67);
    let source = format!("x = {opens}1{closes}\n");
    assert!(
        matches!(
            parse_source(source, "deep.py".to_owned()),
            Err(ParseError::Syntax { .. })
        ),
        "201 cumulative mixed-kind brackets must be rejected"
    );
}

#[test]
fn sequential_brackets_do_not_accumulate_depth() {
    // 300 each of empty tuple/list/dict in a list: 901 cumulative opens but a
    // simultaneous depth of only 2. Only passes if every close-bracket arm
    // (`)`, `]`, `}`) decrements — deleting any one would falsely reject this.
    let row = "(), [], {}, ";
    let source = format!("x = [{}]\n", row.repeat(300));
    assert!(
        parse_source(source, "ok.py".to_owned()).is_ok(),
        "shallow-but-wide bracket nesting must not accumulate depth"
    );
}

// --- Indentation boundary (pins MAX_INDENT_DEPTH = 99 and the Dedent arm) ---

#[test]
fn indentation_at_limit_parses() {
    // 99 indentation levels is accepted (CPython MAXINDENT); the body sits at 99.
    assert!(
        parse_source(nested_if_blocks(99), "ok.py".to_owned()).is_ok(),
        "99 indentation levels is at the limit and must parse"
    );
}

#[test]
fn indentation_one_over_limit_reports_cpython_message() {
    // The 100th indentation level is rejected, matching CPython.
    match parse_source(nested_if_blocks(100), "deep.py".to_owned()) {
        Err(ParseError::Syntax { message, .. }) => assert!(
            message.contains("too many levels of indentation"),
            "indentation rejection should match CPython's message, got: {message}"
        ),
        other => panic!("expected ParseError::Syntax at 100 levels, got {other:?}"),
    }
}

#[test]
fn sequential_indents_do_not_accumulate() {
    // 200 sibling `if` blocks: 200 cumulative Indent tokens but a simultaneous
    // depth of only 1. Only passes if the Dedent arm decrements — deleting it
    // would push the counter past 99 and falsely reject this valid file.
    let source = "if True:\n    pass\n".repeat(200);
    assert!(
        parse_source(source, "ok.py".to_owned()).is_ok(),
        "many shallow sibling blocks must not accumulate indentation depth"
    );
}
