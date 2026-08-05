//! Tests for [TYPEINF-ANNOTATION-RESOLUTION]. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-ANNOTATION-RESOLUTION
//
// The cascade is exercised through `returns_compatibility`, the first rule
// migrated onto `crate::annotation::AnnotationResolver`. Every case here was
// RED before the cascade landed: `InferredType::from_annotation(<source
// text>)` turned each of these annotations into an opaque `Named(..)`, and
// `shared::is_unverifiable_return_type` skipped every `Named` — so a wrong
// return through an alias or a same-file class drew nothing at all
// (Refs #378).

use super::common::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Assert `returns_compatibility` fires — the annotation resolved to a
/// checkable type and the returned value does not fit it.
fn assert_fires(source: &str, why: &str) -> TestResult {
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"returns_compatibility"),
        "{why}, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// Assert silence — either the value fits, or the name is genuinely
/// unresolvable and stays gradual.
fn assert_silent(source: &str, why: &str) -> TestResult {
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"returns_compatibility"),
        "{why}, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Step 1 of the cascade — the type-alias table
// ---------------------------------------------------------------------------

#[test]
fn pep695_type_alias_target_is_checked() -> TestResult {
    assert_fires(
        "type MyInt = int\n\ndef f() -> MyInt:\n    return \"x\"\n",
        "a PEP 695 alias must expand to `int` and reject a str return",
    )
}

#[test]
fn pep695_type_alias_target_accepts_matching_value() -> TestResult {
    assert_silent(
        "type MyInt = int\n\ndef f() -> MyInt:\n    return 1\n",
        "expanding the alias must not make a correct return fire",
    )
}

#[test]
fn explicit_typealias_target_is_checked() -> TestResult {
    assert_fires(
        "from typing import TypeAlias\n\nMyInt: TypeAlias = int\n\ndef f() -> MyInt:\n    return \"x\"\n",
        "an `X: TypeAlias = ...` declaration must expand like a PEP 695 alias",
    )
}

#[test]
fn implicit_alias_target_is_checked() -> TestResult {
    assert_fires(
        "MyInt = int\n\ndef f() -> MyInt:\n    return \"x\"\n",
        "an implicit alias (`X = <type expression>`) must expand",
    )
}

#[test]
fn alias_chain_expands_to_the_root_type() -> TestResult {
    assert_fires(
        "type A = B\ntype B = int\n\ndef f() -> A:\n    return \"x\"\n",
        "an alias chain must expand transitively to `int`",
    )
}

#[test]
fn alias_used_before_declaration_still_expands() -> TestResult {
    // Declaration order must not decide resolution: the tables are built for
    // the whole module before any annotation is resolved.
    assert_fires(
        "def f() -> Later:\n    return \"x\"\n\ntype Later = int\n",
        "an alias declared AFTER the function must still expand",
    )
}

#[test]
fn implicit_alias_may_reference_a_later_declaration() -> TestResult {
    assert_fires(
        "Early = Late\ntype Late = int\n\ndef f() -> Early:\n    return \"x\"\n",
        "the implicit-alias pass runs after the explicit one, so forward references resolve",
    )
}

#[test]
fn alias_expands_at_every_nesting_depth() -> TestResult {
    assert_fires(
        "type Elem = int\n\ndef f() -> list[Elem]:\n    return [\"x\"]\n",
        "an alias nested inside `list[..]` must expand",
    )
}

#[test]
fn alias_nested_two_levels_deep_expands() -> TestResult {
    assert_fires(
        "type Elem = int\n\ndef f() -> dict[str, list[Elem]]:\n    return {\"k\": [\"x\"]}\n",
        "alias transparency is not depth-limited",
    )
}

#[test]
fn generic_alias_binds_its_parameter() -> TestResult {
    assert_fires(
        "type Pair[T] = list[T]\n\ndef f() -> Pair[int]:\n    return [\"x\"]\n",
        "a parameterised alias must substitute its argument",
    )
}

#[test]
fn recursive_alias_terminates_and_stays_silent() -> TestResult {
    // Refs #371. The cycle guard must stop expansion without rejecting the
    // alias — an infinite expansion would hang the checker.
    assert_silent(
        "type J = list[J]\n\ndef f() -> J:\n    return []\n",
        "a recursive alias must terminate and not fire",
    )
}

#[test]
fn non_type_assignment_is_not_an_alias() -> TestResult {
    // `X = 5` binds a value, not a type. Treating it as an alias would resolve
    // the annotation to nonsense; it must stay gradual instead.
    assert_silent(
        "MyInt = 5\n\ndef f() -> MyInt:\n    return \"x\"\n",
        "a value binding must not be read as a type alias",
    )
}

// ---------------------------------------------------------------------------
// Step 2 of the cascade — the same-file class table
// ---------------------------------------------------------------------------

#[test]
fn same_file_class_target_is_checked() -> TestResult {
    assert_fires(
        "class C:\n    pass\n\ndef f() -> C:\n    return 42\n",
        "a resolvable same-file class is nominal — an int literal cannot satisfy it",
    )
}

#[test]
fn same_file_class_declared_after_use_is_checked() -> TestResult {
    assert_fires(
        "def f() -> C:\n    return 42\n\nclass C:\n    pass\n",
        "class resolution must not depend on declaration order",
    )
}

#[test]
fn nested_class_target_is_checked() -> TestResult {
    assert_fires(
        "class Outer:\n    class Inner:\n        pass\n\ndef f() -> Inner:\n    return 42\n",
        "classes are collected at any nesting depth",
    )
}

#[test]
fn protocol_class_target_stays_gradual() -> TestResult {
    // Structural assignability is not modelled yet, so a Protocol target must
    // NOT be treated as nominal — doing so would be a false positive on
    // spec-valid code.
    assert_silent(
        "from typing import Protocol\n\nclass P(Protocol):\n    pass\n\ndef f() -> P:\n    return 42\n",
        "a Protocol target must stay gradual until structural typing lands",
    )
}

#[test]
fn typeddict_class_target_stays_gradual() -> TestResult {
    assert_silent(
        "from typing import TypedDict\n\nclass T(TypedDict):\n    a: int\n\ndef f() -> T:\n    return {}\n",
        "a TypedDict target is structural and must stay gradual",
    )
}

#[test]
fn user_class_shadowing_a_builtin_wins() -> TestResult {
    // Builtins are consulted LAST, so a module-level declaration shadows one
    // exactly as Python does: this `int` is the user's class, and a str
    // literal does not satisfy it.
    assert_fires(
        "class int:\n    pass\n\ndef f() -> int:\n    return \"x\"\n",
        "a same-file class must shadow the builtin of the same name",
    )
}

// ---------------------------------------------------------------------------
// Step 3 of the cascade — imports (typeshed seam left for #324)
// ---------------------------------------------------------------------------

#[test]
fn unresolved_imported_name_stays_gradual() -> TestResult {
    // Project-symbol resolution is not delivered yet; until it is, an
    // imported name is `Unknown` and must suppress rather than guess.
    assert_silent(
        "from other_module import Thing\n\ndef f() -> Thing:\n    return 42\n",
        "an unresolved imported name must stay gradual, never fire",
    )
}

#[test]
fn imported_typing_alias_resolves_through_its_original_name() -> TestResult {
    // `from typing import List as L` must resolve `L` to `list`, which means
    // keeping the ORIGINAL name across the alias.
    assert_fires(
        "from typing import List as L\n\ndef f() -> L[int]:\n    return \"x\"\n",
        "an aliased typing import must resolve through its original name",
    )
}

#[test]
fn typing_attribute_spelling_resolves() -> TestResult {
    assert_fires(
        "import typing\n\ndef f() -> typing.List[int]:\n    return \"x\"\n",
        "the `typing.X` attribute spelling must resolve like the bare member",
    )
}

#[test]
fn aliased_typing_module_attribute_spelling_resolves() -> TestResult {
    assert_fires(
        "import typing as t\n\ndef f() -> t.List[int]:\n    return \"x\"\n",
        "`t.List` must resolve when `t` is bound to the typing module",
    )
}

// ---------------------------------------------------------------------------
// Step 5 of the cascade — forward references
// ---------------------------------------------------------------------------

#[test]
fn quoted_alias_forward_reference_expands() -> TestResult {
    assert_fires(
        "type MyInt = int\n\ndef f() -> \"MyInt\":\n    return \"x\"\n",
        "a string annotation is re-parsed and re-resolved through the same cascade",
    )
}

#[test]
fn quoted_same_file_class_forward_reference_resolves() -> TestResult {
    assert_fires(
        "def f() -> \"C\":\n    return 42\n\nclass C:\n    pass\n",
        "the classic forward-reference spelling must resolve to the class",
    )
}

// ---------------------------------------------------------------------------
// The `Literal` skip — the ONLY remaining value-dependent suppression
// ---------------------------------------------------------------------------

#[test]
fn literal_alias_target_stays_suppressed() -> TestResult {
    // `is_value_dependent_target` recurses THROUGH the resolved alias: the
    // kind-only return inference cannot see that `True` is `Literal[True]`.
    assert_silent(
        "from typing import Literal\n\ntype Flag = Literal[True]\n\ndef f() -> Flag:\n    return True\n",
        "a Literal reached through an alias must still suppress",
    )
}
