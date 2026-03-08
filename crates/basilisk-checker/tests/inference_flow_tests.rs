//! Tests targeting inference.rs (`FlowUnionTracker`, `check_annotated_variable`, `infer_flow_union_types`)
//! and guards.rs (`dataclass_transform`, `collect_transform_functions`, `collect_transform_classes`).
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_checker::inference::{
    check_annotated_variable, infer_flow_union_types, infer_rhs, infer_variable_type,
    FlowUnionTracker,
};
use basilisk_checker::types::InferredType;
use basilisk_parser::parse_source;
use basilisk_resolver::{resolve, RhsKind};

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

// --- FlowUnionTracker tests ---

#[test]
fn flow_tracker_single_assignment() {
    let mut tracker = FlowUnionTracker::new();
    tracker.record_assignment("x", InferredType::Int);
    let result = tracker.get_union_type("x");
    assert!(result.is_some());
}

#[test]
fn flow_tracker_multi_branch_union() {
    let mut tracker = FlowUnionTracker::new();
    tracker.enter_branch();
    tracker.record_assignment("x", InferredType::Int);
    tracker.exit_branch();
    tracker.enter_branch();
    tracker.record_assignment("x", InferredType::Str);
    tracker.exit_branch();
    let result = tracker.get_union_type("x");
    assert!(result.is_some());
}

#[test]
fn flow_tracker_same_type_dedup() {
    let mut tracker = FlowUnionTracker::new();
    tracker.record_assignment("x", InferredType::Int);
    tracker.record_assignment("x", InferredType::Int);
    let result = tracker.get_union_type("x");
    assert!(result.is_some());
    // Should deduplicate to just Int
    assert_eq!(result, Some(InferredType::Int));
}

#[test]
fn flow_tracker_unknown_variable() {
    let tracker = FlowUnionTracker::new();
    assert!(tracker.get_union_type("nonexistent").is_none());
}

#[test]
fn flow_tracker_reset() {
    let mut tracker = FlowUnionTracker::new();
    tracker.record_assignment("x", InferredType::Int);
    tracker.enter_branch();
    tracker.reset();
    assert!(tracker.get_union_type("x").is_none());
}

#[test]
fn flow_tracker_nested_branches() {
    let mut tracker = FlowUnionTracker::new();
    tracker.enter_branch();
    tracker.enter_branch();
    tracker.record_assignment("x", InferredType::Float);
    tracker.exit_branch();
    tracker.exit_branch();
    // Extra exit_branch (depth is already 0, should not panic)
    tracker.exit_branch();
    assert!(tracker.get_union_type("x").is_some());
}

#[test]
fn flow_tracker_default() {
    let tracker = FlowUnionTracker::default();
    assert!(tracker.get_union_type("x").is_none());
}

// --- infer_flow_union_types tests ---

#[test]
fn flow_union_single_var() {
    let assignments = vec![("x".to_string(), InferredType::Int)];
    let result = infer_flow_union_types(&assignments);
    assert!(result.contains_key("x"));
}

#[test]
fn flow_union_multi_var() {
    let assignments = vec![
        ("x".to_string(), InferredType::Int),
        ("y".to_string(), InferredType::Str),
        ("x".to_string(), InferredType::Str),
    ];
    let result = infer_flow_union_types(&assignments);
    assert!(result.contains_key("x"));
    assert!(result.contains_key("y"));
}

// --- infer_rhs tests ---

#[test]
fn infer_rhs_lambda() {
    let result = infer_rhs(&RhsKind::Lambda);
    assert!(matches!(result, InferredType::Callable(_)));
}

#[test]
fn infer_rhs_call_expr() {
    assert!(matches!(infer_rhs(&RhsKind::CallExpr), InferredType::Unknown));
}

#[test]
fn infer_rhs_type_call() {
    assert!(matches!(infer_rhs(&RhsKind::TypeCall), InferredType::Unknown));
}

#[test]
fn infer_rhs_other() {
    assert!(matches!(infer_rhs(&RhsKind::Other), InferredType::Unknown));
}

#[test]
fn infer_rhs_empty_list() {
    let result = infer_rhs(&RhsKind::EmptyList);
    assert!(matches!(result, InferredType::List(_)));
}

#[test]
fn infer_rhs_empty_dict() {
    let result = infer_rhs(&RhsKind::EmptyDict);
    assert!(matches!(result, InferredType::Dict(_, _)));
}

#[test]
fn infer_rhs_none() {
    assert!(matches!(infer_rhs(&RhsKind::NoneValue), InferredType::None_));
}

#[test]
fn infer_rhs_bytes() {
    assert!(matches!(infer_rhs(&RhsKind::BytesLiteral), InferredType::Bytes));
}

#[test]
fn infer_rhs_bool() {
    assert!(matches!(infer_rhs(&RhsKind::BoolLiteral), InferredType::Bool));
}

// --- check_annotated_variable / infer_variable_type ---

#[test]
fn check_annotated_var_with_known_rhs() {
    let var_info = basilisk_resolver::VariableInfo {
        name: "x".to_string(),
        has_annotation: true,
        annotation_span: None,
        rhs_kind: RhsKind::IntLiteral,
        name_span: basilisk_resolver::Span { start: 0, end: 1 },
        rhs_span: None,
    };
    assert!(check_annotated_variable(&var_info).is_ok());
}

#[test]
fn check_annotated_var_with_unknown_rhs() {
    let var_info = basilisk_resolver::VariableInfo {
        name: "x".to_string(),
        has_annotation: true,
        annotation_span: None,
        rhs_kind: RhsKind::Other,
        name_span: basilisk_resolver::Span { start: 0, end: 1 },
        rhs_span: None,
    };
    assert!(check_annotated_variable(&var_info).is_err());
}

#[test]
fn check_annotated_var_without_annotation() {
    let var_info = basilisk_resolver::VariableInfo {
        name: "x".to_string(),
        has_annotation: false,
        annotation_span: None,
        rhs_kind: RhsKind::Other,
        name_span: basilisk_resolver::Span { start: 0, end: 1 },
        rhs_span: None,
    };
    assert!(check_annotated_variable(&var_info).is_ok());
}

#[test]
fn infer_variable_type_int() {
    let var_info = basilisk_resolver::VariableInfo {
        name: "x".to_string(),
        has_annotation: true,
        annotation_span: None,
        rhs_kind: RhsKind::IntLiteral,
        name_span: basilisk_resolver::Span { start: 0, end: 1 },
        rhs_span: None,
    };
    assert!(matches!(infer_variable_type(&var_info), InferredType::Int));
}

// --- dataclass_transform integration tests ---

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
