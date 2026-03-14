//! Tests for resolver: test_docstring.

mod common;

use common::resolve_src;

#[test]
fn function_docstring_extracted() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    \"\"\"This is a docstring.\"\"\"\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(func.is_some());
    assert!(
        func.is_some_and(|f| f.docstring.is_some()),
        "docstring must be extracted"
    );
    Ok(())
}

#[test]
fn class_docstring_extracted() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Foo:\n",
        "    \"\"\"This is a docstring.\"\"\"\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    assert!(cls.and_then(|c| c.docstring.as_ref()).is_some());
    Ok(())
}
