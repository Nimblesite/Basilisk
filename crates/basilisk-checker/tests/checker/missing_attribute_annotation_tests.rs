//! Tests for [BSK-E0005] from [CHKARCH-DIAG-MISSING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-MISSING
// Integration tests for BSK-E0005: Missing class attribute type annotation.

use super::common::*;

#[test]
fn scalar_literal_suppressed() -> Result<(), Box<dyn std::error::Error>> {
    let source = "class Foo:\n    value = 42\n";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-E0005"),
        "scalar literal (int) class attr should not fire BSK-E0005 — type is trivially inferrable"
    );
    Ok(())
}

#[test]
fn annotated_class_attr_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "class Foo:\n    value: int = 42\n";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-E0005"),
        "annotated class attr should not fire BSK-E0005"
    );
    Ok(())
}

#[test]
fn enum_class_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from enum import Enum\n\nclass Color(Enum):\n    RED = 1\n    GREEN = 2\n";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-E0005"),
        "Enum class should be exempt from BSK-E0005"
    );
    Ok(())
}

#[test]
fn protocol_class_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source =
        "from typing import Protocol\n\nclass MyProto(Protocol):\n    name = \"default\"\n";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-E0005"),
        "Protocol class should be exempt from BSK-E0005"
    );
    Ok(())
}

#[test]
fn namedtuple_class_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import NamedTuple\n\nclass Point(NamedTuple):\n    x = 0\n";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-E0005"),
        "NamedTuple class should be exempt from BSK-E0005"
    );
    Ok(())
}

#[test]
fn subclass_overriding_annotated_parent_attr_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class BaseRoute:
    priority: int = 10

class AdminRoute(BaseRoute):
    priority = 100
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    let e0005_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0005")
        .collect();
    assert!(
        e0005_diags.is_empty(),
        "subclass overriding an annotated parent attribute should not fire BSK-E0005, got: {:?}",
        e0005_diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn all_scalar_literal_types_suppressed() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class Config:
    retries = 3
    label = \"default\"
    threshold = 0.5
    verbose = True
    magic = b\"\\x00\"
    nothing = None
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    let e0005_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0005")
        .collect();
    assert!(
        e0005_diags.is_empty(),
        "scalar literal attrs (int, str, float, bool, bytes, None) should not fire BSK-E0005, got: {:?}",
        e0005_diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn scalar_string_literal_suppressed() -> Result<(), Box<dyn std::error::Error>> {
    let source = "class Foo:\n    label = \"hello\"\n";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-E0005"),
        "scalar string literal class attr should not fire BSK-E0005 — type is trivially str"
    );
    Ok(())
}

#[test]
fn scalar_bool_literal_suppressed() -> Result<(), Box<dyn std::error::Error>> {
    let source = "class Foo:\n    flag = True\n";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-E0005"),
        "scalar bool literal class attr should not fire BSK-E0005 — type is trivially bool"
    );
    Ok(())
}

#[test]
fn multiple_scalar_attrs_suppressed() -> Result<(), Box<dyn std::error::Error>> {
    let source = "class Foo:\n    a = 1\n    b = 2\n    c = 3\n";
    let diags = run_with_config(source, &annotation_rules_config())?;
    let count = diags.iter().filter(|d| d.code.code == "BSK-E0005").count();
    assert_eq!(
        count, 0,
        "scalar literal attrs should not fire BSK-E0005 — types are trivially inferrable"
    );
    Ok(())
}

#[test]
fn subclass_new_scalar_attr_suppressed() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class BaseRoute:
    priority: int = 10

class AdminRoute(BaseRoute):
    priority = 100
    new_attr = 42
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    let e0005_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0005")
        .collect();
    assert!(
        e0005_diags.is_empty(),
        "scalar literal attrs (even new ones in subclass) should not fire BSK-E0005, got: {:?}",
        e0005_diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn non_inferrable_rhs_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class Foo:
    result = some_function()
    computed = x + y
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    let e0005_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0005")
        .collect();
    assert!(
        !e0005_diags.is_empty(),
        "non-inferrable RHS attrs should fire BSK-E0005"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Regression tests: scalar literals must NEVER produce false-positive BSK-E0005.
// These tests exercise real-world patterns that previously regressed.
// ---------------------------------------------------------------------------

/// The exact pattern from the `redundant_annotations.py` example file:
/// an inheritance hierarchy where parent and children use scalar literals.
/// NONE of these should fire BSK-E0005.
#[test]
fn regression_animal_hierarchy_no_false_positives() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class Animal:
    sound = \"...\"
    legs = 4

class Dog(Animal):
    sound = \"woof\"

class Cat(Animal):
    sound = \"meow\"

class Snake(Animal):
    legs = 0
    sound = \"hiss\"
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    let e0005_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0005")
        .collect();
    assert!(
        e0005_diags.is_empty(),
        "scalar literal attrs in animal hierarchy must not fire BSK-E0005 (regression), got: {:?}",
        e0005_diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

/// The config override pattern: base config with scalar defaults, subclasses
/// override with new scalar values. No BSK-E0005 anywhere.
#[test]
fn regression_config_override_pattern() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class DatabaseConfig:
    host = \"localhost\"
    port = 5432
    pool_size = 10
    ssl = False

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
    let e0005_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0005")
        .collect();
    assert!(
        e0005_diags.is_empty(),
        "config override pattern with scalar literals must not fire BSK-E0005 (regression), got: {:?}",
        e0005_diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

/// Standalone classes with scalar literals — type is trivially inferrable.
#[test]
fn regression_standalone_scalar_classes() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class Standalone:
    value = 42

class UnannotatedParent:
    raw = 99

class ChildOfUnannotated(UnannotatedParent):
    raw = 100

class UnrelatedToBaseRoute:
    path = \"/unrelated\"
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    let e0005_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0005")
        .collect();
    assert!(
        e0005_diags.is_empty(),
        "standalone classes with scalar literals must not fire BSK-E0005 (regression), got: {:?}",
        e0005_diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

/// Each scalar literal type individually — exhaustive per-type regression guard.
#[test]
fn regression_each_scalar_type_individually() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        ("int", "class A:\n    x = 42\n"),
        ("float", "class B:\n    x = 3.14\n"),
        ("str", "class C:\n    x = \"hello\"\n"),
        ("bool", "class D:\n    x = True\n"),
        ("bytes", "class E:\n    x = b\"data\"\n"),
        ("None", "class F:\n    x = None\n"),
    ];
    for (type_name, source) in cases {
        let diags = run_with_config(source, &annotation_rules_config())?;
        let e0005_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.code.code == "BSK-E0005")
            .collect();
        assert!(
            e0005_diags.is_empty(),
            "{type_name} literal must not fire BSK-E0005 (regression), got: {:?}",
            e0005_diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }
    Ok(())
}

/// Mixed class: some attrs are scalar (suppressed), some are not (should fire).
/// Ensures the filter is precise — only scalars suppressed, non-inferrable still fires.
#[test]
fn regression_mixed_scalar_and_non_inferrable() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class Mixed:
    name = \"default\"
    count = 0
    flag = False
    unknown = some_call()
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    let e0005_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0005")
        .collect();
    assert_eq!(
        e0005_diags.len(),
        1,
        "only the non-inferrable attr should fire BSK-E0005, scalars must be suppressed, got: {:?}",
        e0005_diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    assert!(
        e0005_diags[0].message.contains("unknown"),
        "BSK-E0005 should fire for `unknown`, got: {}",
        e0005_diags[0].message
    );
    Ok(())
}

#[test]
fn type_alias_type_in_class_exempt() -> Result<(), Box<dyn std::error::Error>> {
    // A class-body `X = TypeAliasType("X", ...)` is a type-alias definition, not
    // a data attribute, so it must not require an annotation
    // (conformance aliases_typealiastype.py).
    let source = "from typing import TypeAliasType\nclass A:\n    GoodAlias = TypeAliasType(\"GoodAlias\", list[int])\n";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-E0005"),
        "TypeAliasType alias in a class body must not fire BSK-E0005, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn tuple_literal_match_args_exempt() -> Result<(), Box<dyn std::error::Error>> {
    // A tuple literal of inferrable elements is fully inferrable; `__match_args__`
    // must not require an annotation (conformance dataclasses_match_args.py).
    let source = "class DC:\n    __match_args__ = (\"a\", \"b\")\n    empty = ()\n";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-E0005"),
        "tuple-literal class attrs must not fire BSK-E0005, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn pep695_type_param_attr_exempt() -> Result<(), Box<dyn std::error::Error>> {
    // An attribute whose name matches one of the class's PEP 695 type parameters
    // is the type variable in scope, not a data attribute
    // (conformance generics_syntax_scoping.py).
    let source = "class C[T]:\n    T = 0\n";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-E0005"),
        "PEP 695 type-param-named attr must not fire BSK-E0005, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn empty_collection_still_fires() -> Result<(), Box<dyn std::error::Error>> {
    // The tuple exemption must NOT leak to empty list/dict (element types unknown).
    let source = "class Foo:\n    data = []\n";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        codes(&diags).contains(&"BSK-E0005"),
        "empty-list class attr must still fire BSK-E0005 (element type unknown), got: {:?}",
        codes(&diags)
    );
    Ok(())
}
