mod common;

use common::{resolve_src};

#[test]
fn dataclass_transform_applied() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import dataclass_transform\n",
        "@dataclass_transform()\n",
        "def model(cls: type) -> type:\n",
        "    return cls\n",
        "@model\n",
        "class User:\n",
        "    name: str\n",
        "    age: int\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let user = resolved.classes.iter().find(|c| c.name == "User");
    assert!(user.is_some());
    Ok(())
}

#[test]
fn dataclass_kw_only_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass, KW_ONLY\n",
        "@dataclass\n",
        "class Config:\n",
        "    x: int\n",
        "    _: KW_ONLY\n",
        "    y: str = 'default'\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Config");
    assert!(cls.is_some());
    Ok(())
}

#[test]
fn dataclass_init_var_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass, InitVar\n",
        "@dataclass\n",
        "class Config:\n",
        "    x: int\n",
        "    database: InitVar[str] = 'default'\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Config");
    assert!(cls.is_some());
    Ok(())
}

#[test]
fn dataclass_field_init_false() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass, field\n",
        "@dataclass\n",
        "class Config:\n",
        "    x: int\n",
        "    y: str = field(init=False, default='hi')\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Config");
    assert!(cls.is_some());
    Ok(())
}

#[test]
fn dataclass_field_kw_only_override() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass, field\n",
        "@dataclass\n",
        "class Config:\n",
        "    x: int = field(kw_only=True)\n",
        "    y: str = field(kw_only=False)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Config");
    assert!(cls.is_some());
    Ok(())
}

#[test]
fn dataclass_eq_false_hashable() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass\n",
        "@dataclass(eq=False)\n",
        "class MyClass:\n",
        "    x: int\n",
        "MyClass(1).__hash__()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.unhashable_hash_call_violations.is_empty(),
        "dataclass with eq=False should be hashable"
    );
    Ok(())
}

#[test]
fn dataclass_frozen_hashable() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass\n",
        "@dataclass(frozen=True)\n",
        "class MyClass:\n",
        "    x: int\n",
        "MyClass(1).__hash__()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.unhashable_hash_call_violations.is_empty(),
        "frozen dataclass should be hashable"
    );
    Ok(())
}

#[test]
fn dataclass_transform_factory_marks_class() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import dataclass_transform\n",
        "def myfield(*, init: bool = True, kw_only: bool = False) -> None: ...\n",
        "@dataclass_transform(field_specifiers=(myfield,))\n",
        "def my_dataclass(cls: type) -> type: ...\n",
        "@my_dataclass\n",
        "class Foo:\n",
        "    x: int\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    assert!(cls.is_some_and(|c| c.is_dataclass));
    Ok(())
}

#[test]
fn dataclass_transform_kw_only_default_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import dataclass_transform\n",
        "@dataclass_transform(kw_only_default=True)\n",
        "def my_dc(cls: type) -> type: ...\n",
        "@my_dc\n",
        "class Foo:\n",
        "    x: int\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    assert!(cls.is_some_and(|c| c.is_dataclass));
    assert!(cls.is_some_and(|c| c.is_dataclass_kw_only));
    Ok(())
}

#[test]
fn dataclass_transform_field_init_false() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import dataclass_transform\n",
        "def myfield(*, init: bool = True) -> None: ...\n",
        "@dataclass_transform(field_specifiers=(myfield,))\n",
        "def my_dc(cls: type) -> type: ...\n",
        "@my_dc\n",
        "class Foo:\n",
        "    x: int = myfield(init=False)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    let attr = cls.and_then(|c| c.attributes.iter().find(|a| a.name == "x"));
    assert!(attr.is_some_and(|a| a.is_init_false));
    Ok(())
}

#[test]
fn dataclass_order_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass\n",
        "@dataclass(order=True)\n",
        "class Foo:\n",
        "    x: int\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    assert!(cls.is_some_and(|c| c.is_dataclass_order));
    Ok(())
}

#[test]
fn dataclass_unsafe_hash_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass\n",
        "@dataclass(unsafe_hash=True)\n",
        "class Foo:\n",
        "    x: int\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    assert!(cls.is_some_and(|c| c.is_dataclass_unsafe_hash));
    Ok(())
}

#[test]
fn dataclass_init_false_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass\n",
        "@dataclass(init=False)\n",
        "class Foo:\n",
        "    x: int = 0\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    assert!(cls.is_some_and(|c| c.is_dataclass_init_false));
    Ok(())
}

#[test]
fn dataclass_match_args_false_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass\n",
        "@dataclass(match_args=False)\n",
        "class Foo:\n",
        "    x: int\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    assert!(cls.is_some_and(|c| c.is_dataclass_match_args_false));
    Ok(())
}

#[test]
fn dataclass_slots_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass\n",
        "@dataclass(slots=True)\n",
        "class Foo:\n",
        "    x: int\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    assert!(cls.is_some_and(|c| c.is_dataclass_slots));
    Ok(())
}
