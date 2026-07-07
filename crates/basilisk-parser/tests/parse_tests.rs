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

// --- Operator-chain boundary (pins MAX_EXPR_OPERATORS = 50_000, GitHub #278) ---

/// An `n`-operator `1 + 1 + …` chain (`n + 1` terms).
fn plus_chain(operators: usize) -> String {
    format!("x = {}1\n", "1 + ".repeat(operators))
}

#[test]
fn operator_chain_at_limit_parses() {
    // Exactly 50,000 chained operators is accepted. Parsing (and dropping) the
    // resulting 50,000-deep BinOp tree needs an analysis-sized stack — exactly
    // how every production entry point runs it ([LSPARCH-ARCH-STACK]) — so the
    // test provides one rather than gambling on the harness thread.
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| parse_source(plus_chain(50_000), "ok.py".to_owned()).is_ok())
        .expect("spawn analysis-stack thread");
    assert!(
        handle.join().expect("analysis-stack thread panicked"),
        "a 50,000-operator chain is at the limit and must parse"
    );
}

#[test]
fn operator_chain_one_over_limit_reports_message() {
    // The 50,001st chained operator is rejected — linearly, before the parser
    // ever builds the deep AST, so no big stack is needed here.
    match parse_source(plus_chain(50_001), "deep.py".to_owned()) {
        Err(ParseError::Syntax { message, .. }) => assert!(
            message.contains("expression too deeply nested"),
            "operator-chain rejection should explain itself, got: {message}"
        ),
        other => panic!("expected ParseError::Syntax at 50,001 operators, got {other:?}"),
    }
}

#[test]
fn flat_operators_and_commas_do_not_accumulate_chain_depth() {
    // 60,000 `+` tokens overall, but commas break every chain at length one —
    // a giant flat literal is legitimate generated code. Only passes if the
    // chain-break arm resets: deleting it would falsely reject this file.
    let source = format!("x = [{}]\n", "1 + 1, ".repeat(60_000));
    assert!(
        parse_source(source, "ok.py".to_owned()).is_ok(),
        "flat comma-separated sums must not accumulate chain depth"
    );
}

#[test]
fn nested_brackets_isolate_operator_chains() {
    // Two 30,000-operator chains — one parenthesised inside the other's line —
    // total 60,001 operators, but each bracket level stays under the limit.
    // Only passes if entering a bracket pushes a fresh counter and closing it
    // restores the outer one.
    let inner = format!("({}1)", "1 + ".repeat(30_000));
    let source = format!("x = {}{inner}\n", "1 + ".repeat(30_000));
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || parse_source(source, "ok.py".to_owned()).is_ok())
        .expect("spawn analysis-stack thread");
    assert!(
        handle.join().expect("analysis-stack thread panicked"),
        "bracketed sub-chains must not leak depth into the enclosing chain"
    );
}

#[test]
fn unbalanced_closer_does_not_disarm_the_chain_guard() {
    // A stray `)` (a syntax error ruff reports — but only if the guard lets
    // the source through) must not corrupt the module-level chain counter:
    // an over-limit chain after it is still rejected by the guard first.
    let source = format!(") \n{}", plus_chain(50_001));
    match parse_source(source, "deep.py".to_owned()) {
        Err(ParseError::Syntax { message, .. }) => assert!(
            message.contains("expression too deeply nested"),
            "the guard must survive unbalanced closers, got: {message}"
        ),
        other => panic!("expected the depth guard to reject, got {other:?}"),
    }
}

#[test]
fn deep_unbracketed_constructs_are_rejected_not_crashed() {
    // Every un-bracketed depth-building construct must hit the guard: unary
    // runs, attribute chains, ternary nests, and lambda nests all build one
    // AST level per token with zero bracket nesting.
    let cases = [
        format!("z = {}1\n", "-".repeat(60_000)),
        format!("a = 1\nx = a{}\n", ".real".repeat(60_000)),
        format!("y = {}1\n", "1 if True else ".repeat(60_000)),
        format!("f = {}1\n", "lambda: ".repeat(60_000)),
    ];
    for source in cases {
        assert!(
            matches!(
                parse_source(source, "deep.py".to_owned()),
                Err(ParseError::Syntax { .. })
            ),
            "un-bracketed deep constructs must be rejected, not crash the visitors"
        );
    }
}
