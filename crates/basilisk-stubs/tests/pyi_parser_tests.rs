//! Tests for [STUBRES-ENGINE] / [STUBRES-PYI]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-ENGINE
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for `.pyi` stub file parsing.

use std::path::Path;

use basilisk_stubs::types::{StubParamKind, StubSource, StubTier};
use basilisk_stubs::{parse_pyi_source, StubModule};

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

#[test]
fn simple_function() {
    let stub = parse_stub("def greet(name: str) -> str: ...\n");
    assert!(stub.functions.contains_key("greet"));
    let func = stub.functions.get("greet").expect("greet should exist");
    assert_eq!(func.return_type.as_deref(), Some("str"));
    assert_eq!(func.params.len(), 1);
    let p0 = func.params.first().expect("should have param 0");
    assert_eq!(p0.name, "name");
    assert_eq!(p0.annotation.as_deref(), Some("str"));
}

#[test]
fn rejects_pyi_past_the_shared_parser_depth_limit() {
    let source = format!("value = {}0{}\n", "(".repeat(201), ")".repeat(201));
    let result = parse_pyi_source(
        &source,
        Path::new("deep.pyi"),
        "deep",
        StubSource::UserStub,
        StubTier::Tier1,
    );
    assert!(
        result.is_err(),
        "stub parsing must use Basilisk's crash-safe parser boundary"
    );
}

#[test]
fn overloaded_function() {
    let source = "\
from typing import overload

@overload
def process(x: int) -> int: ...
@overload
def process(x: str) -> str: ...
def process(x: int | str) -> int | str: ...
";
    let stub = parse_stub(source);
    assert!(stub.overloads.contains_key("process"));
    let overloads = stub.overloads.get("process").expect("process overloads");
    assert_eq!(overloads.len(), 2);
    assert!(stub.functions.contains_key("process"));
    let impl_fn = stub.functions.get("process").expect("process impl");
    assert!(!impl_fn.is_overload);
}

#[test]
fn class_with_methods() {
    let source = "\
class Dog:
    name: str
    age: int
    def bark(self) -> str: ...
    def fetch(self, item: str) -> bool: ...
";
    let stub = parse_stub(source);
    assert!(stub.classes.contains_key("Dog"));
    let dog = stub.classes.get("Dog").expect("Dog should exist");
    assert_eq!(dog.attributes.len(), 2);
    assert_eq!(dog.methods.len(), 2);
    // Methods should skip `self`
    let bark = dog.methods.first().expect("should have bark");
    assert_eq!(bark.name, "bark");
    assert!(bark.params.is_empty());
    assert_eq!(bark.return_type.as_deref(), Some("str"));
}

#[test]
fn class_with_bases() {
    let source = "class MyList(list[int]): ...\n";
    let stub = parse_stub(source);
    let cls = stub.classes.get("MyList").expect("MyList should exist");
    assert_eq!(cls.bases, vec!["list[int]"]);
}

#[test]
fn module_variable() {
    let source = "VERSION: str\nDEBUG: bool\n";
    let stub = parse_stub(source);
    assert_eq!(stub.variables.len(), 2);
    let version = stub.variables.get("VERSION").expect("VERSION should exist");
    assert_eq!(version.annotation.as_deref(), Some("str"));
    let debug = stub.variables.get("DEBUG").expect("DEBUG should exist");
    assert_eq!(debug.annotation.as_deref(), Some("bool"));
}

#[test]
fn async_function() {
    let source = "async def fetch(url: str) -> bytes: ...\n";
    let stub = parse_stub(source);
    let func = stub.functions.get("fetch").expect("fetch should exist");
    assert!(func.is_async);
}

#[test]
fn complex_annotations() {
    let source =
        "def transform(data: list[dict[str, int]], flag: bool = ...) -> tuple[str, ...]: ...\n";
    let stub = parse_stub(source);
    let func = stub
        .functions
        .get("transform")
        .expect("transform should exist");
    let p0 = func.params.first().expect("should have param 0");
    assert_eq!(p0.annotation.as_deref(), Some("list[dict[str, int]]"));
    let p1 = func.params.get(1).expect("should have param 1");
    assert!(p1.has_default);
    assert_eq!(func.return_type.as_deref(), Some("tuple[str, ...]"));
}

#[test]
fn union_annotation() {
    let source = "def accept(x: int | str | None) -> bool: ...\n";
    let stub = parse_stub(source);
    let func = stub.functions.get("accept").expect("accept should exist");
    let p0 = func.params.first().expect("should have param 0");
    assert_eq!(p0.annotation.as_deref(), Some("int | str | None"));
}

#[test]
fn class_overloaded_methods() {
    let source = "\
from typing import overload

class Parser:
    @overload
    def parse(self, data: str) -> str: ...
    @overload
    def parse(self, data: bytes) -> bytes: ...
    def parse(self, data: str | bytes) -> str | bytes: ...
";
    let stub = parse_stub(source);
    let cls = stub.classes.get("Parser").expect("Parser should exist");
    assert_eq!(cls.methods.len(), 3);
    assert!(stub.overloads.contains_key("Parser.parse"));
    let overloads = stub
        .overloads
        .get("Parser.parse")
        .expect("Parser.parse overloads");
    assert_eq!(overloads.len(), 2);
}

#[test]
fn positional_only_params() {
    let source = "def div(x: float, y: float, /) -> float: ...\n";
    let stub = parse_stub(source);
    let func = stub.functions.get("div").expect("div should exist");
    assert_eq!(func.params.len(), 2);
    let p0 = func.params.first().expect("should have param 0");
    assert_eq!(p0.kind, StubParamKind::PositionalOnly);
    let p1 = func.params.get(1).expect("should have param 1");
    assert_eq!(p1.kind, StubParamKind::PositionalOnly);
}

#[test]
fn keyword_only_params() {
    let source = "def config(*, debug: bool, verbose: bool = ...) -> None: ...\n";
    let stub = parse_stub(source);
    let func = stub.functions.get("config").expect("config should exist");
    assert_eq!(func.params.len(), 2);
    let p0 = func.params.first().expect("should have param 0");
    assert_eq!(p0.kind, StubParamKind::KeywordOnly);
    let p1 = func.params.get(1).expect("should have param 1");
    assert_eq!(p1.kind, StubParamKind::KeywordOnly);
    assert!(p1.has_default);
}

#[test]
fn varargs_and_kwargs() {
    let source = "def flexible(*args: int, **kwargs: str) -> None: ...\n";
    let stub = parse_stub(source);
    let func = stub
        .functions
        .get("flexible")
        .expect("flexible should exist");
    assert_eq!(func.params.len(), 2);
    let p0 = func.params.first().expect("should have param 0");
    assert_eq!(p0.kind, StubParamKind::Vararg);
    assert_eq!(p0.annotation.as_deref(), Some("int"));
    let p1 = func.params.get(1).expect("should have param 1");
    assert_eq!(p1.kind, StubParamKind::Kwarg);
    assert_eq!(p1.annotation.as_deref(), Some("str"));
}

#[test]
fn type_alias_assignment() {
    let source = "Callback = Callable[[int], str]\n";
    let stub = parse_stub(source);
    assert!(stub.variables.contains_key("Callback"));
}

#[test]
fn static_and_class_methods() {
    let source = "\
class Util:
    @staticmethod
    def helper(x: int) -> int: ...
    @classmethod
    def create(cls, name: str) -> Util: ...
";
    let stub = parse_stub(source);
    let cls = stub.classes.get("Util").expect("Util should exist");
    let helper = cls.methods.first().expect("should have helper");
    assert!(helper.decorators.contains(&"staticmethod".to_owned()));
    let create = cls.methods.get(1).expect("should have create");
    assert!(create.decorators.contains(&"classmethod".to_owned()));
    assert_eq!(create.params.len(), 1);
    let p0 = create.params.first().expect("should have param 0");
    assert_eq!(p0.name, "name");
}

#[test]
fn empty_stub_module() {
    let stub = parse_stub("");
    assert!(stub.functions.is_empty());
    assert!(stub.classes.is_empty());
    assert!(stub.variables.is_empty());
    assert!(stub.overloads.is_empty());
}

#[test]
fn decorated_function() {
    let source = "\
from functools import cache

@cache
def expensive(n: int) -> int: ...
";
    let stub = parse_stub(source);
    let func = stub
        .functions
        .get("expensive")
        .expect("expensive should exist");
    assert!(func.decorators.contains(&"cache".to_owned()));
}
