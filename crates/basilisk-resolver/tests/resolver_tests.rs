//! Integration tests for basilisk-resolver.

use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

#[test]
fn detects_unannotated_parameter() {
    let src = "def process(data) -> None:\n    pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned()).unwrap();
    let resolved = resolve(&parsed).unwrap();

    assert_eq!(resolved.functions.len(), 1);
    let func = &resolved.functions[0];
    assert_eq!(func.name, "process");
    assert_eq!(func.parameters.len(), 1);
    assert!(!func.parameters[0].has_annotation);
    assert!(func.has_return_annotation);
}

#[test]
fn detects_fully_annotated_function() {
    let src = "def greet(name: str) -> str:\n    return name\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned()).unwrap();
    let resolved = resolve(&parsed).unwrap();

    assert_eq!(resolved.functions.len(), 1);
    let func = &resolved.functions[0];
    assert!(func.parameters[0].has_annotation);
    assert!(func.has_return_annotation);
}

#[test]
fn detects_missing_return_annotation() {
    let src = "def fetch(url: str):\n    pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned()).unwrap();
    let resolved = resolve(&parsed).unwrap();

    assert!(!resolved.functions[0].has_return_annotation);
}

#[test]
fn finds_nested_functions() {
    let src = "def outer() -> None:\n    def inner(x: int) -> None:\n        pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned()).unwrap();
    let resolved = resolve(&parsed).unwrap();

    assert_eq!(
        resolved.functions.len(),
        2,
        "should find both outer and inner"
    );
    let names: Vec<&str> = resolved.functions.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"outer"));
    assert!(names.contains(&"inner"));
}

#[test]
fn handles_methods_in_class() {
    let src = "class Foo:\n    def bar(self, x: int) -> None:\n        pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned()).unwrap();
    let resolved = resolve(&parsed).unwrap();

    assert_eq!(resolved.functions.len(), 1);
    assert_eq!(resolved.functions[0].name, "bar");
}

#[test]
fn handles_empty_module() {
    let src = String::new();
    let parsed = parse_source(src, "empty.py".to_owned()).unwrap();
    let resolved = resolve(&parsed).unwrap();

    assert!(resolved.functions.is_empty());
}

#[test]
fn detects_vararg_and_kwarg() {
    let src = "def variadic(*args: int, **kwargs: str) -> None:\n    pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned()).unwrap();
    let resolved = resolve(&parsed).unwrap();

    let func = &resolved.functions[0];
    assert!(func
        .vararg
        .as_ref()
        .map(|p| p.has_annotation)
        .unwrap_or(false));
    assert!(func
        .kwarg
        .as_ref()
        .map(|p| p.has_annotation)
        .unwrap_or(false));
}

#[test]
fn span_start_before_end() {
    let src = "def foo(x: int) -> None:\n    pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned()).unwrap();
    let resolved = resolve(&parsed).unwrap();

    let func = &resolved.functions[0];
    assert!(func.def_span.start < func.def_span.end);
    assert!(func.name_span.start < func.name_span.end);
}
