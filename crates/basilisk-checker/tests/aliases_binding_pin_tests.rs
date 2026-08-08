//! Pins for [`aliases_newtype`] and [`aliases_implicit`] under
//! [ASTREBUILD-LAW] ([CHKARCH-DIAG-STRUCTURAL]): every verdict must come from
//! resolved bindings and the semantic relation layer
//! ([RESOLV-CANONICAL-RELATION]), never from identifier spellings.
//!
//! Spec sources: PEP 484 (`NewType` semantics — the constructor accepts only
//! values of the base type, and the returned object is not a class), PEP 613
//! (`TypeAlias`), and bounded `TypeVar`s
//! (<https://typing.python.org/en/latest/spec/generics.html#typevar>).
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs,
    dead_code
)]

mod common;

fn diagnostics(source: &str) -> Vec<common::Diagnostic> {
    common::run(source).expect("source must check")
}

fn assert_rule(source: &str, rule: &str, expected: usize, obligation: &str) -> Vec<String> {
    let diagnostics = diagnostics(source);
    common::assert_rule_count(&diagnostics, rule, expected, obligation);
    common::messages_for(&diagnostics, rule)
        .into_iter()
        .map(str::to_owned)
        .collect()
}

// --- aliases_newtype: PEP 484 `NewType` -----------------------------------

#[test]
fn aliased_base_type_still_relates_constructor_arguments() {
    // PEP 484 and the typing specification require the generated constructor
    // to accept exactly one value assignable to its base type. None of the
    // symbols used at the call site retain the conformance example's spelling.
    let msgs = assert_rule(
        r#"
from typing import NewType as mint_nominal
from builtins import int as mineral_number

QuarryTicket = mint_nominal(
    "QuarryTicket",
    mineral_number,
)
rejected_ticket = QuarryTicket("granite")
"#,
        "aliases_newtype",
        1,
        "a NewType constructor must reject a value not assignable to its resolved base",
    );
    assert!(
        msgs.iter()
            .any(|m| m.contains("not compatible with its base type")),
        "an aliased import of `int` IS `int`; the str argument must be \
         flagged regardless of the base's spelling — got: {msgs:?}"
    );
}

#[test]
fn aliased_isinstance_with_newtype_is_flagged() {
    // PEP 484: the object returned by NewType is not a real class; it
    // "cannot be used in isinstance() checks".
    let msgs = assert_rule(
        r#"
from typing import NewType as mint_nominal
from builtins import int as mineral_number
from builtins import isinstance as runtime_probe

CoreSample = mint_nominal("CoreSample", mineral_number)
invalid_probe = runtime_probe(1, CoreSample)
"#,
        "aliases_newtype",
        1,
        "the object returned by NewType is not a runtime class",
    );
    assert!(
        msgs.iter()
            .any(|m| m.contains("cannot be used as the second argument")),
        "an aliased import of `isinstance` IS `isinstance` — got: {msgs:?}"
    );
}

#[test]
fn shadowed_isinstance_with_newtype_is_not_flagged() {
    let msgs = assert_rule(
        r#"
from typing import NewType as mint_nominal
from builtins import int as mineral_number

CoreSample = mint_nominal("CoreSample", mineral_number)

def runtime_probe(candidate, category):
    return True

ordinary_result = runtime_probe(1, CoreSample)
"#,
        "aliases_newtype",
        0,
        "an unrelated user function must not be mistaken for the isinstance builtin",
    );
    assert!(
        !msgs
            .iter()
            .any(|m| m.contains("cannot be used as the second argument")),
        "the unrelated `runtime_probe` function is not the builtin runtime type check: {msgs:?}"
    );
}

#[test]
fn qualified_type_annotation_flags_newtype_assignment() {
    // PEP 484: `NewType(...)` returns a callable, not a class object, so it
    // is not assignable to a `type`-annotated variable.
    let msgs = assert_rule(
        r#"
import builtins as runtime_types
from typing import NewType as mint_nominal

CoreSample = mint_nominal("CoreSample", runtime_types.int)
category_object: runtime_types.type = CoreSample
"#,
        "aliases_newtype",
        1,
        "NewType returns a callable rather than a class object assignable to builtins.type",
    );
    assert!(
        msgs.iter().any(|m| m.contains("does not return a class object")),
        "`builtins.type` IS `type`; qualification must not change the \
         verdict — got: {msgs:?}"
    );
}

#[test]
fn shadowed_type_annotation_does_not_flag_newtype_assignment() {
    let msgs = assert_rule(
        r#"
from typing import NewType as mint_nominal
from builtins import int as mineral_number

CoreSample = mint_nominal("CoreSample", mineral_number)

class category_object:
    pass

record: category_object = CoreSample
"#,
        "aliases_newtype",
        0,
        "an unrelated user class must not be treated as builtins.type by spelling",
    );
    assert!(
        !msgs.iter().any(|m| m.contains("does not return a class object")),
        "the unrelated `category_object` class does not denote the builtin `type`: {msgs:?}"
    );
}

// --- aliases_implicit: bounded TypeVar arguments (PEP 484) -----------------

#[test]
fn aliased_bound_and_argument_still_check() {
    // Typing spec, generics: "the type parameter must be a subtype of the
    // bound" — identity comes from bindings, not spelling.
    let msgs = assert_rule(
        r#"
from typing import TypeAlias as alias_declaration
from typing import TypeVar as parameter_factory

class GeologicalRecord:
    pass

class UnrelatedArtifact:
    pass

RecordKind = parameter_factory("RecordKind", bound=GeologicalRecord)
RecordShelf: alias_declaration = list[RecordKind]
rejected_shelf: RecordShelf[UnrelatedArtifact] = []
"#,
        "aliases_implicit",
        1,
        "a substituted type must be assignable to the TypeVar's resolved upper bound",
    );
    assert!(
        msgs.iter().any(|m| m.contains("does not satisfy bound")),
        "`UnrelatedArtifact` is not assignable to `GeologicalRecord`: {msgs:?}"
    );
}

#[test]
fn plain_bound_violation_still_flagged() {
    let msgs = assert_rule(
        r#"
import typing as type_contracts

class FoundationLayer:
    pass

class SurfaceArtifact:
    pass

LayerKind = type_contracts.TypeVar("LayerKind", bound=FoundationLayer)
LayerShelf: type_contracts.TypeAlias = list[LayerKind]
rejected_shelf: LayerShelf[SurfaceArtifact] = []
"#,
        "aliases_implicit",
        1,
        "a nominally unrelated type must violate the resolved upper bound",
    );
    assert!(
        msgs.iter().any(|m| m.contains("does not satisfy bound")),
        "`SurfaceArtifact` is not assignable to `FoundationLayer`: {msgs:?}"
    );
}

#[test]
fn satisfied_bound_is_not_flagged() {
    let msgs = assert_rule(
        r#"
from typing import TypeAlias as alias_declaration
from typing import TypeVar as parameter_factory

class GeologicalRecord:
    pass

class VerifiedRecord(GeologicalRecord):
    pass

RecordKind = parameter_factory("RecordKind", bound=GeologicalRecord)
RecordShelf: alias_declaration = list[RecordKind]
accepted_shelf: RecordShelf[VerifiedRecord] = []
"#,
        "aliases_implicit",
        0,
        "a subclass is assignable to its TypeVar upper bound",
    );
    assert!(
        !msgs.iter().any(|m| m.contains("does not satisfy bound")),
        "`VerifiedRecord` is a subclass of `GeologicalRecord`: {msgs:?}"
    );
}

#[test]
fn unresolved_bound_or_argument_abstains() {
    // Unresolvable imports lower to Unknown; the relation abstains rather
    // than inventing a verdict ([RESOLV-CANONICAL-RELATION]).
    let msgs = assert_rule(
        r#"
from typing import TypeAlias as alias_declaration
from typing import TypeVar as parameter_factory
from unavailable_geology import FoundationLayer, CandidateLayer

LayerKind = parameter_factory("LayerKind", bound=FoundationLayer)
LayerShelf: alias_declaration = list[LayerKind]
unknown_shelf: LayerShelf[CandidateLayer] = []
"#,
        "aliases_implicit",
        0,
        "unknown types must not fabricate an upper-bound incompatibility",
    );
    assert!(
        !msgs.iter().any(|m| m.contains("does not satisfy bound")),
        "neither side resolves; the checker must abstain, not guess — \
         got: {msgs:?}"
    );
}
