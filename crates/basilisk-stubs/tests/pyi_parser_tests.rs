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
    let func = &stub.functions["greet"];
    assert_eq!(func.return_type.as_deref(), Some("str"));
    assert_eq!(func.params.len(), 1);
    assert_eq!(func.params[0].name, "name");
    assert_eq!(func.params[0].annotation.as_deref(), Some("str"));
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
    assert_eq!(stub.overloads["process"].len(), 2);
    assert!(stub.functions.contains_key("process"));
    let impl_fn = &stub.functions["process"];
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
    let dog = &stub.classes["Dog"];
    assert_eq!(dog.attributes.len(), 2);
    assert_eq!(dog.methods.len(), 2);
    // Methods should skip `self`
    let bark = &dog.methods[0];
    assert_eq!(bark.name, "bark");
    assert!(bark.params.is_empty());
    assert_eq!(bark.return_type.as_deref(), Some("str"));
}

#[test]
fn class_with_bases() {
    let source = "class MyList(list[int]): ...\n";
    let stub = parse_stub(source);
    let cls = &stub.classes["MyList"];
    assert_eq!(cls.bases, vec!["list[int]"]);
}

#[test]
fn module_variable() {
    let source = "VERSION: str\nDEBUG: bool\n";
    let stub = parse_stub(source);
    assert_eq!(stub.variables.len(), 2);
    assert_eq!(
        stub.variables["VERSION"].annotation.as_deref(),
        Some("str")
    );
    assert_eq!(
        stub.variables["DEBUG"].annotation.as_deref(),
        Some("bool")
    );
}

#[test]
fn async_function() {
    let source = "async def fetch(url: str) -> bytes: ...\n";
    let stub = parse_stub(source);
    let func = &stub.functions["fetch"];
    assert!(func.is_async);
}

#[test]
fn complex_annotations() {
    let source =
        "def transform(data: list[dict[str, int]], flag: bool = ...) -> tuple[str, ...]: ...\n";
    let stub = parse_stub(source);
    let func = &stub.functions["transform"];
    assert_eq!(
        func.params[0].annotation.as_deref(),
        Some("list[dict[str, int]]")
    );
    assert!(func.params[1].has_default);
    assert_eq!(func.return_type.as_deref(), Some("tuple[str, ...]"));
}

#[test]
fn union_annotation() {
    let source = "def accept(x: int | str | None) -> bool: ...\n";
    let stub = parse_stub(source);
    let func = &stub.functions["accept"];
    assert_eq!(
        func.params[0].annotation.as_deref(),
        Some("int | str | None")
    );
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
    let cls = &stub.classes["Parser"];
    assert_eq!(cls.methods.len(), 3);
    assert!(stub.overloads.contains_key("Parser.parse"));
    assert_eq!(stub.overloads["Parser.parse"].len(), 2);
}

#[test]
fn positional_only_params() {
    let source = "def div(x: float, y: float, /) -> float: ...\n";
    let stub = parse_stub(source);
    let func = &stub.functions["div"];
    assert_eq!(func.params.len(), 2);
    assert_eq!(func.params[0].kind, StubParamKind::PositionalOnly);
    assert_eq!(func.params[1].kind, StubParamKind::PositionalOnly);
}

#[test]
fn keyword_only_params() {
    let source = "def config(*, debug: bool, verbose: bool = ...) -> None: ...\n";
    let stub = parse_stub(source);
    let func = &stub.functions["config"];
    assert_eq!(func.params.len(), 2);
    assert_eq!(func.params[0].kind, StubParamKind::KeywordOnly);
    assert_eq!(func.params[1].kind, StubParamKind::KeywordOnly);
    assert!(func.params[1].has_default);
}

#[test]
fn varargs_and_kwargs() {
    let source = "def flexible(*args: int, **kwargs: str) -> None: ...\n";
    let stub = parse_stub(source);
    let func = &stub.functions["flexible"];
    assert_eq!(func.params.len(), 2);
    assert_eq!(func.params[0].kind, StubParamKind::Vararg);
    assert_eq!(func.params[0].annotation.as_deref(), Some("int"));
    assert_eq!(func.params[1].kind, StubParamKind::Kwarg);
    assert_eq!(func.params[1].annotation.as_deref(), Some("str"));
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
    let cls = &stub.classes["Util"];
    let helper = &cls.methods[0];
    assert!(helper.decorators.contains(&"staticmethod".to_owned()));
    let create = &cls.methods[1];
    assert!(create.decorators.contains(&"classmethod".to_owned()));
    assert_eq!(create.params.len(), 1);
    assert_eq!(create.params[0].name, "name");
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
    let func = &stub.functions["expensive"];
    assert!(func.decorators.contains(&"cache".to_owned()));
}
