mod common;

use basilisk_parser::parse_source;
use basilisk_resolver::resolve;
use common::resolve_src;

#[test]
fn detects_unannotated_parameter() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def process(data) -> None:\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;

    assert_eq!(resolved.functions.len(), 1);
    let func = &resolved.functions[0];
    assert_eq!(func.name, "process");
    assert_eq!(func.parameters.len(), 1);
    assert!(!func.parameters[0].has_annotation);
    assert!(func.return_annotation.is_present());
    Ok(())
}

#[test]
fn detects_fully_annotated_function() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def greet(name: str) -> str:\n    return name\n".to_owned();
    let resolved = resolve_src(&src)?;

    assert_eq!(resolved.functions.len(), 1);
    let func = &resolved.functions[0];
    assert!(func.parameters[0].has_annotation);
    assert!(func.return_annotation.is_present());
    Ok(())
}

#[test]
fn detects_missing_return_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def fetch(url: str):\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;

    assert!(!resolved.functions[0].return_annotation.is_present());
    Ok(())
}

#[test]
fn finds_nested_functions() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def outer() -> None:\n    def inner(x: int) -> None:\n        pass\n".to_owned();
    let resolved = resolve_src(&src)?;

    assert_eq!(
        resolved.functions.len(),
        2,
        "should find both outer and inner"
    );
    let names: Vec<&str> = resolved.functions.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"outer"));
    assert!(names.contains(&"inner"));
    Ok(())
}

#[test]
fn handles_methods_in_class() -> Result<(), Box<dyn std::error::Error>> {
    let src = "class Foo:\n    def bar(self, x: int) -> None:\n        pass\n".to_owned();
    let resolved = resolve_src(&src)?;

    assert_eq!(resolved.functions.len(), 1);
    assert_eq!(resolved.functions[0].name, "bar");
    Ok(())
}

#[test]
fn handles_empty_module() -> Result<(), Box<dyn std::error::Error>> {
    let src = String::new();
    let parsed = parse_source(src, "empty.py".to_owned())?;
    let resolved = resolve(&parsed)?;

    assert!(resolved.functions.is_empty());
    Ok(())
}

#[test]
fn detects_vararg_and_kwarg() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def variadic(*args: int, **kwargs: str) -> None:\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;

    let func = &resolved.functions[0];
    assert!(func.vararg.as_ref().is_some_and(|p| p.has_annotation));
    assert!(func.kwarg.as_ref().is_some_and(|p| p.has_annotation));
    Ok(())
}

#[test]
fn span_start_before_end() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: int) -> None:\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;

    let func = &resolved.functions[0];
    assert!(func.def_span.start < func.def_span.end);
    assert!(func.name_span.start < func.name_span.end);
    Ok(())
}
