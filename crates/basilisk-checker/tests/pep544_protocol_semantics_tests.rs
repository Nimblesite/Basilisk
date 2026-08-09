//! PEP 544: protocol declarations, instantiation, and runtime checks.
//! Specifications: https://peps.python.org/pep-0544/ and
//! https://typing.python.org/en/latest/spec/protocol.html

mod common;

use common::{assert_rule_count, run};

#[test]
fn protocol_cannot_be_instantiated() -> Result<(), Box<dyn std::error::Error>> {
    for source in [
        r#"
from typing import Protocol as Shape

class Vessel(Shape):
    def volume(self) -> int: ...

value = Vessel()
"#,
        r#"
import typing

class Vessel(typing.Protocol):
    def volume(self) -> int: ...

value = Vessel()
"#,
    ] {
        let diagnostics = run(source)?;
        assert_rule_count(
            &diagnostics,
            "protocols_explicit",
            1,
            "PEP 544 prohibits instantiating a protocol class",
        );
    }
    Ok(())
}

#[test]
fn ordinary_class_with_the_same_members_can_be_instantiated(
) -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(r#"
class Vessel:
    def volume(self) -> int:
        return 1

value = Vessel()
"#)?;
    assert_rule_count(
        &diagnostics,
        "protocols_explicit",
        0,
        "PEP 544's instantiation restriction applies to protocols, not ordinary classes",
    );
    Ok(())
}

#[test]
fn protocol_instance_attribute_must_be_declared() -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(r#"
from typing import Protocol as Shape

class Vessel(Shape):
    name: str

    def prepare(self) -> None:
        self.name = "registered"
        self.secret = 1
"#)?;
    assert_rule_count(
        &diagnostics,
        "protocols_definition",
        1,
        "PEP 544 requires protocol instance attributes to be explicitly declared",
    );
    Ok(())
}

#[test]
fn non_runtime_protocol_rejects_isinstance() -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(r#"
from typing import Protocol as Shape

class Vessel(Shape):
    def volume(self) -> int: ...

value: object = object()
isinstance(value, Vessel)
"#)?;
    assert_rule_count(
        &diagnostics,
        "protocols_runtime_checkable",
        1,
        "PEP 544 allows isinstance only for protocols marked runtime_checkable",
    );
    Ok(())
}

#[test]
fn runtime_checkable_method_protocol_allows_isinstance() -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(r#"
from typing import Protocol as Shape, runtime_checkable as runtime_shape

@runtime_shape
class Vessel(Shape):
    def volume(self) -> int: ...

value: object = object()
isinstance(value, Vessel)
"#)?;
    assert_rule_count(
        &diagnostics,
        "protocols_runtime_checkable",
        0,
        "PEP 544 permits isinstance with a runtime_checkable protocol",
    );
    Ok(())
}

#[test]
fn data_protocol_rejects_issubclass_even_when_runtime_checkable(
) -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(r#"
from typing import Protocol as Shape, runtime_checkable as runtime_shape

@runtime_shape
class Vessel(Shape):
    name: str

issubclass(object, Vessel)
"#)?;
    assert_rule_count(
        &diagnostics,
        "protocols_runtime_checkable",
        1,
        "PEP 544 permits issubclass only for non-data runtime protocols",
    );
    Ok(())
}
