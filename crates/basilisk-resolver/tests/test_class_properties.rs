mod common;

use common::resolve_src;

#[test]
fn class_metaclass_keyword_detected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Meta(type): ...\n",
        "class Foo(metaclass=Meta):\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    assert!(cls.and_then(|c| c.metaclass_name.as_ref()).is_some());
    Ok(())
}

#[test]
fn class_is_enum_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from enum import Enum\n",
        "class Color(Enum):\n",
        "    RED = 1\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Color");
    assert!(cls.is_some_and(|c| c.is_enum));
    Ok(())
}

#[test]
fn class_has_subscript_base_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Generic, TypeVar\n",
        "T = TypeVar('T')\n",
        "class Container(Generic[T]):\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Container");
    assert!(cls.is_some_and(|c| c.has_subscript_base));
    Ok(())
}

#[test]
fn nested_class_method_in_functions() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Outer:\n",
        "    class Inner:\n",
        "        def inner_method(self) -> None:\n",
        "            pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.functions.iter().any(|f| f.name == "inner_method"));
    Ok(())
}

#[test]
fn class_attr_nonmember_call_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from enum import nonmember, Enum\n",
        "class Color(Enum):\n",
        "    RED = 1\n",
        "    helper = nonmember(lambda: None)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Color");
    let attr = cls.and_then(|c| c.attributes.iter().find(|a| a.name == "helper"));
    assert!(attr.is_some_and(|a| a.rhs_is_nonmember_call));
    Ok(())
}

#[test]
fn class_attr_init_var_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass, InitVar\n",
        "@dataclass\n",
        "class Foo:\n",
        "    x: int\n",
        "    init_only: InitVar[int]\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    let attr = cls.and_then(|c| c.attributes.iter().find(|a| a.name == "init_only"));
    assert!(attr.is_some_and(|a| a.is_init_var));
    Ok(())
}

#[test]
fn class_attr_field_init_false_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass, field\n",
        "@dataclass\n",
        "class Foo:\n",
        "    x: int = field(init=False, default=0)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    let attr = cls.and_then(|c| c.attributes.iter().find(|a| a.name == "x"));
    assert!(attr.is_some_and(|a| a.is_init_false));
    Ok(())
}

#[test]
fn class_attr_field_kw_only_true_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass, field\n",
        "@dataclass\n",
        "class Foo:\n",
        "    x: int = field(kw_only=True)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    let attr = cls.and_then(|c| c.attributes.iter().find(|a| a.name == "x"));
    assert!(attr.is_some_and(|a| a.is_kw_only));
    Ok(())
}

#[test]
fn kw_only_sentinel_makes_subsequent_attrs_kw_only() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass, KW_ONLY\n",
        "@dataclass\n",
        "class Foo:\n",
        "    x: int\n",
        "    _: KW_ONLY\n",
        "    y: int\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    let y_attr = cls.and_then(|c| c.attributes.iter().find(|a| a.name == "y"));
    assert!(y_attr.is_some_and(|a| a.is_kw_only));
    let x_attr = cls.and_then(|c| c.attributes.iter().find(|a| a.name == "x"));
    assert!(!x_attr.is_none_or(|a| a.is_kw_only));
    Ok(())
}

#[test]
fn class_attr_lambda_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("class Foo:\n", "    func = lambda: None\n",).to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    let attr = cls.and_then(|c| c.attributes.iter().find(|a| a.name == "func"));
    assert!(attr.is_some_and(|a| a.rhs_is_lambda));
    Ok(())
}

#[test]
fn class_attr_descriptor_call_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("class Foo:\n", "    bar = staticmethod(lambda: None)\n",).to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    let attr = cls.and_then(|c| c.attributes.iter().find(|a| a.name == "bar"));
    assert!(attr.is_some_and(|a| a.rhs_is_descriptor_call));
    Ok(())
}

#[test]
fn class_pep695_type_param_names() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("class Foo[T]:\n", "    pass\n",).to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    assert!(cls.is_some_and(|c| c.has_pep695_type_params));
    assert!(cls.is_some_and(|c| c.pep695_type_param_names.contains(&"T".to_string())));
    Ok(())
}

#[test]
fn class_keywords_includes_total() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict, total=False):\n",
        "    name: str\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Movie");
    assert!(cls.is_some_and(|c| c.class_keywords.contains(&"total".to_string())));
    Ok(())
}

#[test]
fn class_defined_inside_while_block() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "while False:\n",
        "    class WhileClass:\n",
        "        def m(self) -> None: ...\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.classes.iter().any(|c| c.name == "WhileClass"));
    Ok(())
}

#[test]
fn class_defined_inside_for_block() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "for _ in range(1):\n",
        "    class ForClass:\n",
        "        def m(self) -> None: ...\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.classes.iter().any(|c| c.name == "ForClass"));
    Ok(())
}

#[test]
fn class_defined_inside_with_block() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "with open('f') as fh:\n",
        "    class WithClass:\n",
        "        def m(self) -> None: ...\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.classes.iter().any(|c| c.name == "WithClass"));
    Ok(())
}
