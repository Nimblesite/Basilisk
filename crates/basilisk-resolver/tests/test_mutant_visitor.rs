//! Tests for resolver: test_mutant_visitor.

mod common;

use common::resolve_src;

#[test]
fn collect_unconditional_assigns_ann_assign() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> str:\n",
        "    result: str = 'hello'\n",
        "    return result\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .iter()
        .find(|f| f.name == "foo")
        .ok_or("foo not found")?;
    assert!(
        func.unconditional_assigns.contains(&"result".to_owned()),
        "annotated assign must appear in unconditional_assigns"
    );
    Ok(())
}

#[test]
fn collect_unconditional_assigns_for_target() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    for item in range(3):\n",
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .iter()
        .find(|f| f.name == "foo")
        .ok_or("foo not found")?;
    assert!(
        func.unconditional_assigns.contains(&"item".to_owned()),
        "for loop variable must appear in unconditional_assigns"
    );
    Ok(())
}

#[test]
fn unhashable_keys_in_assign_stmt() -> Result<(), Box<dyn std::error::Error>> {
    let src = "d = {[1, 2]: 'bad'}\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.module_vars.is_empty(),
        "variable d must be resolved"
    );
    Ok(())
}

#[test]
fn unhashable_keys_in_return_stmt() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def bad() -> dict:\n", "    return {[1, 2]: 'bad'}\n",).to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .iter()
        .find(|f| f.name == "bad")
        .ok_or("bad not found")?;
    assert!(
        !func.unhashable_keys.is_empty(),
        "unhashable key in return must be collected"
    );
    Ok(())
}

#[test]
fn unhashable_keys_in_expr_stmt() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def f() -> None:\n", "    {[1, 2]: 'key'}\n",).to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .iter()
        .find(|f| f.name == "f")
        .ok_or("f not found")?;
    assert!(
        !func.unhashable_keys.is_empty(),
        "unhashable key in expr stmt must be collected"
    );
    Ok(())
}

#[test]
fn collect_module_level_calls_from_assign() -> Result<(), Box<dyn std::error::Error>> {
    let src = "T = TypeVar('T', int, str)\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.typevar_calls.is_empty(),
        "TypeVar in plain assignment must be collected"
    );
    Ok(())
}
