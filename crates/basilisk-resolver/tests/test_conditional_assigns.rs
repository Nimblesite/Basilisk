//! Tests for resolver: `test_conditional_assigns`.

mod common;

use common::resolve_src;

#[test]
fn elif_else_functions_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "if True:\n",
        "    def foo() -> None:\n",
        "        pass\n",
        "elif False:\n",
        "    def bar() -> None:\n",
        "        pass\n",
        "else:\n",
        "    def baz() -> None:\n",
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let names: Vec<&str> = resolved.functions.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"foo"));
    assert!(names.contains(&"bar"));
    assert!(names.contains(&"baz"));
    Ok(())
}

#[test]
fn unconditional_assigns_from_if_else() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    if True:\n",
        "        x = 1\n",
        "    else:\n",
        "        x = 2\n",
        "    return x\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let Some(func) = resolved.functions.iter().find(|f| f.name == "foo") else {
        return Err("function not found".into());
    };
    assert!(
        func.unconditional_assigns.contains(&"x".to_owned()),
        "x must be unconditionally assigned through if/else"
    );
    Ok(())
}
