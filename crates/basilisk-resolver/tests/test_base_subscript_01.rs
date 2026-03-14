#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Tests for resolver: `test_base_subscript_01`.

mod common;

use common::resolve_src;

#[test]
fn class_base_subscript_entries() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Generic, TypeVar\n",
        "T = TypeVar('T')\n",
        "class Base:\n",
        "    pass\n",
        "class Child(Base, Generic[T]):\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let child = resolved.classes.iter().find(|c| c.name == "Child");
    assert!(child.is_some());
    Ok(())
}

#[test]
fn class_base_subscripts_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Generic, TypeVar\n",
        "T = TypeVar('T')\n",
        "class Container(Generic[T]):\n",
        "    pass\n",
        "class IntContainer(Container[int]):\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "IntContainer");
    assert!(cls.is_some_and(|c| !c.base_subscripts.is_empty()));
    Ok(())
}

#[test]
fn generator_violation_invalid_return_type() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def gen() -> list:\n    yield 1\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.generator_violations.is_empty());
    Ok(())
}

#[test]
fn generator_violation_async_invalid_return() -> Result<(), Box<dyn std::error::Error>> {
    let src = "async def gen() -> str:\n    yield 'hello'\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.generator_violations.is_empty());
    Ok(())
}

#[test]
fn generator_no_violation_valid_return() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Generator\n",
        "def gen() -> Generator[int, None, None]:\n",
        "    yield 1\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.generator_violations.is_empty());
    Ok(())
}

#[test]
fn generator_user_defined_return_not_flagged() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class MyIterator:\n",
        "    def __next__(self) -> int: ...\n",
        "def gen() -> MyIterator:\n",
        "    yield 1\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.generator_violations.is_empty());
    Ok(())
}

#[test]
fn async_generator_valid_return() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import AsyncGenerator\n",
        "async def gen() -> AsyncGenerator[int, None]:\n",
        "    yield 1\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.generator_violations.is_empty());
    Ok(())
}

#[test]
fn multiple_unbounded_with_unpack_form() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Unpack\n",
        "def f(x: tuple[Unpack[tuple[str, ...]], Unpack[tuple[int, ...]]]) -> None:\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.multiple_unbounded_tuple_spans.is_empty());
    Ok(())
}

#[test]
fn dataclass_field_kw_only_override_v2() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass, field\n",
        "@dataclass\n",
        "class Conf:\n",
        "    name: str\n",
        "    debug: bool = field(kw_only=True)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.classes.iter().any(|c| c.name == "Conf"));
    Ok(())
}

#[test]
fn dataclass_field_init_false_v2() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass, field\n",
        "@dataclass\n",
        "class Conf:\n",
        "    name: str\n",
        "    cached: int = field(init=False, default=0)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.classes.iter().any(|c| c.name == "Conf"));
    Ok(())
}

#[test]
fn dataclass_transform_with_field_specifiers() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import dataclass_transform\n",
        "def myfield(*, default: object = ..., init: bool = True, kw_only: bool = False) -> object: ...\n",
        "@dataclass_transform(kw_only_default=True, field_specifiers=(myfield,))\n",
        "class Base:\n",
        "    pass\n",
        "class Child(Base):\n",
        "    name: str\n",
    ).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.classes.iter().any(|c| c.name == "Child"));
    Ok(())
}

#[test]
fn dataclass_transform_bare_decorator() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import dataclass_transform\n",
        "@dataclass_transform\n",
        "class Meta(type):\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.classes.is_empty());
    Ok(())
}

#[test]
fn dataclass_transform_attribute_form() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "import typing\n",
        "@typing.dataclass_transform()\n",
        "class Meta(type):\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.classes.is_empty());
    Ok(())
}

#[test]
fn readonly_kwargs_subscript_assign() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict, ReadOnly, Unpack\n",
        "class Config(TypedDict):\n",
        "    name: ReadOnly[str]\n",
        "    value: int\n",
        "def f(**kwargs: Unpack[Config]) -> None:\n",
        "    kwargs['name'] = 'new'\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.readonly_violations.is_empty());
    Ok(())
}

#[test]
fn final_walrus_operator_violation_v2() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Final\n",
        "X: Final[int] = 10\n",
        "if (X := 20):\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.final_violations.is_empty());
    Ok(())
}

#[test]
fn abstract_class_instantiation_detected_v2() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from abc import ABC, abstractmethod\n",
        "class Animal(ABC):\n",
        "    @abstractmethod\n",
        "    def speak(self) -> str: ...\n",
        "a = Animal()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let abstract_violations: Vec<_> = resolved
        .protocol_instantiation_violations
        .iter()
        .filter(|v| v.is_abstract)
        .collect();
    assert!(!abstract_violations.is_empty());
    Ok(())
}

#[test]
fn dataclass_attribute_form_eq_false() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "import dataclasses\n",
        "@dataclasses.dataclass(eq=False)\n",
        "class Point:\n",
        "    x: int\n",
        "    y: int\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved
        .classes
        .iter()
        .any(|c| c.name == "Point" && c.is_dataclass_eq_false));
    Ok(())
}

#[test]
fn field_attribute_form_kw_only() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "import dataclasses\n",
        "@dataclasses.dataclass\n",
        "class Conf:\n",
        "    name: str\n",
        "    debug: bool = dataclasses.field(kw_only=True)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.classes.iter().any(|c| c.name == "Conf"));
    Ok(())
}

#[test]
fn field_attribute_form_init_false() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "import dataclasses\n",
        "@dataclasses.dataclass\n",
        "class Conf:\n",
        "    name: str\n",
        "    cached: int = dataclasses.field(init=False, default=0)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.classes.iter().any(|c| c.name == "Conf"));
    Ok(())
}

#[test]
fn initvar_attribute_form() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "import dataclasses\n",
        "@dataclasses.dataclass\n",
        "class Foo:\n",
        "    x: int\n",
        "    y: dataclasses.InitVar[int] = 0\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.classes.iter().any(|c| c.name == "Foo"));
    Ok(())
}

#[test]
fn unconditional_self_assign_if_else() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Foo:\n",
        "    def __init__(self, flag: bool) -> None:\n",
        "        if flag:\n",
        "            self.x = 1\n",
        "            self.y = 2\n",
        "        else:\n",
        "            self.x = 3\n",
        "        self.z = 4\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.classes.iter().any(|c| c.name == "Foo"));
    Ok(())
}

#[test]
fn typeddict_ann_assign_missing_keys() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "    year: int\n",
        "m: Movie = {'name': 'test'}\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let missing = resolved.typeddict_key_violations.iter().any(|v| {
        matches!(
            &v.kind,
            basilisk_resolver::TypedDictKeyViolationKind::InvalidDictLiteral { missing_keys, .. }
            if !missing_keys.is_empty()
        )
    });
    assert!(missing);
    Ok(())
}

#[test]
fn typeddict_dict_literal_wrong_value_type() -> Result<(), Box<dyn std::error::Error>> {
    // Dict literal with string value for int field - should be flagged at ann_assign level
    let src = concat!(
        "from typing import TypedDict\n",
        "class Point(TypedDict):\n",
        "    x: int\n",
        "    y: int\n",
        "p: Point = {'x': 1, 'y': 'not_int'}\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    // The td_check_ann_assign checks dict literal values at module level
    let wrong = resolved.typeddict_key_violations.iter().any(|v| {
        matches!(
            &v.kind,
            basilisk_resolver::TypedDictKeyViolationKind::WrongSubscriptValueType { key, .. }
            if key == "y"
        )
    });
    assert!(
        wrong,
        "should flag string literal for int field in dict literal"
    );
    Ok(())
}

#[test]
fn pep695_bound_refs_outer_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Outer[T]:\n",
        "    class Inner[U: T]:\n",
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.pep695_bound_violations.is_empty());
    Ok(())
}

#[test]
fn pep695_bound_binop_outer_ref() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Outer[T]:\n",
        "    class Inner[U: str | T]:\n",
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.pep695_bound_violations.is_empty());
    Ok(())
}

#[test]
fn pep695_bound_subscript_outer_ref() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Outer[T]:\n",
        "    class Inner[U: dict[str, T]]:\n",
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.pep695_bound_violations.is_empty());
    Ok(())
}

#[test]
fn typeddict_subscript_assign_wrong_int_for_str() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "m: Movie = {'name': 'test'}\n",
        "m['name'] = 42\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let wrong = resolved.typeddict_key_violations.iter().any(|v| {
        matches!(
            &v.kind,
            basilisk_resolver::TypedDictKeyViolationKind::WrongSubscriptValueType { key, .. }
            if key == "name"
        )
    });
    assert!(wrong);
    Ok(())
}

#[test]
fn typeddict_none_literal_value() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Config(TypedDict):\n",
        "    value: int\n",
        "c: Config = {'value': 0}\n",
        "c['value'] = None\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let wrong = resolved.typeddict_key_violations.iter().any(|v| {
        matches!(
            &v.kind,
            basilisk_resolver::TypedDictKeyViolationKind::WrongSubscriptValueType { key, .. }
            if key == "value"
        )
    });
    assert!(wrong);
    Ok(())
}

#[test]
fn typeddict_bool_literal_for_str_field() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Config(TypedDict):\n",
        "    name: str\n",
        "c: Config = {'name': 'x'}\n",
        "c['name'] = True\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let wrong = resolved.typeddict_key_violations.iter().any(|v| {
        matches!(
            &v.kind,
            basilisk_resolver::TypedDictKeyViolationKind::WrongSubscriptValueType { key, .. }
            if key == "name"
        )
    });
    assert!(wrong);
    Ok(())
}
