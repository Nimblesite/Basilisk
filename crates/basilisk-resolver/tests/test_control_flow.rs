mod common;

use common::resolve_src;

#[test]
fn resolves_for_loop_with_else_clause() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "def foo() -> None:\n    for i in range(10):\n        pass\n    else:\n        pass\n"
            .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.functions.len(), 1);
    assert_eq!(resolved.functions[0].name, "foo");
    Ok(())
}

#[test]
fn resolves_while_loop_with_else_clause() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "def foo() -> None:\n    while True:\n        break\n    else:\n        pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.functions.len(), 1);
    Ok(())
}

#[test]
fn resolves_with_statement() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    with open('f') as g:\n        pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.functions.len(), 1);
    Ok(())
}

#[test]
fn resolves_try_except_else_finally() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    try:\n",
        "        pass\n",
        "    except Exception:\n",
        "        pass\n",
        "    else:\n",
        "        pass\n",
        "    finally:\n",
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.functions.len(), 1);
    Ok(())
}

#[test]
fn non_module_level_imports_not_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    import os\n    from sys import argv\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.imports.is_empty(),
        "function-level imports must not be collected"
    );
    Ok(())
}

#[test]
fn non_module_level_assigns_not_collected_as_vars() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    x = 1\n    y: int = 2\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.module_vars.is_empty(),
        "function-level assigns must not be collected as module vars"
    );
    Ok(())
}

#[test]
fn collects_return_from_for_loop() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "def foo() -> int:\n    for i in range(10):\n        return i\n    return 0\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    assert!(
        func.return_stmts.len() >= 2,
        "must find returns inside for loop"
    );
    Ok(())
}

#[test]
fn collects_return_from_while_loop() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> int:\n    while True:\n        return 1\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    assert_eq!(func.return_stmts.len(), 1);
    assert!(func.return_stmts[0].has_value);
    Ok(())
}

#[test]
fn collects_return_from_with_statement() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> int:\n    with open('f') as f:\n        return 1\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    assert_eq!(func.return_stmts.len(), 1);
    assert!(func.return_stmts[0].has_value);
    Ok(())
}

#[test]
fn collects_return_from_try_and_except() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> int:\n",
        "    try:\n",
        "        return 1\n",
        "    except Exception:\n",
        "        return 0\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    assert_eq!(
        func.return_stmts.len(),
        2,
        "return in try body + return in except body = 2"
    );
    Ok(())
}

#[test]
fn collects_return_none_as_no_value() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    return None\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    assert_eq!(func.return_stmts.len(), 1);
    assert!(
        !func.return_stmts[0].has_value,
        "return None must be has_value=false"
    );
    Ok(())
}

#[test]
fn collects_return_name_from_for_loop() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "def foo() -> int:\n    for i in range(10):\n        return i\n    return 0\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    assert!(
        func.return_name_refs.iter().any(|(name, _)| name == "i"),
        "return i inside for must be collected"
    );
    Ok(())
}

#[test]
fn collects_return_name_from_while_loop() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> int:\n    while True:\n        return result\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    assert!(func
        .return_name_refs
        .iter()
        .any(|(name, _)| name == "result"));
    Ok(())
}

#[test]
fn collects_return_name_from_with_statement() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> int:\n    with open('f') as ctx:\n        return val\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    assert!(func.return_name_refs.iter().any(|(name, _)| name == "val"));
    Ok(())
}

#[test]
fn collects_return_name_from_try_block() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> int:\n",
        "    try:\n",
        "        return a\n",
        "    except Exception:\n",
        "        return b\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    assert!(func.return_name_refs.iter().any(|(name, _)| name == "a"));
    assert!(func.return_name_refs.iter().any(|(name, _)| name == "b"));
    Ok(())
}

#[test]
fn calls_collected_from_nested_if() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    pass\n",
        "if True:\n",
        "    foo()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.calls.is_empty());
    Ok(())
}

#[test]
fn reveal_type_calls_in_if_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("x = 5\n", "if True:\n", "    reveal_type(x)\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.reveal_type_calls.is_empty());
    Ok(())
}
