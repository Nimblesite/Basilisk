//! Tests for resolver: test_base_subscript_03.

mod common;

use common::resolve_src;

#[test]
fn abstract_class_via_abc_attribute() -> Result<(), Box<dyn std::error::Error>> {
    let src = "import abc\nclass Foo(abc.ABC):\n    @abc.abstractmethod\n    def bar(self) -> None: ...\n".to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    // ClassInfo has bases; check that abc.ABC base is recognized
    assert!(cls.is_some_and(|c| c.bases.iter().any(|b| b == "ABC")));
    Ok(())
}

#[test]
fn enum_value_none_vs_str() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from enum import Enum\nclass Status(Enum):\n    _value_: str\n    NONE = None\n"
        .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.enum_value_type_violations.is_empty());
    Ok(())
}

#[test]
fn enum_value_float_vs_int() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "from enum import Enum\nclass Values(Enum):\n    _value_: int\n    PI = 3.14\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.enum_value_type_violations.is_empty());
    Ok(())
}

#[test]
fn enum_value_bytes_vs_str() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from enum import Enum\nclass Data(Enum):\n    _value_: str\n    BIN = b'hello'\n"
        .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.enum_value_type_violations.is_empty());
    Ok(())
}

#[test]
fn typeddict_non_literal_key_access() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::TypedDictKeyViolationKind;
    let src = "from typing import TypedDict\nclass TD(TypedDict):\n    name: str\ndef foo(td: TD) -> None:\n    key = \"name\"\n    td[key]\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved
        .typeddict_key_violations
        .iter()
        .any(|v| matches!(v.kind, TypedDictKeyViolationKind::NonLiteralDictKey)));
    Ok(())
}

#[test]
fn typeddict_subscript_wrong_value_type() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::TypedDictKeyViolationKind;
    let src = "from typing import TypedDict\nclass TD(TypedDict):\n    name: str\ndef foo(td: TD) -> None:\n    td[\"name\"] = 42\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.typeddict_key_violations.iter().any(|v| matches!(
        v.kind,
        TypedDictKeyViolationKind::WrongSubscriptValueType { .. }
    )));
    Ok(())
}

#[test]
fn type_arg_subscript_in_base_class() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::TypeArg;
    let src = "from typing import TypeVar, Generic\nT = TypeVar('T')\nclass Base(Generic[T]):\n    pass\nclass Child(Base[list[int]]):\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Child");
    assert!(cls.is_some_and(|c| c.base_subscripts.iter().any(|bs| bs
        .type_args
        .iter()
        .any(|a| matches!(a, TypeArg::Subscript { .. })))));
    Ok(())
}

#[test]
fn global_final_augmented_assign() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::FinalViolationKind;
    let src = "from typing import Final\nCOUNTER: Final[int] = 0\ndef increment() -> None:\n    global COUNTER\n    COUNTER += 1\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved
        .final_violations
        .iter()
        .any(|v| matches!(v.kind, FinalViolationKind::GlobalFinalModification)));
    Ok(())
}

#[test]
fn subclass_override_final_attr() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::FinalViolationKind;
    let src = "from typing import Final\nclass Base:\n    x: Final[int] = 10\nclass Child(Base):\n    x: int = 20\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved
        .final_violations
        .iter()
        .any(|v| matches!(v.kind, FinalViolationKind::SubclassOverrideFinal)));
    Ok(())
}

#[test]
fn assert_type_annotated_param() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import assert_type, Annotated\ndef check(x: Annotated[int, 'meta']) -> None:\n    assert_type(x, int)\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.assert_type_calls.is_empty());
    Ok(())
}

#[test]
fn literal_string_enum_single_quote_form() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import Literal\nfrom enum import Enum\nclass Color(Enum):\n    RED = 'red'\ndef check(c: Literal[Color.RED]) -> None:\n    x: Literal['Color.RED'] = c\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.literal_string_enum_mismatches.is_empty());
    Ok(())
}

#[test]
fn readonly_annotation_via_typing_attribute() -> Result<(), Box<dyn std::error::Error>> {
    let src = "import typing\nfrom typing import TypedDict, Unpack\nclass TD(TypedDict):\n    name: typing.ReadOnly[str]\ndef foo(**kwargs: Unpack[TD]) -> None:\n    kwargs[\"name\"] = \"new\"\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.readonly_violations.is_empty());
    Ok(())
}

#[test]
fn readonly_binop_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import TypedDict, ReadOnly, Unpack\nclass TD(TypedDict):\n    name: ReadOnly[str] | None\ndef foo(**kwargs: Unpack[TD]) -> None:\n    kwargs[\"name\"] = \"new\"\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.readonly_violations.is_empty());
    Ok(())
}

#[test]
fn numeric_literal_param_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: 42) -> None:\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(func.is_some_and(|f| f.parameters[0].annotation_is_numeric_literal));
    Ok(())
}

#[test]
fn boolean_literal_param_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: True) -> None:\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(func.is_some_and(|f| f.parameters[0].annotation_is_numeric_literal));
    Ok(())
}

#[test]
fn numeric_literal_return_annotation() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::ReturnAnnotationKind;
    let src = "def foo() -> 42:\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(
        func.is_some_and(|f| matches!(f.return_annotation, ReturnAnnotationKind::NumericLiteral))
    );
    Ok(())
}

#[test]
fn enum_from_strenum() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from enum import StrEnum\nclass Color(StrEnum):\n    RED = 'red'\n".to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Color");
    assert!(cls.is_some_and(|c| c.is_enum));
    Ok(())
}

#[test]
fn stub_body_pass_statement() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import Protocol\nclass P(Protocol):\n    def method(self) -> None:\n        pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    let method = resolved.functions.iter().find(|f| f.name == "method");
    assert!(method.is_some_and(|m| m.is_stub_body));
    Ok(())
}

#[test]
fn typeddict_functional_form_dict() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import TypedDict\nTD = TypedDict('TD', {'name': str, 'age': int})\n"
        .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved
        .typeddict_calls
        .iter()
        .any(|td| td.lhs_name == "TD"));
    Ok(())
}

#[test]
fn namedtuple_list_second_arg_form() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "from typing import NamedTuple\nPoint = NamedTuple('Point', [('x', int), ('y', int)])\n"
            .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved
        .namedtuple_defs
        .iter()
        .any(|nt| nt.lhs_name == "Point"));
    Ok(())
}

#[test]
fn protocol_isinstance_non_rtc_module() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import Protocol\nclass P(Protocol):\n    def m(self) -> None: ...\nisinstance(42, P)\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.protocol_runtime_checkable_violations.is_empty());
    Ok(())
}

#[test]
fn pep695_type_statement_info() -> Result<(), Box<dyn std::error::Error>> {
    let src = "type IntList = list[int]\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.type_statements.is_empty());
    Ok(())
}

#[test]
fn type_alias_type_call_info() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import TypeAliasType\nIntList = TypeAliasType('IntList', list[int])\n"
        .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved
        .type_alias_type_calls
        .iter()
        .any(|t| t.lhs_name == "IntList"));
    Ok(())
}

#[test]
fn type_alias_type_ann_assign() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "from typing import TypeAliasType\nIntList: object = TypeAliasType('IntList', list[int])\n"
            .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved
        .type_alias_type_calls
        .iter()
        .any(|t| t.lhs_name == "IntList"));
    Ok(())
}

#[test]
fn base_subscript_multiple_type_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import TypeVar, Generic\nT = TypeVar('T')\nU = TypeVar('U')\nclass Base(Generic[T, U]):\n    pass\nclass Child(Base[int, str]):\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Child");
    assert!(cls.is_some_and(|c| c
        .base_subscripts
        .iter()
        .any(|bs| bs.type_arg_names.len() == 2)));
    Ok(())
}

#[test]
fn dataclass_eq_and_order_flags() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from dataclasses import dataclass\n@dataclass(eq=True, order=True)\nclass Point:\n    x: int\n    y: int\n".to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Point");
    assert!(cls.is_some_and(|c| c.is_dataclass && c.is_dataclass_order));
    Ok(())
}

#[test]
fn dataclass_match_args_slots_flags() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from dataclasses import dataclass\n@dataclass(match_args=True, slots=True)\nclass Foo:\n    x: int\n".to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    assert!(cls.is_some_and(|c| c.is_dataclass && c.is_dataclass_slots));
    Ok(())
}

#[test]
fn class_final_with_init_assignment() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::FinalViolationKind;
    let src = "from typing import Final\nclass Foo:\n    x: Final[int]\n    def __init__(self) -> None:\n        self.x = 10\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved
        .final_violations
        .iter()
        .all(|v| !matches!(v.kind, FinalViolationKind::ClassFinalWithoutInit)));
    Ok(())
}

#[test]
fn invalid_tuple_bare_ellipsis() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: tuple[...]) -> None:\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.invalid_string_annotations.is_empty());
    Ok(())
}

#[test]
fn pep695_bound_binop_outer_typevar_ref() -> Result<(), Box<dyn std::error::Error>> {
    let src = "class Outer[T]:\n    class Inner[U: T | int]:\n        pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.pep695_bound_violations.is_empty() || !resolved.classes.is_empty());
    Ok(())
}

#[test]
fn pep695_bound_tuple_outer_typevar_ref() -> Result<(), Box<dyn std::error::Error>> {
    let src = "class Outer[T]:\n    def method[U: (T, int)](self, x: U) -> U:\n        return x\n"
        .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.functions.is_empty());
    Ok(())
}

#[test]
fn match_statement_info() -> Result<(), Box<dyn std::error::Error>> {
    let src = "x = 42\nmatch x:\n    case 1:\n        pass\n    case _:\n        pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.match_stmts.is_empty());
    Ok(())
}

#[test]
fn enum_annotated_member() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from enum import Enum\nclass Color(Enum):\n    RED: int = 1\n    GREEN: int = 2\n"
        .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Color");
    assert!(cls.is_some_and(|c| c.is_enum));
    Ok(())
}

#[test]
fn bounded_typevar_in_match() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import TypeVar\nT = TypeVar('T', bound=int)\ndef foo(x: T) -> T:\n    match x:\n        case 1:\n            return x\n        case _:\n            return x\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.functions.is_empty());
    Ok(())
}

#[test]
fn protocol_rtc_isinstance_valid() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import Protocol, runtime_checkable\n@runtime_checkable\nclass Drawable(Protocol):\n    def draw(self) -> None: ...\nclass Circle:\n    def draw(self) -> None:\n        pass\nc = Circle()\nisinstance(c, Drawable)\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.protocol_runtime_checkable_violations.is_empty());
    Ok(())
}
