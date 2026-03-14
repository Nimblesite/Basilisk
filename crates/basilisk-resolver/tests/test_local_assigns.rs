//! Tests for resolver: `test_local_assigns`.

mod common;

use common::resolve_src;

#[test]
fn collects_ann_assign_in_function_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    x: int = 1\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .first()
        .expect("expected at least one function");
    assert!(
        func.all_local_assigns.contains(&"x".to_string()),
        "annotated assign must be collected"
    );
    Ok(())
}

#[test]
fn collects_tuple_targets_from_for_loop() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    for a, b in [(1, 2)]:\n        pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .first()
        .expect("expected at least one function");
    assert!(func.all_local_assigns.contains(&"a".to_string()));
    assert!(func.all_local_assigns.contains(&"b".to_string()));
    Ok(())
}

#[test]
fn collects_assigns_from_while_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    while True:\n        x = 1\n        break\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .first()
        .expect("expected at least one function");
    assert!(func.all_local_assigns.contains(&"x".to_string()));
    Ok(())
}

#[test]
fn collects_with_statement_variable() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    with open('f') as ctx:\n        pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .first()
        .expect("expected at least one function");
    assert!(
        func.all_local_assigns.contains(&"ctx".to_string()),
        "with-statement variable must be collected"
    );
    Ok(())
}

#[test]
fn collects_try_except_named_exception() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    try:\n",
        "        pass\n",
        "    except Exception as exc:\n",
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .first()
        .expect("expected at least one function");
    assert!(
        func.all_local_assigns.contains(&"exc".to_string()),
        "named exception binding must be collected"
    );
    Ok(())
}

#[test]
fn collects_nested_function_name_as_local_assign() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def outer() -> None:\n    def inner() -> None:\n        pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    let outer = resolved.functions.iter().find(|f| f.name == "outer");
    assert!(outer.is_some(), "outer must be present");
    let outer = outer.ok_or("outer not found")?;
    assert!(
        outer.all_local_assigns.contains(&"inner".to_string()),
        "nested function name must appear in enclosing scope's assigns"
    );
    Ok(())
}

#[test]
fn collects_assigns_from_for_else_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    for i in range(10):\n",
        "        x = 1\n",
        "    else:\n",
        "        y = 2\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .first()
        .expect("expected at least one function");
    assert!(
        func.all_local_assigns.contains(&"x".to_string()),
        "assign in for body must be collected"
    );
    assert!(
        func.all_local_assigns.contains(&"y".to_string()),
        "assign in for else body must be collected"
    );
    Ok(())
}

#[test]
fn collects_assigns_from_while_else_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    while True:\n",
        "        a = 1\n",
        "        break\n",
        "    else:\n",
        "        b = 2\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .first()
        .expect("expected at least one function");
    assert!(
        func.all_local_assigns.contains(&"a".to_string()),
        "assign in while body must be collected"
    );
    assert!(
        func.all_local_assigns.contains(&"b".to_string()),
        "assign in while else body must be collected"
    );
    Ok(())
}

#[test]
fn collects_list_target_unpacking() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def foo() -> None:\n", "    [a, b] = [1, 2]\n",).to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .first()
        .expect("expected at least one function");
    assert!(
        func.all_local_assigns.contains(&"a".to_string()),
        "first name in list-target must be collected"
    );
    assert!(
        func.all_local_assigns.contains(&"b".to_string()),
        "second name in list-target must be collected"
    );
    Ok(())
}
