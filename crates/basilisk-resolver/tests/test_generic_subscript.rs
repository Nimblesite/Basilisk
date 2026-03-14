//! Tests for resolver: `test_generic_subscript`.

mod common;

use common::resolve_src;

#[test]
fn generic_subscript_site_bare_expr() -> Result<(), Box<dyn std::error::Error>> {
    let src = "list[int]\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.generic_subscript_sites.len(), 1);
    assert_eq!(resolved.generic_subscript_sites[0].base_name, "list");
    assert_eq!(resolved.generic_subscript_sites[0].arg_count, 1);
    Ok(())
}

#[test]
fn generic_subscript_site_annotated_assign() -> Result<(), Box<dyn std::error::Error>> {
    let src = "x: dict[str, int] = {}\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.generic_subscript_sites.len(), 1);
    assert_eq!(resolved.generic_subscript_sites[0].base_name, "dict");
    assert_eq!(resolved.generic_subscript_sites[0].arg_count, 2);
    Ok(())
}

#[test]
fn generic_subscript_site_function_param() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def process(data: list[int]) -> None:\n", "    pass\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.generic_subscript_sites.len(), 1);
    assert_eq!(resolved.generic_subscript_sites[0].base_name, "list");
    Ok(())
}

#[test]
fn generic_subscript_site_in_class_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("class Foo:\n", "    x: list[int]\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.generic_subscript_sites.is_empty());
    Ok(())
}

#[test]
fn generic_subscript_in_nested_function() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def outer() -> None:\n",
        "    def inner(x: list[int]) -> None:\n",
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.generic_subscript_sites.is_empty());
    Ok(())
}

#[test]
fn generic_subscript_site_list_int() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("from typing import List\n", "x: List[int] = []\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.generic_subscript_sites.is_empty());
    Ok(())
}

#[test]
fn generic_subscript_in_function_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Dict\n",
        "def foo(x: Dict[str, int]) -> Dict[str, int]:\n",
        "    return x\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.generic_subscript_sites.is_empty());
    Ok(())
}
