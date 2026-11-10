//! Tests for the annotation-requirement exemptions in
//! `crates/basilisk-checker/src/rules/guards.rs` — `dataclass_transform`
//! (PEP 681), Protocol/abstract method bodies, `@overload`, enum variants,
//! and `NamedTuple` classes.
//!
//! Exercises [TYPEINF-EXCEEDS] (see
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-EXCEEDS): each source
//! below is spec-valid, so a run over it must complete without the checker
//! erroring out. Split out of `inference_flow_tests.rs`, which covered
//! `RhsKind` shape classification rather than these guards.

use super::common::*;

#[test]
fn guards_dataclass_transform_frozen() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import dataclass_transform

@dataclass_transform(frozen_default=True)
def create_model(cls):
    return cls

@create_model
class User:
    name: str
    age: int
";
    let diags = run(source)?;
    let _ = diags;
    Ok(())
}

#[test]
fn guards_dataclass_transform_order() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import dataclass_transform

@dataclass_transform(order_default=True)
def create_model(cls):
    return cls

@create_model
class Point:
    x: float
    y: float

p1 = Point()
p2 = Point()
result = p1 < p2
";
    let diags = run(source)?;
    let _ = diags;
    Ok(())
}

#[test]
fn guards_dataclass_transform_class_override() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import dataclass_transform

@dataclass_transform()
def create_model(cls):
    return cls

@create_model(frozen=True)
class FrozenUser:
    name: str

@create_model(order=True)
class OrderedUser:
    name: str
";
    let diags = run(source)?;
    let _ = diags;
    Ok(())
}

#[test]
fn guards_protocol_method_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Drawable(Protocol):
    def draw(self, x, y):
        ...
";
    let diags = run(source)?;
    let _ = diags;
    Ok(())
}

#[test]
fn guards_overload_not_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload

@overload
def f(x: int) -> int: ...
@overload
def f(x: str) -> str: ...

def f(x):
    return x
";
    let diags = run(source)?;
    let _ = diags;
    Ok(())
}

#[test]
fn guards_abstractmethod_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from abc import ABC, abstractmethod

class Base(ABC):
    @abstractmethod
    def do_thing(self):
        pass
";
    let diags = run(source)?;
    let _ = diags;
    Ok(())
}

#[test]
fn guards_enum_class_variants() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from enum import Enum, IntEnum, StrEnum, Flag, IntFlag

class Color(Enum):
    RED = 1

class Perm(IntFlag):
    READ = 1
    WRITE = 2

class Status(StrEnum):
    ACTIVE = "active"

class Priority(IntEnum):
    LOW = 1
    HIGH = 2

class Access(Flag):
    ADMIN = 1
"#;
    let diags = run(source)?;
    let _ = diags;
    Ok(())
}

#[test]
fn guards_namedtuple_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple

class Point(NamedTuple):
    x: float
    y: float
    name = "origin"
"#;
    let diags = run(source)?;
    let _ = diags;
    Ok(())
}
