//! Tests for resolver: test_enum_violations.

mod common;

use common::resolve_src;

#[test]
fn enum_value_type_mismatch_detected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from enum import Enum\n",
        "class Color(Enum):\n",
        "    _value_: int\n",
        "    RED = 'red'\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.enum_value_type_violations.is_empty(),
        "str value for int _value_ must produce a violation"
    );
    Ok(())
}

#[test]
fn enum_value_type_compatible_no_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from enum import Enum\n",
        "class Color(Enum):\n",
        "    _value_: int\n",
        "    RED = 1\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.enum_value_type_violations.is_empty(),
        "int value for int _value_ must not produce a violation"
    );
    Ok(())
}

#[test]
fn enum_value_type_bool_is_int_subtype() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from enum import Enum\n",
        "class MyEnum(Enum):\n",
        "    _value_: int\n",
        "    FLAG = True\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.enum_value_type_violations.is_empty(),
        "bool is compatible with int _value_"
    );
    Ok(())
}

#[test]
fn enum_value_type_float_accepts_int() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from enum import Enum\n",
        "class MyEnum(Enum):\n",
        "    _value_: float\n",
        "    VAL = 42\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.enum_value_type_violations.is_empty(),
        "int is compatible with float _value_"
    );
    Ok(())
}

#[test]
fn enum_init_value_param_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from enum import Enum\n",
        "class Color(Enum):\n",
        "    _value_: int\n",
        "    def __init__(self, val: str) -> None:\n",
        "        self._value_ = val\n",
        "    RED = 'red'\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.enum_value_type_violations.is_empty(),
        "str param assigned to int _value_ must produce a violation"
    );
    Ok(())
}

#[test]
fn enum_no_value_annotation_no_violations() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from enum import Enum\n",
        "class Color(Enum):\n",
        "    RED = 'red'\n",
        "    BLUE = 'blue'\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.enum_value_type_violations.is_empty(),
        "no _value_ annotation means no violations"
    );
    Ok(())
}

#[test]
fn enum_non_enum_class_no_violations() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class NotAnEnum:\n",
        "    _value_: int\n",
        "    RED = 'red'\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.enum_value_type_violations.is_empty(),
        "non-enum class must not be checked for _value_ violations"
    );
    Ok(())
}

#[test]
fn enum_int_enum_also_detected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from enum import IntEnum\n",
        "class Color(IntEnum):\n",
        "    _value_: int\n",
        "    RED = 'not_int'\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.enum_value_type_violations.is_empty());
    Ok(())
}
