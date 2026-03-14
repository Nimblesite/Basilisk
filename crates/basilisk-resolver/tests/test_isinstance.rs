//! Tests for resolver: test_isinstance.

mod common;

use common::resolve_src;

#[test]
fn isinstance_typeddict_detected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "x = {}\n",
        "if isinstance(x, Movie):\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.isinstance_typeddict_violations.is_empty(),
        "isinstance with TypedDict class must produce a violation"
    );
    Ok(())
}

#[test]
fn isinstance_non_typeddict_no_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Regular:\n",
        "    pass\n",
        "x = {}\n",
        "if isinstance(x, Regular):\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.isinstance_typeddict_violations.is_empty());
    Ok(())
}

#[test]
fn isinstance_typeddict_in_assign() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "x = {}\n",
        "result = isinstance(x, Movie)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.isinstance_typeddict_violations.is_empty());
    Ok(())
}

#[test]
fn isinstance_typeddict_in_ann_assign() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "x = {}\n",
        "result: bool = isinstance(x, Movie)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.isinstance_typeddict_violations.is_empty());
    Ok(())
}

#[test]
fn isinstance_typeddict_in_function() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "def check(x: object) -> None:\n",
        "    isinstance(x, Movie)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.isinstance_typeddict_violations.is_empty());
    Ok(())
}

#[test]
fn isinstance_typeddict_in_expr_stmt() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "x = {}\n",
        "isinstance(x, Movie)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.isinstance_typeddict_violations.is_empty());
    Ok(())
}

#[test]
fn isinstance_typeddict_in_while() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "x = {}\n",
        "while isinstance(x, Movie):\n",
        "    break\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.isinstance_typeddict_violations.is_empty());
    Ok(())
}

#[test]
fn isinstance_typeddict_in_for() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "for x in [{}]:\n",
        "    isinstance(x, Movie)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.isinstance_typeddict_violations.is_empty());
    Ok(())
}

#[test]
fn isinstance_typeddict_in_class() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "class Checker:\n",
        "    x = isinstance({}, Movie)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.isinstance_typeddict_violations.is_empty());
    Ok(())
}

#[test]
fn isinstance_typeddict_in_elif() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "x = {}\n",
        "if False:\n",
        "    pass\n",
        "elif isinstance(x, Movie):\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.isinstance_typeddict_violations.is_empty());
    Ok(())
}
