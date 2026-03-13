mod common;

use common::{resolve_src};

#[test]
fn assert_type_in_function_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import assert_type\n",
        "def foo(x: int) -> None:\n",
        "    assert_type(x, int)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.assert_type_calls.is_empty());
    Ok(())
}

#[test]
fn assert_type_in_class_method() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import assert_type\n",
        "class Foo:\n",
        "    def bar(self, x: int) -> None:\n",
        "        assert_type(x, int)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.assert_type_calls.is_empty());
    Ok(())
}

#[test]
fn assert_type_in_if_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import assert_type\n",
        "def foo(x: int) -> None:\n",
        "    if True:\n",
        "        assert_type(x, int)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.assert_type_calls.is_empty());
    Ok(())
}

#[test]
fn assert_type_in_for_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import assert_type\n",
        "def foo(xs: list) -> None:\n",
        "    for x in xs:\n",
        "        assert_type(x, int)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.assert_type_calls.is_empty());
    Ok(())
}

#[test]
fn assert_type_in_while_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import assert_type\n",
        "def foo(x: int) -> None:\n",
        "    while True:\n",
        "        assert_type(x, int)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.assert_type_calls.is_empty());
    Ok(())
}

#[test]
fn assert_type_in_with_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import assert_type\n",
        "def foo(x: int) -> None:\n",
        "    with open('f') as fh:\n",
        "        assert_type(x, int)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.assert_type_calls.is_empty());
    Ok(())
}

#[test]
fn assert_type_in_try_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import assert_type\n",
        "def foo(x: int) -> None:\n",
        "    try:\n",
        "        assert_type(x, int)\n",
        "    except:\n",
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.assert_type_calls.is_empty());
    Ok(())
}

#[test]
fn assert_type_in_except_handler() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import assert_type\n",
        "def foo(x: int) -> None:\n",
        "    try:\n",
        "        pass\n",
        "    except Exception:\n",
        "        assert_type(x, int)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.assert_type_calls.is_empty());
    Ok(())
}
