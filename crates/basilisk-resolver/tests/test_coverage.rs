mod common;

use common::{resolve_src};
use basilisk_resolver::RhsKind;

#[test]
fn protocol_provided_via_base_class() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Protocol\n",
        "class HasGreet(Protocol):\n",
        "    def greet(self) -> str:\n",
        "        ...\n",
        "class GreetBase:\n",
        "    def greet(self) -> str:\n",
        "        return 'hi'\n",
        "class Impl(GreetBase, HasGreet):\n",
        "    pass\n",
        "x = Impl()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.protocol_instantiation_violations.is_empty(),
        "method provided by base class should satisfy protocol"
    );
    Ok(())
}

#[test]
fn attr_access_in_if_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Foo:\n",
        "    x: int = 1\n",
        "if True:\n",
        "    Foo.x\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.module_attr_accesses.is_empty());
    Ok(())
}

#[test]
fn typevar_covariant_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import TypeVar\nT = TypeVar('T', covariant=True)\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved
        .typevar_calls
        .iter()
        .any(|t| t.name == "T" && t.is_covariant));
    Ok(())
}

#[test]
fn typevar_contravariant_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import TypeVar\nT = TypeVar('T', contravariant=True)\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved
        .typevar_calls
        .iter()
        .any(|t| t.name == "T" && t.is_contravariant));
    Ok(())
}

#[test]
fn typevar_infer_variance_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import TypeVar\nT = TypeVar('T', infer_variance=True)\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved
        .typevar_calls
        .iter()
        .any(|t| t.name == "T" && t.has_infer_variance));
    Ok(())
}

#[test]
fn generic_with_subscript_non_typevar_arg() -> Result<(), Box<dyn std::error::Error>> {
    // `list[int]` is not a simple name, so it should be reported as non-typevar
    let src =
        "from typing import Generic\nclass MyClass(Generic[list[int]]):\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "MyClass");
    assert!(cls.is_some_and(|c| !c.generic_non_typevar_args.is_empty()));
    Ok(())
}

#[test]
fn unconditional_assigns_elif_chain() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo(x: int) -> int:\n",
        "    if x > 0:\n",
        "        result = 1\n",
        "    elif x < 0:\n",
        "        result = -1\n",
        "    else:\n",
        "        result = 0\n",
        "    return result\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.functions.is_empty());
    Ok(())
}

#[test]
fn dataclass_transform_bare_name_decorator() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import dataclass_transform\n",
        "@dataclass_transform\n",
        "def my_decorator(cls: type) -> type:\n",
        "    return cls\n",
        "@my_decorator\n",
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
fn dataclass_transform_attribute_form_decorator() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "import typing\n",
        "@typing.dataclass_transform(kw_only_default=True)\n",
        "def my_deco(cls: type) -> type:\n",
        "    return cls\n",
        "@my_deco\n",
        "class Bar:\n",
        "    name: str\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Bar");
    assert!(cls.is_some_and(|c| c.is_dataclass && c.is_dataclass_kw_only));
    Ok(())
}

#[test]
fn dataclass_via_non_call_non_name_decorator_no_crash() -> Result<(), Box<dyn std::error::Error>> {
    // A decorator that is not a call or name expression should be handled gracefully
    let src = "from dataclasses import dataclass\n@dataclass\nclass Foo:\n    x: int\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.classes.is_empty());
    Ok(())
}

#[test]
fn field_attribute_form_field_call() -> Result<(), Box<dyn std::error::Error>> {
    // field() via dataclasses.field form
    let src = concat!(
        "import dataclasses\n",
        "@dataclasses.dataclass\n",
        "class Foo:\n",
        "    x: int = dataclasses.field(kw_only=True)\n",
        "    y: int = dataclasses.field(init=False, default=0)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    assert!(cls.is_some_and(|c| c.attributes.len() == 2));
    Ok(())
}

#[test]
fn initvar_attribute_form_annotation() -> Result<(), Box<dyn std::error::Error>> {
    // InitVar via dataclasses.InitVar form
    let src = concat!(
        "import dataclasses\n",
        "@dataclasses.dataclass\n",
        "class Foo:\n",
        "    x: int\n",
        "    y: dataclasses.InitVar[int] = 0\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    assert!(cls.is_some_and(|c| c.attributes.iter().any(|a| a.is_init_var)));
    Ok(())
}

#[test]
fn typevar_first_arg_non_string() -> Result<(), Box<dyn std::error::Error>> {
    // TypeVar with non-string first arg: string_name should be None
    let src = "from typing import TypeVar\nT = TypeVar(42)\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved
        .typevar_calls
        .iter()
        .any(|t| t.name == "T" && t.string_name.is_none()));
    Ok(())
}

#[test]
fn complex_number_rhs_classified() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::RhsKind;
    let src = "z = 3+4j\n".to_owned();
    let resolved = resolve_src(&src)?;
    let var = resolved.module_vars.iter().find(|v| v.name == "z");
    assert!(var.is_some_and(|v| matches!(v.rhs_kind, RhsKind::Other)));
    Ok(())
}

#[test]
fn dataclass_attribute_form_eq_false_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "import dataclasses\n@dataclasses.dataclass(eq=False)\nclass Foo:\n    x: int\n".to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    assert!(cls.is_some_and(|c| c.is_dataclass_eq_false));
    Ok(())
}

#[test]
fn call_in_module_assign_collected() -> Result<(), Box<dyn std::error::Error>> {
    // Test that simple name calls in assign stmts are collected
    let src = "x = SomeClass(1, 2)\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.calls.iter().any(|c| c.callee == "SomeClass"));
    Ok(())
}

#[test]
fn class_base_attribute_expression() -> Result<(), Box<dyn std::error::Error>> {
    let src = "import abc\nclass Foo(abc.ABC):\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    assert!(cls.is_some_and(|c| c.base_expression_names.contains(&"abc".to_owned())));
    Ok(())
}

#[test]
fn type_alias_with_tuple_rhs() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "from typing import TypeAlias, Union\nMyType: TypeAlias = Union[str, int]\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.type_alias_defs.is_empty());
    Ok(())
}

#[test]
fn type_alias_with_forward_ref() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import TypeAlias\nMyType: TypeAlias = \"Foo\" | \"Bar\"\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.type_alias_defs.is_empty());
    Ok(())
}

#[test]
fn type_alias_subscript_rhs() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import TypeAlias\nMyList: TypeAlias = list[int]\n".to_owned();
    let resolved = resolve_src(&src)?;
    let alias = resolved.type_alias_defs.iter().find(|a| a.name == "MyList");
    assert!(alias.is_some());
    Ok(())
}

#[test]
fn class_base_with_call_expression() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def make_base() -> type:\n    pass\nclass Foo(make_base()):\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    assert!(cls.is_some_and(|c| c.base_expression_names.contains(&"make_base".to_owned())));
    Ok(())
}

#[test]
fn decorator_via_attribute_name() -> Result<(), Box<dyn std::error::Error>> {
    let src = "import abc\nclass Foo:\n    @abc.abstractmethod\n    def bar(self) -> None: ...\n"
        .to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "bar");
    assert!(func.is_some_and(|f| f.decorators.iter().any(|d| d == "abstractmethod")));
    Ok(())
}

#[test]
fn unpack_tuple_unbounded_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import Unpack\ndef foo(x: tuple[int, Unpack[tuple[str, ...]]]) -> None:\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.functions.is_empty());
    Ok(())
}

#[test]
fn multiple_unbounded_tuple_components() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import TypeVarTuple, Unpack\nTs = TypeVarTuple('Ts')\nUs = TypeVarTuple('Us')\ndef foo(x: tuple[*Ts, *Us]) -> None:\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.multiple_unbounded_tuple_spans.is_empty());
    Ok(())
}

#[test]
fn typeddict_binop_read_check() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import TypedDict, ReadOnly, Unpack\nclass TD(TypedDict):\n    count: ReadOnly[int]\ndef foo(**kw: Unpack[TD]) -> int:\n    return kw[\"count\"] + 1\n".to_owned();
    let resolved = resolve_src(&src)?;
    // Reading ReadOnly fields is fine
    assert!(!resolved.functions.is_empty());
    Ok(())
}
