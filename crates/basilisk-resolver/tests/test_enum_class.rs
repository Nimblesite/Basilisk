#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Tests for resolver: `test_enum_class`.

mod common;

use common::resolve_src;

#[test]
fn enum_value_type_annotation_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from enum import Enum\n",
        "class Color(Enum):\n",
        "    _value_: int\n",
        "    RED = 'red'\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.enum_value_type_violations.is_empty());
    Ok(())
}

#[test]
fn enum_init_value_param_type() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from enum import Enum\n",
        "class Color(Enum):\n",
        "    _value_: int\n",
        "    def __init__(self, v: str) -> None:\n",
        "        self._value_ = v\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.enum_value_type_violations.is_empty());
    Ok(())
}
