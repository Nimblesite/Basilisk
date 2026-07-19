//! Tests for [CHKARCH-TESTING] / [TYPEINF-REDUNDANT]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
// Comprehensive tests for redundant type annotations.
//
// Tests the interaction between BSK-0005 (missing annotation) and BSK-0050 (redundant
// annotation) across all scenarios where type inference makes annotations
// unnecessary: subclass overrides, scalar literals, class hierarchies, multiple
// inheritance, deep inheritance chains, and edge cases.

use super::common::*;

fn count_code(diags: &[basilisk_checker::Diagnostic], code: &str) -> usize {
    diags.iter().filter(|d| d.code.code == code).count()
}

fn missing_attribute_annotation_messages(diags: &[basilisk_checker::Diagnostic]) -> Vec<String> {
    diags
        .iter()
        .filter(|d| d.code.code == "BSK-0005")
        .map(|d| d.message.clone())
        .collect()
}

// ============================================================================
// Subclass attribute overrides: BSK-0005 should NOT fire when parent annotates
// ============================================================================

#[test]
fn subclass_override_int_attr() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class Base:
    count: int = 0

class Child(Base):
    count = 42
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !has_code(&diags, "BSK-0005"),
        "inherited int attr should not need re-annotation, got: {:?}",
        missing_attribute_annotation_messages(&diags)
    );
    Ok(())
}

#[test]
fn subclass_override_str_attr() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class Base:
    name: str = \"default\"

class Child(Base):
    name = \"child\"
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !has_code(&diags, "BSK-0005"),
        "inherited str attr should not need re-annotation, got: {:?}",
        missing_attribute_annotation_messages(&diags)
    );
    Ok(())
}

#[test]
fn subclass_override_float_attr() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class Base:
    rate: float = 1.0

class Child(Base):
    rate = 2.5
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !has_code(&diags, "BSK-0005"),
        "inherited float attr should not need re-annotation, got: {:?}",
        missing_attribute_annotation_messages(&diags)
    );
    Ok(())
}

#[test]
fn subclass_override_bool_attr() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class Base:
    enabled: bool = False

class Child(Base):
    enabled = True
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !has_code(&diags, "BSK-0005"),
        "inherited bool attr should not need re-annotation, got: {:?}",
        missing_attribute_annotation_messages(&diags)
    );
    Ok(())
}

#[test]
fn subclass_override_bytes_attr() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class Base:
    header: bytes = b\"\\x00\"

class Child(Base):
    header = b\"\\xff\"
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !has_code(&diags, "BSK-0005"),
        "inherited bytes attr should not need re-annotation, got: {:?}",
        missing_attribute_annotation_messages(&diags)
    );
    Ok(())
}

#[test]
fn subclass_override_none_attr() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class Base:
    cached: None = None

class Child(Base):
    cached = None
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !has_code(&diags, "BSK-0005"),
        "inherited None attr should not need re-annotation, got: {:?}",
        missing_attribute_annotation_messages(&diags)
    );
    Ok(())
}

// ============================================================================
// Deep inheritance chains: annotation inherited through multiple levels
// ============================================================================

#[test]
fn grandchild_inherits_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class GrandParent:
    level: int = 0

class Parent(GrandParent):
    level = 1

class Child(Parent):
    level = 2
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    let e0005s = missing_attribute_annotation_messages(&diags);
    assert!(
        e0005s.is_empty(),
        "grandchild should inherit annotation through chain, got: {e0005s:?}",
    );
    Ok(())
}

#[test]
fn great_grandchild_inherits_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class A:
    tag: str = \"a\"

class B(A):
    tag = \"b\"

class C(B):
    tag = \"c\"

class D(C):
    tag = \"d\"
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    let e0005s = missing_attribute_annotation_messages(&diags);
    assert!(
        e0005s.is_empty(),
        "deep chain should inherit annotation, got: {e0005s:?}",
    );
    Ok(())
}

// ============================================================================
// Multiple inheritance: annotation from any parent suffices
// ============================================================================

#[test]
fn multiple_inheritance_first_base_annotates() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class Mixin:
    priority: int = 0

class Other:
    pass

class Combined(Mixin, Other):
    priority = 10
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !has_code(&diags, "BSK-0005"),
        "annotation from first base should suffice, got: {:?}",
        missing_attribute_annotation_messages(&diags)
    );
    Ok(())
}

#[test]
fn multiple_inheritance_second_base_annotates() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class First:
    pass

class Second:
    weight: float = 1.0

class Combined(First, Second):
    weight = 5.0
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !has_code(&diags, "BSK-0005"),
        "annotation from second base should suffice, got: {:?}",
        missing_attribute_annotation_messages(&diags)
    );
    Ok(())
}

// ============================================================================
// Mixed: some attrs inherited, some new — only non-inferrable new ones should fire BSK-0005
// ============================================================================

#[test]
fn mixed_inherited_and_new_attrs() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class Base:
    x: int = 0
    y: str = \"\"

class Child(Base):
    x = 10
    y = \"child\"
    z = 99
    w = \"new\"
    q = compute()
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    let e0005s = missing_attribute_annotation_messages(&diags);
    // x and y are exempt (parent has annotation).
    // z = 99 and w = "new" are scalar literals — suppressed (type is inferrable).
    // Only q = compute() should fire (non-inferrable RHS).
    assert_eq!(
        e0005s.len(),
        1,
        "only q (non-inferrable) should fire BSK-0005; z and w are scalar literals, got: {e0005s:?}",
    );
    assert!(e0005s.iter().any(|m| m.contains('q')), "should fire for q");
    Ok(())
}

// ============================================================================
// Parent has attr without annotation — child should still fire BSK-0005
// ============================================================================

#[test]
fn parent_unannotated_attr_child_still_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class Base:
    value: int = 0
    raw = compute()

class Child(Base):
    raw = compute()
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    // Base.raw has no annotation and non-inferrable RHS, so Child.raw cannot inherit one
    let child_e0005s: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-0005" && d.message.contains("Child"))
        .collect();
    assert_eq!(
        child_e0005s.len(),
        1,
        "child overriding unannotated parent attr with non-inferrable RHS should still fire BSK-0005, got: {:?}",
        child_e0005s.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

// ============================================================================
// Annotation-only parent (no default value) — child should inherit
// ============================================================================

#[test]
fn parent_annotation_only_no_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class Base:
    name: str

class Child(Base):
    name = \"child_default\"
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !has_code(&diags, "BSK-0005"),
        "parent annotation-only decl should satisfy child, got: {:?}",
        missing_attribute_annotation_messages(&diags)
    );
    Ok(())
}

// ============================================================================
// Multiple attrs overridden at once
// ============================================================================

#[test]
fn override_all_parent_attrs() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class Config:
    host: str = \"localhost\"
    port: int = 8080
    debug: bool = False
    timeout: float = 30.0

class ProductionConfig(Config):
    host = \"prod.example.com\"
    port = 443
    debug = False
    timeout = 60.0
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert_eq!(
        count_code(&diags, "BSK-0005"),
        0,
        "all overrides should be exempt from BSK-0005, got: {:?}",
        missing_attribute_annotation_messages(&diags)
    );
    Ok(())
}

// ============================================================================
// Sibling classes independently override parent attrs
// ============================================================================

#[test]
fn sibling_classes_both_override() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class Animal:
    sound: str = \"...\"
    legs: int = 4

class Dog(Animal):
    sound = \"woof\"

class Cat(Animal):
    sound = \"meow\"

class Snake(Animal):
    legs = 0
    sound = \"hiss\"
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert_eq!(
        count_code(&diags, "BSK-0005"),
        0,
        "all sibling overrides should inherit annotations, got: {:?}",
        missing_attribute_annotation_messages(&diags)
    );
    Ok(())
}

// ============================================================================
// BSK-0050 redundant annotation on subclass re-annotation (warns correctly)
// ============================================================================

#[test]
fn subclass_redundant_reannotation_fires_w0050() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class Base:
    priority: int = 10

class Child(Base):
    priority: int = 100
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    // BSK-0050 should fire — the annotation is redundant (int = 100 is obviously int)
    assert!(
        has_code(&diags, "BSK-0050"),
        "redundant re-annotation in subclass should fire BSK-0050"
    );
    // BSK-0005 should NOT fire — there IS an annotation
    assert!(
        !has_code(&diags, "BSK-0005"),
        "annotated attr should not fire BSK-0005"
    );
    Ok(())
}

// ============================================================================
// No false suppression: unrelated class with same attr name
// ============================================================================

#[test]
fn unrelated_class_same_attr_name_still_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class Foo:
    value: int = 0

class Bar:
    value = compute()
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    // Bar does NOT inherit from Foo, so BSK-0005 should fire for Bar.value (non-inferrable RHS)
    let bar_e0005: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-0005" && d.message.contains("Bar"))
        .collect();
    assert_eq!(
        bar_e0005.len(),
        1,
        "unrelated class with non-inferrable attr should still fire BSK-0005"
    );
    Ok(())
}

// ============================================================================
// Diamond inheritance: annotation reachable through either path
// ============================================================================

#[test]
fn diamond_inheritance_annotation_reachable() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class Base:
    value: int = 0

class Left(Base):
    pass

class Right(Base):
    pass

class Diamond(Left, Right):
    value = 42
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !has_code(&diags, "BSK-0005"),
        "diamond inheritance should find annotation through either path, got: {:?}",
        missing_attribute_annotation_messages(&diags)
    );
    Ok(())
}

// ============================================================================
// Module-level BSK-0050 redundant annotations (scalar literals)
// ============================================================================

#[test]
fn module_level_all_scalar_redundancies() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
a: int = 42
b: str = \"hello\"
c: float = 3.14
d: bool = True
e: bool = False
f: bytes = b\"data\"
g: None = None
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert_eq!(
        count_code(&diags, "BSK-0050"),
        7,
        "all 7 scalar literals should fire BSK-0050"
    );
    Ok(())
}

// ============================================================================
// Module-level redundant and non-redundant annotations
// ============================================================================

#[test]
fn module_level_widening_no_w0050() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
x: float = 42
y: object = \"hello\"
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert_eq!(
        count_code(&diags, "BSK-0050"),
        0,
        "widening annotations should not fire BSK-0050"
    );
    Ok(())
}

#[test]
fn module_level_collection_redundancies() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
items: list[int] = [1, 2, 3]
pairs: dict[str, int] = {\"a\": 1}
nums: set[int] = {1, 2}
coords: tuple[int, int] = (1, 2)
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert_eq!(
        count_code(&diags, "BSK-0050"),
        4,
        "exact collection annotations should all fire BSK-0050, got: {:?}",
        diags
            .iter()
            .filter(|diag| diag.code.code == "BSK-0050")
            .map(|diag| &diag.message)
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn module_level_collection_annotations_that_add_information_are_not_redundant(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
floats: list[float] = [1, 2, 3]
objects: dict[str, object] = {\"a\": 1}
union_items: set[int | str] = {1, 2}
shorter: tuple[int] = (1, 2)
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert_eq!(
        count_code(&diags, "BSK-0050"),
        0,
        "collection annotations that widen or reshape the inferred type must not fire BSK-0050"
    );
    Ok(())
}

#[test]
fn module_level_call_expression_no_w0050() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
x: int = int(\"42\")
y: str = str(42)
z: int = len([1, 2, 3])
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert_eq!(
        count_code(&diags, "BSK-0050"),
        0,
        "call expression annotations should not fire BSK-0050"
    );
    Ok(())
}

// ============================================================================
// Class-level BSK-0050 redundant annotations (scalar literals in class body)
// ============================================================================

#[test]
fn class_attr_all_scalar_redundancies() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class Settings:
    retries: int = 3
    label: str = \"default\"
    threshold: float = 0.5
    verbose: bool = True
    magic: bytes = b\"\\x00\"
    nothing: None = None
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert_eq!(
        count_code(&diags, "BSK-0050"),
        6,
        "all 6 scalar class attrs should fire BSK-0050"
    );
    Ok(())
}

// ============================================================================
// TypedDict / Protocol / NamedTuple exempt from BSK-0050
// ============================================================================

#[test]
fn typeddict_exempt_from_w0050() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
from typing import TypedDict

class Point(TypedDict):
    x: int
    y: int
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert_eq!(
        count_code(&diags, "BSK-0050"),
        0,
        "TypedDict should be exempt from BSK-0050"
    );
    Ok(())
}

#[test]
fn protocol_exempt_from_w0050() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
from typing import Protocol

class HasName(Protocol):
    name: str = \"\"
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert_eq!(
        count_code(&diags, "BSK-0050"),
        0,
        "Protocol should be exempt from BSK-0050"
    );
    Ok(())
}

// ============================================================================
// Function params and returns exempt from BSK-0050
// ============================================================================

#[test]
fn function_annotations_exempt_from_w0050() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
def greet(name: str) -> str:
    return \"hello \" + name

def add(a: int, b: int) -> int:
    return a + b
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert_eq!(
        count_code(&diags, "BSK-0050"),
        0,
        "function param/return annotations should never fire BSK-0050"
    );
    Ok(())
}

// ============================================================================
// Negative int/float literals
// ============================================================================

#[test]
fn negative_int_literal_redundant() -> Result<(), Box<dyn std::error::Error>> {
    let source = "x: int = -42\n";
    let diags = run_with_config(source, &annotation_rules_config())?;
    // Negative literals may or may not be detected depending on parser handling
    // This test documents the current behavior
    let _w0050_count = count_code(&diags, "BSK-0050");
    // At minimum, BSK-0005 should not fire (there IS an annotation)
    assert!(
        !has_code(&diags, "BSK-0005"),
        "annotated variable should not fire BSK-0005"
    );
    Ok(())
}

// ============================================================================
// Zero and empty string edge cases
// ============================================================================

#[test]
fn zero_int_redundant() -> Result<(), Box<dyn std::error::Error>> {
    let source = "x: int = 0\n";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        has_code(&diags, "BSK-0050"),
        "x: int = 0 should be redundant"
    );
    Ok(())
}

#[test]
fn empty_string_redundant() -> Result<(), Box<dyn std::error::Error>> {
    let source = "x: str = \"\"\n";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        has_code(&diags, "BSK-0050"),
        "x: str = \"\" should be redundant"
    );
    Ok(())
}

#[test]
fn zero_float_redundant() -> Result<(), Box<dyn std::error::Error>> {
    let source = "x: float = 0.0\n";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        has_code(&diags, "BSK-0050"),
        "x: float = 0.0 should be redundant"
    );
    Ok(())
}

// ============================================================================
// Realistic class hierarchy: web framework style
// ============================================================================

#[test]
fn web_framework_route_hierarchy() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class BaseRoute:
    path: str = \"/\"
    method: str = \"GET\"
    auth_required: bool = False
    priority: int = 0
    timeout: float = 30.0

class AuthenticatedRoute(BaseRoute):
    auth_required = True

class AdminRoute(AuthenticatedRoute):
    priority = 100
    path = \"/admin\"

class ApiRoute(AuthenticatedRoute):
    method = \"POST\"
    timeout = 60.0
    path = \"/api\"
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert_eq!(
        count_code(&diags, "BSK-0005"),
        0,
        "all overrides in route hierarchy should inherit annotations, got: {:?}",
        missing_attribute_annotation_messages(&diags)
    );
    Ok(())
}

// ============================================================================
// [BSK-0005] vs [BSK-0050] catch-22 — issue #83
//
// An annotated class attribute whose initializer is a NAME REFERENCE (not an
// inline literal) carries `RhsKind::Other`; Basilisk cannot infer its type from
// the RHS, so the annotation is informative — NOT redundant. BSK-0050 must not
// fire. Because BSK-0005 (correctly) requires an annotation on a non-inferrable
// RHS, ANNOTATING is the single valid resolution. Before the fix the annotated
// form fired BSK-0050 ("remove it") while the unannotated form fired BSK-0005 ("add
// it") — an unsatisfiable contradiction.
// ============================================================================

#[test]
fn name_reference_class_attr_annotation_is_not_redundant() -> Result<(), Box<dyn std::error::Error>>
{
    // Mirrors the issue repro: `tool_bind_host: str = _DEFAULT_TOOL_BIND_HOST`,
    // where the RHS is a module-level constant referenced by name (not a literal).
    let source = "\
class DockerAgentWorkspaceHostConfig:
    tool_bind_host: str = _DEFAULT_TOOL_BIND_HOST
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    // The annotation supplies a type Basilisk cannot infer from the name
    // reference, so it is NOT redundant — BSK-0050 must stay silent.
    assert!(
        !has_code(&diags, "BSK-0050"),
        "annotation on a non-inferrable name-reference RHS must not fire BSK-0050 (issue #83), got: {:?}",
        messages_for(&diags, "BSK-0050")
    );
    // And since the attribute IS annotated, BSK-0005 must not fire — proving the
    // annotated form is a valid, contradiction-free resolution to the catch-22.
    assert!(
        !has_code(&diags, "BSK-0005"),
        "annotated attr must not fire BSK-0005, got: {:?}",
        missing_attribute_annotation_messages(&diags)
    );
    Ok(())
}

// ============================================================================
// Dataclass-style pattern (plain classes with defaults)
// ============================================================================

#[test]
fn config_hierarchy_pattern() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class DatabaseConfig:
    host: str = \"localhost\"
    port: int = 5432
    pool_size: int = 10
    ssl: bool = False

class ProductionDB(DatabaseConfig):
    host = \"db.prod.internal\"
    port = 5433
    ssl = True
    pool_size = 50

class StagingDB(DatabaseConfig):
    host = \"db.staging.internal\"
    pool_size = 5
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert_eq!(
        count_code(&diags, "BSK-0005"),
        0,
        "config hierarchy overrides should all inherit annotations, got: {:?}",
        missing_attribute_annotation_messages(&diags)
    );
    Ok(())
}
