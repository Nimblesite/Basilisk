mod common;

use common::resolve_src;

#[test]
fn collect_from_stmt_for_loop_body_has_nested_function() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def outer() -> None:\n",
        "    for i in range(3):\n",
        "        def inner(x: int) -> None:\n",
        "            pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let names: Vec<&str> = resolved.functions.iter().map(|f| f.name.as_str()).collect();
    assert!(
        names.contains(&"inner"),
        "inner must be collected from for-body"
    );
    Ok(())
}

#[test]
fn collect_from_stmt_while_body_has_nested_function() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def outer() -> None:\n",
        "    while False:\n",
        "        def inner(x: int) -> None:\n",
        "            pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let names: Vec<&str> = resolved.functions.iter().map(|f| f.name.as_str()).collect();
    assert!(
        names.contains(&"inner"),
        "inner must be collected from while-body"
    );
    Ok(())
}

#[test]
fn collect_from_stmt_with_body_has_nested_function() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def outer() -> None:\n",
        "    with open('f') as g:\n",
        "        def inner(x: int) -> None:\n",
        "            pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let names: Vec<&str> = resolved.functions.iter().map(|f| f.name.as_str()).collect();
    assert!(
        names.contains(&"inner"),
        "inner must be collected from with-body"
    );
    Ok(())
}

#[test]
fn collect_from_handlers_collects_function_in_except() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def outer() -> None:\n",
        "    try:\n",
        "        pass\n",
        "    except Exception:\n",
        "        def inner(x: int) -> None:\n",
        "            pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let names: Vec<&str> = resolved.functions.iter().map(|f| f.name.as_str()).collect();
    assert!(
        names.contains(&"inner"),
        "inner must be collected from except handler"
    );
    Ok(())
}
