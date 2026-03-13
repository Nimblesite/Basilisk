mod common;

use common::{resolve_src};
use basilisk_resolver::RhsKind;
use basilisk_resolver::ViolationKind;

#[test]
fn typeddict_bool_compatible_with_int() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Flags(TypedDict):\n",
        "    count: int\n",
        "f: Flags = {'count': True}\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let wrong = resolved.typeddict_key_violations.iter().any(|v| {
        matches!(
            &v.kind,
            basilisk_resolver::TypedDictKeyViolationKind::WrongSubscriptValueType { key, .. }
            if key == "count"
        )
    });
    assert!(!wrong, "bool should be compatible with int");
    Ok(())
}

#[test]
fn typeddict_float_literal_compatible() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Measurement(TypedDict):\n",
        "    value: float\n",
        "def f(m: Measurement) -> None:\n",
        "    m['value'] = 3.14\n",
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
    assert!(
        !wrong,
        "float literal should be compatible with float field"
    );
    Ok(())
}

#[test]
fn typeddict_float_literal_for_int_field() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Point(TypedDict):\n",
        "    x: int\n",
        "p: Point = {'x': 0}\n",
        "p['x'] = 3.14\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let wrong = resolved.typeddict_key_violations.iter().any(|v| {
        matches!(
            &v.kind,
            basilisk_resolver::TypedDictKeyViolationKind::WrongSubscriptValueType { key, .. }
            if key == "x"
        )
    });
    assert!(wrong, "float literal should be incompatible with int field");
    Ok(())
}

#[test]
fn bounded_typevar_kwonly_param_v2() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Foo[T: str]:\n",
        "    def method(self, *, val: T) -> None:\n",
        "        val.nonexistent_attr\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_in_try_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Foo[T: str]:\n",
        "    def method(self, val: T) -> None:\n",
        "        try:\n",
        "            val.nonexistent\n",
        "        except Exception:\n",
        "            pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_in_with_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Foo[T: int]:\n",
        "    def method(self, val: T) -> None:\n",
        "        with open('x') as f:\n",
        "            val.nonexistent\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_in_for_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Foo[T: str]:\n",
        "    def method(self, val: T) -> None:\n",
        "        for _ in range(3):\n",
        "            val.fake_method\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_in_while_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Foo[T: str]:\n",
        "    def method(self, val: T) -> None:\n",
        "        while True:\n",
        "            val.nonexistent\n",
        "            break\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_in_compare_expr() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Foo[T: int]:\n",
        "    def method(self, val: T) -> None:\n",
        "        x = val.nonexistent < 5\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_in_unaryop_expr() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Foo[T: str]:\n",
        "    def method(self, val: T) -> None:\n",
        "        x = not val.nonexistent\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_in_boolop_expr() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Foo[T: str]:\n",
        "    def method(self, val: T) -> None:\n",
        "        x = val.nonexistent or True\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_in_annassign() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Foo[T: str]:\n",
        "    def method(self, val: T) -> None:\n",
        "        x: int = val.nonexistent\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_in_binop_expr() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Foo[T: int]:\n",
        "    def method(self, val: T) -> None:\n",
        "        x = val.nonexistent + 1\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_in_elif_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Foo[T: str]:\n",
        "    def method(self, val: T, flag: bool) -> None:\n",
        "        if flag:\n",
        "            pass\n",
        "        elif not flag:\n",
        "            val.nonexistent\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn bounded_typevar_call_arg() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Foo[T: str]:\n",
        "    def method(self, val: T) -> None:\n",
        "        print(val.nonexistent)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.bounded_typevar_attr_violations.is_empty());
    Ok(())
}

#[test]
fn base_subscript_with_subscript_arg() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Generic, TypeVar\n",
        "T = TypeVar('T')\n",
        "class Base(Generic[T]):\n",
        "    pass\n",
        "class Child(Base[list[int]]):\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Child");
    assert!(cls.is_some_and(|c| !c.base_subscripts.is_empty()));
    Ok(())
}

#[test]
fn base_subscript_with_literal_type_arg() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Generic, TypeVar\n",
        "T = TypeVar('T')\n",
        "class Base(Generic[T]):\n",
        "    pass\n",
        "class Child(Base[42]):\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Child");
    assert!(cls.is_some_and(|c| !c.base_subscripts.is_empty()));
    Ok(())
}

#[test]
fn protocol_class_factory_instantiation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Protocol\n",
        "class Drawable(Protocol):\n",
        "    def draw(self) -> None: ...\n",
        "def factory(cls: type[Drawable]) -> Drawable:\n",
        "    return cls()\n",
        "factory(Drawable)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.protocol_instantiation_violations.is_empty());
    Ok(())
}

#[test]
fn dataclass_transform_field_specifier_positional() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import dataclass_transform\n",
        "def myfield(name: str, *, default: object = ..., init: bool = True, kw_only: bool = False) -> object: ...\n",
        "@dataclass_transform(field_specifiers=(myfield,))\n",
        "class Base:\n",
        "    pass\n",
        "class Child(Base):\n",
        "    x: int = myfield('x', init=True)\n",
    ).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.classes.iter().any(|c| c.name == "Child"));
    Ok(())
}

#[test]
fn pep695_bound_with_parameterized_subscript() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypeVar\n",
        "T = TypeVar('T', bound=list[int])\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.typevar_calls.is_empty());
    Ok(())
}

#[test]
fn pep695_bound_tuple_outer_ref() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Outer[T]:\n",
        "    class Inner[U: tuple[T, str]]:\n",
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.pep695_bound_violations.is_empty());
    Ok(())
}

#[test]
fn classify_rhs_complex_number() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::RhsKind;
    let src = "z: complex = 3+4j\n".to_owned();
    let resolved = resolve_src(&src)?;
    let var = resolved.module_vars.iter().find(|v| v.name == "z");
    assert!(var.is_some_and(|v| matches!(v.rhs_kind, RhsKind::Other)));
    Ok(())
}

#[test]
fn classify_rhs_set_literal_form() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::RhsKind;
    let src = "s: set = {1, 2, 3}\n".to_owned();
    let resolved = resolve_src(&src)?;
    let var = resolved.module_vars.iter().find(|v| v.name == "s");
    assert!(var.is_some_and(|v| matches!(v.rhs_kind, RhsKind::Set(_))));
    Ok(())
}

#[test]
fn return_name_refs_from_attribute_expr() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(obj: object) -> str:\n    return obj.name\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(func.is_some_and(|f| f.return_name_refs.iter().any(|(n, _)| n == "obj")));
    Ok(())
}

#[test]
fn return_name_refs_from_call_expr() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: int) -> int:\n    return bar(x)\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(func.is_some_and(|f| f.return_name_refs.iter().any(|(n, _)| n == "x")));
    Ok(())
}

#[test]
fn return_name_refs_from_tuple_expr() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(a: int, b: int) -> tuple:\n    return a, b\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(func.is_some_and(|f| f.return_name_refs.iter().any(|(n, _)| n == "a")));
    Ok(())
}

#[test]
fn return_name_refs_from_binop_expr() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(a: int, b: int) -> int:\n    return a + b\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(func.is_some_and(|f| f.return_name_refs.iter().any(|(n, _)| n == "a")));
    Ok(())
}

#[test]
fn return_name_refs_from_subscript_expr() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(items: list) -> int:\n    return items[0]\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(func.is_some_and(|f| f.return_name_refs.iter().any(|(n, _)| n == "items")));
    Ok(())
}

#[test]
fn starred_typevartuple_generic_param() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import TypeVarTuple, Generic, Unpack\nTs = TypeVarTuple('Ts')\nclass MyClass(Generic[*Ts]):\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "MyClass");
    assert!(cls.is_some_and(|c| c.generic_params.iter().any(|p| p.is_typevartuple)));
    Ok(())
}

#[test]
fn non_typevar_expr_in_generic_brackets() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import Generic\nclass MyClass(Generic[int]):\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "MyClass");
    assert!(cls.is_some_and(|c| !c.generic_non_typevar_args.is_empty()));
    Ok(())
}

#[test]
fn dataclass_attribute_style_frozen() -> Result<(), Box<dyn std::error::Error>> {
    let src = "import dataclasses\n@dataclasses.dataclass(frozen=True)\nclass Foo:\n    x: int\n"
        .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    assert!(cls.is_some_and(|c| c.is_dataclass && c.is_dataclass_frozen));
    Ok(())
}

#[test]
fn dataclass_field_via_attribute_style() -> Result<(), Box<dyn std::error::Error>> {
    let src = "import dataclasses\n@dataclasses.dataclass\nclass Foo:\n    x: int = dataclasses.field(default=0)\n".to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    assert!(cls.is_some_and(|c| !c.attributes.is_empty()));
    Ok(())
}

#[test]
fn initvar_via_attribute_style() -> Result<(), Box<dyn std::error::Error>> {
    let src = "import dataclasses\n@dataclasses.dataclass\nclass Foo:\n    x: int\n    y: dataclasses.InitVar[int] = 0\n".to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    assert!(cls.is_some_and(|c| !c.attributes.is_empty()));
    Ok(())
}

#[test]
fn pep695_typevartuple_name() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo[*Ts](*args: *Ts) -> None:\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(func.is_some_and(|f| f.pep695_type_param_names.contains(&"Ts".to_owned())));
    Ok(())
}

#[test]
fn pep695_paramspec_name() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "from typing import Callable\ndef foo[**P](f: Callable[P, None]) -> None:\n    pass\n"
            .to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(func.is_some_and(|f| f.pep695_type_param_names.contains(&"P".to_owned())));
    Ok(())
}

#[test]
fn protocol_instantiation_subscript() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import Protocol, TypeVar\nT = TypeVar('T')\nclass MyProto(Protocol[T]):\n    def method(self) -> T: ...\nx = MyProto[int]()\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.protocol_instantiation_violations.is_empty());
    Ok(())
}

#[test]
fn abstract_class_instantiation_subscript() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from abc import ABC, abstractmethod\nfrom typing import Generic, TypeVar\nT = TypeVar('T')\nclass Base(ABC, Generic[T]):\n    @abstractmethod\n    def method(self) -> T: ...\nx = Base[int]()\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved
        .protocol_instantiation_violations
        .iter()
        .any(|v| v.is_abstract));
    Ok(())
}
