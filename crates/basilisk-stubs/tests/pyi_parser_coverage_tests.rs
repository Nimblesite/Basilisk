//! Tests for [STUBRES-ENGINE]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-ENGINE
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Coverage tests for uncovered paths in `pyi_parser.rs`.

use std::io::Write;
use std::path::Path;

use basilisk_stubs::types::{StubParamKind, StubSource, StubTier};
use basilisk_stubs::{parse_pyi_file, parse_pyi_source, StubModule, StubParseError};

fn parse_stub(source: &str) -> StubModule {
    parse_pyi_source(
        source,
        Path::new("test.pyi"),
        "test",
        StubSource::UserStub,
        StubTier::Tier1,
    )
    .expect("stub should parse")
}

// ── parse_pyi_file from disk ──

#[test]
fn parse_pyi_file_from_disk() {
    let mut tmp = tempfile::NamedTempFile::new().expect("create temp file");
    writeln!(tmp, "def greet(name: str) -> str: ...").expect("write");
    let path = tmp.path();
    let module = parse_pyi_file(path, "greet_mod", StubSource::UserStub, StubTier::Tier1)
        .expect("should parse from disk");
    assert!(module.functions.contains_key("greet"));
    let func = module.functions.get("greet").expect("greet");
    assert_eq!(func.return_type.as_deref(), Some("str"));
}

#[test]
fn parse_pyi_file_io_error() {
    let result = parse_pyi_file(
        Path::new("/nonexistent/path/to/file.pyi"),
        "missing",
        StubSource::UserStub,
        StubTier::Tier1,
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, StubParseError::Io { .. }));
    let msg = err.to_string();
    assert!(msg.contains("failed to read stub file"));
}

// ── Syntax error path ──

#[test]
fn parse_pyi_source_syntax_error() {
    let result = parse_pyi_source(
        "def broken(: ...",
        Path::new("bad.pyi"),
        "bad",
        StubSource::UserStub,
        StubTier::Tier1,
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, StubParseError::Syntax { .. }));
    let msg = err.to_string();
    assert!(msg.contains("syntax error"));
}

// ── ann_assign_target_name returning None ──

#[test]
fn annotated_assign_non_name_target_ignored() {
    // x.y: int should be silently ignored (not added to variables)
    let stub = parse_stub("x = object()\nx.y: int = 5\nz: str\n");
    // z should be there, but x.y should NOT be added as a variable
    assert!(stub.variables.contains_key("z"));
    // The attribute target is not a Name, so it's skipped
    assert!(!stub.variables.contains_key("x.y"));
}

// ── Attribute decorators ──

#[test]
fn dotted_decorator_name() {
    let source = "\
@typing.final
def process(x: int) -> int: ...
";
    let stub = parse_stub(source);
    let func = stub.functions.get("process").expect("process");
    // Covers Expr::Attribute arm in expr_to_decorator_name
    assert!(func.decorators.contains(&"typing.final".to_owned()));
}

// ── Call decorator ──

#[test]
fn call_decorator() {
    let source = "\
@functools.lru_cache(maxsize=128)
def expensive(n: int) -> int: ...
";
    let stub = parse_stub(source);
    let func = stub.functions.get("expensive").expect("expensive");
    assert!(func.decorators.contains(&"functools.lru_cache".to_owned()));
}

// ── Wildcard decorator (not Name, Attribute, or Call) ──

#[test]
fn unknown_decorator_expression() {
    // A lambda as a decorator is syntactically valid but matches the wildcard arm
    let source = "\
@(lambda f: f)
def identity(x: int) -> int: ...
";
    let stub = parse_stub(source);
    let func = stub.functions.get("identity").expect("identity");
    // Wildcard arm returns empty string
    assert!(func.decorators.iter().any(String::is_empty));
}

// ── Attribute annotation (typing.Optional[int]) ──

#[test]
fn dotted_annotation() {
    let source = "def func(x: typing.Optional[int]) -> typing.Dict[str, int]: ...\n";
    let stub = parse_stub(source);
    let func = stub.functions.get("func").expect("func");
    let p0 = func.params.first().expect("param 0");
    assert_eq!(p0.annotation.as_deref(), Some("typing.Optional[int]"));
    assert_eq!(func.return_type.as_deref(), Some("typing.Dict[str, int]"));
}

// ── StringLiteral annotation ──

#[test]
fn string_literal_annotation() {
    let source = "def func(x: \"ForwardRef\") -> \"ReturnRef\": ...\n";
    let stub = parse_stub(source);
    let func = stub.functions.get("func").expect("func");
    let p0 = func.params.first().expect("param 0");
    assert_eq!(p0.annotation.as_deref(), Some("\"ForwardRef\""));
    assert_eq!(func.return_type.as_deref(), Some("\"ReturnRef\""));
}

// ── NumberLiteral annotation ──

#[test]
fn number_literal_annotation() {
    let source = "x: 42\ny: 3.14\n";
    let stub = parse_stub(source);
    let x = stub.variables.get("x").expect("x");
    assert_eq!(x.annotation.as_deref(), Some("42"));
    let y = stub.variables.get("y").expect("y");
    assert_eq!(y.annotation.as_deref(), Some("3.14"));
}

#[test]
fn complex_number_annotation() {
    let source = "x: 1+2j\n";
    let _stub = parse_stub(source);
    // Complex numbers are parsed as BinOp(1, Add, 2j) by ruff, not as NumberLiteral
    // But a bare complex literal like 2j might be NumberLiteral::Complex
    let source_complex = "def func(x: 2j) -> None: ...\n";
    let stub_complex = parse_stub(source_complex);
    let func = stub_complex.functions.get("func").expect("func");
    let p0 = func.params.first().expect("param 0");
    // Should exercise the Complex arm of NumberLiteral
    assert!(p0.annotation.is_some());
}

// ── BooleanLiteral annotation ──

#[test]
fn boolean_literal_annotation() {
    let source = "x: True\ny: False\n";
    let stub = parse_stub(source);
    let x = stub.variables.get("x").expect("x");
    assert_eq!(x.annotation.as_deref(), Some("True"));
    let y = stub.variables.get("y").expect("y");
    assert_eq!(y.annotation.as_deref(), Some("False"));
}

// ── Starred annotation (PEP 646 TypeVarTuple) ──

#[test]
fn starred_annotation() {
    let source = "def func(*args: *tuple[int, ...]) -> None: ...\n";
    let stub = parse_stub(source);
    let func = stub.functions.get("func").expect("func");
    let p0 = func.params.first().expect("param 0");
    // The annotation on *args would be "*tuple[int, ...]"
    assert!(p0.annotation.is_some());
    assert_eq!(p0.kind, StubParamKind::Vararg);
}

// ── Wildcard annotation (fallback to "Unknown") ──

#[test]
fn unknown_annotation_expression() {
    // A set literal as an annotation — nonsensical but parseable
    let source = "x: {1, 2, 3}\n";
    let stub = parse_stub(source);
    let x = stub.variables.get("x").expect("x");
    assert_eq!(x.annotation.as_deref(), Some("Unknown"));
}

// ── Positional-only method parameter skipping (line 355 continue) ──

#[test]
fn method_positional_only_self_skipped() {
    // self and y are positional-only; self gets skipped via `continue` (line 355)
    // y stays since it's not the first. x is regular but skip_first is false
    // because result is non-empty after y is added.
    let source = "\
class MyClass:
    def method(self, y: str, /, x: int) -> None: ...
";
    let stub = parse_stub(source);
    let cls = stub.classes.get("MyClass").expect("MyClass");
    let method = cls.methods.first().expect("method");
    // self is positional-only and should be skipped, y and x remain
    assert_eq!(method.params.len(), 2);
    assert_eq!(method.params[0].name, "y");
    assert_eq!(method.params[1].name, "x");
}

// ── List annotation ──

#[test]
fn list_annotation() {
    let source = "def func(x: [int, str]) -> None: ...\n";
    let stub = parse_stub(source);
    let func = stub.functions.get("func").expect("func");
    let p0 = func.params.first().expect("param 0");
    assert_eq!(p0.annotation.as_deref(), Some("[int, str]"));
}

// ── Module-level variable via assignment (type alias) with dotted annotation ──

#[test]
fn dotted_class_attribute_annotation() {
    let source = "\
class Config:
    timeout: typing.Optional[int]
    name: collections.abc.Sequence[str]
";
    let stub = parse_stub(source);
    let cls = stub.classes.get("Config").expect("Config");
    assert_eq!(cls.attributes.len(), 2);
    let timeout = cls
        .attributes
        .iter()
        .find(|a| a.name == "timeout")
        .expect("timeout");
    assert_eq!(timeout.annotation.as_deref(), Some("typing.Optional[int]"));
}
