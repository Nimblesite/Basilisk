mod common;

use common::resolve_src;

#[test]
fn collects_return_name_from_for_else() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> int:\n",
        "    for i in range(10):\n",
        "        pass\n",
        "    else:\n",
        "        return result\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    assert!(
        func.return_name_refs
            .iter()
            .any(|(name, _)| name == "result"),
        "return name in for-else clause must be collected"
    );
    Ok(())
}

#[test]
fn collects_return_name_from_while_else() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> int:\n",
        "    while True:\n",
        "        break\n",
        "    else:\n",
        "        return outcome\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    assert!(
        func.return_name_refs
            .iter()
            .any(|(name, _)| name == "outcome"),
        "return name in while-else clause must be collected"
    );
    Ok(())
}

#[test]
fn collects_return_from_for_else_clause() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> int:\n",
        "    for i in range(10):\n",
        "        pass\n",
        "    else:\n",
        "        return 99\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    assert_eq!(
        func.return_stmts.len(),
        1,
        "return in for-else clause must be collected"
    );
    assert!(func.return_stmts[0].has_value);
    Ok(())
}

#[test]
fn collects_return_from_while_else_clause() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> int:\n",
        "    while True:\n",
        "        break\n",
        "    else:\n",
        "        return 0\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    assert_eq!(
        func.return_stmts.len(),
        1,
        "return in while-else clause must be collected"
    );
    assert!(func.return_stmts[0].has_value);
    Ok(())
}

#[test]
fn collect_return_stmts_return_inside_for_loop() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def find(items: list) -> int:\n",
        "    for item in items:\n",
        "        return item\n",
        "    return 0\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .iter()
        .find(|f| f.name == "find")
        .ok_or("find not found")?;
    assert!(
        func.return_stmts.len() >= 2,
        "both return stmts must be collected, got {}",
        func.return_stmts.len()
    );
    Ok(())
}
