//! Tests for resolver: `test_unhashable_keys`.

mod common;

use common::resolve_src;

#[test]
fn detects_list_key_in_dict() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    d = {[1, 2]: 'val'}\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .first()
        .expect("expected at least one function");
    assert!(
        !func.unhashable_keys.is_empty(),
        "list dict key must be detected"
    );
    assert_eq!(
        func.unhashable_keys
            .first()
            .expect("expected at least one unhashable key")
            .key_type,
        "list"
    );
    Ok(())
}

#[test]
fn detects_unhashable_key_in_if_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    if True:\n        d = {[1]: 'x'}\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .first()
        .expect("expected at least one function");
    assert!(!func.unhashable_keys.is_empty());
    Ok(())
}

#[test]
fn detects_unhashable_key_in_return_expr() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> dict:\n    return {[1]: 'x'}\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .first()
        .expect("expected at least one function");
    assert!(!func.unhashable_keys.is_empty());
    Ok(())
}

#[test]
fn detects_unhashable_key_in_expr_stmt() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    _ = {[1]: 'x'}\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .first()
        .expect("expected at least one function");
    assert!(!func.unhashable_keys.is_empty());
    Ok(())
}

#[test]
fn detects_unhashable_key_in_while_body() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "def foo() -> None:\n    while True:\n        d = {[1]: 'x'}\n        break\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .first()
        .expect("expected at least one function");
    assert!(!func.unhashable_keys.is_empty());
    Ok(())
}

#[test]
fn detects_unhashable_key_in_with_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    with open('f') as f:\n        d = {[1]: 'x'}\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .first()
        .expect("expected at least one function");
    assert!(!func.unhashable_keys.is_empty());
    Ok(())
}

#[test]
fn detects_unhashable_key_in_try_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    try:\n",
        "        d = {[1]: 'x'}\n",
        "    except Exception:\n",
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .first()
        .expect("expected at least one function");
    assert!(!func.unhashable_keys.is_empty());
    Ok(())
}

#[test]
fn detects_unhashable_key_in_for_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    for i in range(1):\n        d = {[i]: 'x'}\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .first()
        .expect("expected at least one function");
    assert!(!func.unhashable_keys.is_empty());
    Ok(())
}

#[test]
fn detects_set_key_in_dict() -> Result<(), Box<dyn std::error::Error>> {
    // {1, 2} as a dict key — set is unhashable at runtime
    let src = "def foo() -> None:\n    d = {{1, 2}: 'val'}\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .first()
        .expect("expected at least one function");
    assert!(
        !func.unhashable_keys.is_empty(),
        "set dict key must be detected"
    );
    assert_eq!(
        func.unhashable_keys
            .first()
            .expect("expected at least one unhashable key")
            .key_type,
        "set"
    );
    Ok(())
}

#[test]
fn detects_dict_key_in_dict() -> Result<(), Box<dyn std::error::Error>> {
    // {'a': 1} as a dict key — dicts are unhashable
    let src = "def foo() -> None:\n    d = {{'a': 1}: 'val'}\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .first()
        .expect("expected at least one function");
    assert!(
        !func.unhashable_keys.is_empty(),
        "dict dict key must be detected"
    );
    assert_eq!(
        func.unhashable_keys
            .first()
            .expect("expected at least one unhashable key")
            .key_type,
        "dict"
    );
    Ok(())
}

#[test]
fn detects_unhashable_key_inside_tuple_expr() -> Result<(), Box<dyn std::error::Error>> {
    // A tuple assigned as RHS — exercises the Expr::Tuple traversal path
    let src = "def foo() -> None:\n    _ = ({[1]: 2},)\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .first()
        .expect("expected at least one function");
    assert!(
        !func.unhashable_keys.is_empty(),
        "unhashable key inside tuple element must be detected"
    );
    Ok(())
}

#[test]
fn detects_unhashable_key_inside_call_arg() -> Result<(), Box<dyn std::error::Error>> {
    // A dict with list key passed as a function argument — exercises Expr::Call traversal
    let src = "def foo() -> None:\n    _ = some_func({[1]: 2})\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .first()
        .expect("expected at least one function");
    assert!(
        !func.unhashable_keys.is_empty(),
        "unhashable key inside call argument must be detected"
    );
    Ok(())
}

#[test]
fn unhashable_hash_call_on_eq_only_dataclass() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass\n",
        "@dataclass\n",
        "class MyClass:\n",
        "    x: int\n",
        "MyClass(1).__hash__()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.unhashable_hash_call_violations.is_empty(),
        "calling __hash__ on non-frozen dataclass must produce a violation"
    );
    Ok(())
}

#[test]
fn hashable_class_no_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class MyClass:\n",
        "    def __eq__(self, other: object) -> bool:\n",
        "        return True\n",
        "    def __hash__(self) -> int:\n",
        "        return 0\n",
        "MyClass().__hash__()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.unhashable_hash_call_violations.is_empty());
    Ok(())
}

#[test]
fn unhashable_hash_call_in_if_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass\n",
        "@dataclass\n",
        "class MyClass:\n",
        "    x: int\n",
        "if True:\n",
        "    MyClass(1).__hash__()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.unhashable_hash_call_violations.is_empty());
    Ok(())
}

#[test]
fn unhashable_hash_call_in_elif() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass\n",
        "@dataclass\n",
        "class MyClass:\n",
        "    x: int\n",
        "if False:\n",
        "    pass\n",
        "else:\n",
        "    MyClass(1).__hash__()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.unhashable_hash_call_violations.is_empty());
    Ok(())
}

#[test]
fn function_unhashable_keys_in_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def foo() -> None:\n", "    d = {[1, 2]: 'bad'}\n",).to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(!func.is_none_or(|f| f.unhashable_keys.is_empty()));
    Ok(())
}

#[test]
fn unhashable_keys_in_return_dict() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def foo() -> None:\n", "    return {[1]: 'bad'}\n",).to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(!func.is_none_or(|f| f.unhashable_keys.is_empty()));
    Ok(())
}
