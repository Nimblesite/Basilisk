//! Tests for [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
// Tests for resolver: `test_function_properties`.

use super::common::resolve_src;

#[test]
fn async_function_is_async_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("async def foo() -> None:\n", "    pass\n",).to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(func.is_some_and(|f| f.is_async));
    Ok(())
}

#[test]
fn function_vararg_kwarg_info() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo(*args: int, **kwargs: str) -> None:\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(func.as_ref().and_then(|f| f.vararg.as_ref()).is_some());
    assert!(func.as_ref().and_then(|f| f.kwarg.as_ref()).is_some());
    Ok(())
}

#[test]
fn function_return_name_refs() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo(x: int) -> int:\n",
        "    result = x + 1\n",
        "    return result\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(!func.is_none_or(|f| f.return_name_refs.is_empty()));
    Ok(())
}

#[test]
fn function_kwonly_parameters() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def foo(*, x: int, y: str) -> None:\n", "    pass\n",).to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(func.is_some_and(|f| f.parameters.len() >= 2));
    Ok(())
}

#[test]
fn function_body_ends_with_return_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def foo(x: int) -> int:\n", "    return x\n",).to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(func.is_some_and(|f| f.body_ends_with_return));
    Ok(())
}

#[test]
fn function_body_last_stmt_raise() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def foo() -> None:\n", "    raise ValueError('oops')\n",).to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(func.is_some_and(|f| f.body_last_stmt_terminates));
    Ok(())
}

#[test]
fn function_pep695_type_params() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def foo[T](x: T) -> T:\n", "    return x\n",).to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(func.is_some_and(|f| f.has_pep695_type_params));
    assert!(func.is_some_and(|f| f.pep695_type_param_names.contains(&"T".to_string())));
    Ok(())
}

#[test]
fn function_local_vars_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    x: int = 5\n",
        "    y: str = 'hi'\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(func.is_some_and(|f| f.local_vars.len() >= 2));
    Ok(())
}

#[test]
fn function_posonly_params() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo(x: int, y: int, /, z: int) -> None:\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(func.is_some_and(|f| f.parameters.len() >= 3));
    Ok(())
}
