#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Tests for resolver: `test_readonly`.

mod common;

use common::resolve_src;

#[test]
fn readonly_typeddict_subscript_assign_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "from typing import ReadOnly\n",
        "class Config(TypedDict):\n",
        "    name: ReadOnly[str]\n",
        "config: Config = {'name': 'test'}\n",
        "config['name'] = 'new'\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.readonly_violations.is_empty(),
        "subscript assign to ReadOnly field must produce a violation"
    );
    Ok(())
}

#[test]
fn readonly_typeddict_update_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "from typing import ReadOnly\n",
        "class Config(TypedDict):\n",
        "    name: ReadOnly[str]\n",
        "config: Config = {'name': 'test'}\n",
        "config.update({'name': 'new'})\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.readonly_violations.is_empty(),
        "calling update on ReadOnly TypedDict must produce a violation"
    );
    Ok(())
}

#[test]
fn readonly_kwargs_violation_detected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict, Unpack, ReadOnly\n",
        "class Config(TypedDict):\n",
        "    name: ReadOnly[str]\n",
        "def process(**kwargs: Unpack[Config]) -> None:\n",
        "    kwargs['name'] = 'new'\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.readonly_violations.is_empty(),
        "subscript assign to ReadOnly kwarg field must produce a violation"
    );
    Ok(())
}

#[test]
fn class_attr_readonly_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict, ReadOnly\n",
        "class Foo(TypedDict):\n",
        "    x: ReadOnly[int]\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    let attr = cls.and_then(|c| c.attributes.iter().find(|a| a.name == "x"));
    assert!(attr.is_some_and(|a| a.is_readonly));
    Ok(())
}

#[test]
fn readonly_kwargs_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict, ReadOnly, Unpack\n",
        "class Config(TypedDict):\n",
        "    name: ReadOnly[str]\n",
        "def update(**kwargs: Unpack[Config]) -> None:\n",
        "    kwargs['name'] = 'new'\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.readonly_violations.is_empty(),
        "assigning to ReadOnly key via kwargs should be a violation"
    );
    Ok(())
}
