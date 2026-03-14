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
