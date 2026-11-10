//! Pins REAL typing-spec behaviour that the text-matched rule bodies get
//! wrong. Every test here was first measured against the release binary
//! built from this checkout *before* the condemned helpers were deleted, so
//! each one records an observed defect, not a hypothesis.
//!
//! Two obligations are enforced:
//!
//! * **Spelling invariance** — two programs that differ only in how a type is
//!   written (`int` vs `"int"` vs `builtins.int`, `Union[A, B]` vs `A | B`,
//!   `C(x)` vs `(C)(x)`, a direct call vs an aliased one) mean the same thing
//!   to the typing spec and MUST draw identical diagnostics.
//! * **Verdict correctness** — spec-well-typed code draws no diagnostic, and
//!   spec-ill-typed code draws one, attributed to the rule that owns the
//!   obligation.
//!
//! These tests are expected to FAIL. They are the map of what has to be
//! rebuilt on resolved bindings; none of them may be deleted, weakened, or
//! made less assertive to obtain a green suite.

mod common;

use common::{assert_rule_count, run};

/// Diagnostic codes emitted for `source`, sorted and de-duplicated.
fn codes(source: &str) -> Vec<String> {
    let diagnostics = run(source).expect("checker ran");
    let mut out: Vec<String> = diagnostics.iter().map(|d| d.code.code.to_owned()).collect();
    out.sort();
    out.dedup();
    out
}

/// Assert two spellings of the same program draw the same diagnostics.
fn assert_same_verdict(what: &str, left: &str, right: &str) {
    let left_codes = codes(left);
    let right_codes = codes(right);
    assert_eq!(
        left_codes, right_codes,
        "{what}: these two programs mean the same thing to the typing spec, but the \
         first drew {left_codes:?} and the second drew {right_codes:?}; a verdict \
         that changes with the spelling is not a typing verdict"
    );
}

// ─────────────────────────── classes_override_2 ───────────────────────────
//
// PEP 484 forward references: a quoted annotation denotes exactly the type it
// names. `x: int` and `x: "int"` declare the same type, and `builtins.int` is
// that same type again. Redeclaring a base attribute at its own type is not an
// override violation in any of the three spellings.

/// Measured: `x: int` over `x: int` is silent, but `x: "int"` reports
/// `classes_override_2` — a false positive produced by comparing annotation
/// source slices for textual equality.
#[test]
fn quoted_annotation_is_the_same_type_as_the_bare_one() {
    assert_same_verdict(
        "a quoted forward reference denotes the same type as the bare name",
        "class A:\n    x: int = 0\n\n\nclass B(A):\n    x: int = 1\n",
        "class A:\n    x: int = 0\n\n\nclass B(A):\n    x: \"int\" = 1\n",
    );
}

/// Measured: `builtins.int` reports `classes_override_2` against a base
/// declared `int`. Same type, qualified spelling.
#[test]
fn qualified_builtin_is_the_same_type_as_the_bare_one() {
    assert_same_verdict(
        "`builtins.int` is the same type as `int`",
        "class A:\n    x: int = 0\n\n\nclass B(A):\n    x: int = 1\n",
        "import builtins\n\n\nclass A:\n    x: int = 0\n\n\nclass B(A):\n    \
         x: builtins.int = 1\n",
    );
}

// ──────────────────────────── dataclasses_frozen ──────────────────────────
//
// A `@dataclass(frozen=True)` instance rejects attribute assignment however
// the instance was obtained. The instance-to-class map must come from the
// resolved binding, not from slicing the assignment's right-hand side.

const FROZEN_POINT: &str =
    "from dataclasses import dataclass\n\n\n@dataclass(frozen=True)\nclass P:\n    x: float\n\n\n";

/// Measured: `b = P(1.0)` fires; `b = (P)(1.0)` is silent. One pair of
/// redundant parentheses cannot change whether a dataclass is frozen.
#[test]
fn parenthesised_constructor_call_still_yields_a_frozen_instance() {
    assert_same_verdict(
        "redundant parentheses around the callee do not change the instance's type",
        &format!("{FROZEN_POINT}b = P(1.0)\nb.x = 2.0\n"),
        &format!("{FROZEN_POINT}b = (P)(1.0)\nb.x = 2.0\n"),
    );
}

/// Measured: silent. A parameter annotated with the frozen dataclass is a
/// frozen instance; there is no assignment RHS to slice, which is precisely
/// why the text path misses it.
#[test]
fn frozen_dataclass_parameter_rejects_attribute_assignment() {
    let source = format!("{FROZEN_POINT}def g(b: P) -> None:\n    b.x = 2.0\n");
    let diagnostics = run(&source).expect("checker ran");
    assert_rule_count(
        &diagnostics,
        "dataclasses_frozen",
        1,
        "assigning to an attribute of a frozen dataclass received as a parameter is \
         an error; the instance's type comes from its annotation, not from a \
         constructor call at the assignment site",
    );
}

// ──────────────────────────── calls_argument_type ─────────────────────────
//
// `Union[int, float]` and `int | float` are the same type (PEP 604). A `str`
// argument is incompatible with both.

/// Measured: the `int | float` spelling reports `calls_argument_type`; the
/// `Union[int, float]` spelling is silent.
#[test]
fn union_spelling_does_not_change_argument_compatibility() {
    assert_same_verdict(
        "`Union[int, float]` and `int | float` are the same type",
        "def f(a: int | float) -> None: ...\n\n\nf(\"s\")\n",
        "from typing import Union\n\n\ndef f(a: Union[int, float]) -> None: ...\n\n\nf(\"s\")\n",
    );
}

// ─────────────────────────── overloads_consistency ────────────────────────
//
// Overload variants are distinguished by their parameter TYPES. Two variants
// taking `int` and `str` are a correct, complete overload set.

/// Measured: reports `overloads_consistency` ("same parameter signature") on a
/// textbook-correct overload set. The comparison reads a resolver field that
/// is always `None`, so every same-arity pair compares equal.
#[test]
fn overloads_differing_only_in_parameter_type_are_distinct() {
    let source = "from typing import overload\n\n\n\
                  @overload\ndef sound(depth: int) -> int: ...\n\
                  @overload\ndef sound(depth: str) -> str: ...\n\
                  def sound(depth: int | str) -> int | str:\n    return depth\n";
    let diagnostics = run(source).expect("checker ran");
    assert_rule_count(
        &diagnostics,
        "overloads_consistency",
        0,
        "`sound(int)` and `sound(str)` have different parameter types and are \
         therefore distinct overloads; a correct overload set must not be reported \
         as overlapping",
    );
}

// ─────────────────────────── match_exhaustiveness ─────────────────────────
//
// A `match` over an enum that names every member is exhaustive. The typing
// spec does not require a wildcard branch in that case.

/// Measured: reports `match_exhaustiveness` even though both members of the
/// enum are covered.
#[test]
fn match_covering_every_enum_member_is_exhaustive() {
    let source = "from enum import Enum\n\n\n\
                  class Watch(Enum):\n    MORNING = 1\n    DOG = 2\n\n\n\
                  def bell(w: Watch) -> int:\n    match w:\n\
                  \x20       case Watch.MORNING:\n            return 1\n\
                  \x20       case Watch.DOG:\n            return 2\n";
    let diagnostics = run(source).expect("checker ran");
    assert_rule_count(
        &diagnostics,
        "match_exhaustiveness",
        0,
        "every member of `Watch` is matched, so the statement is exhaustive; \
         requiring a wildcard branch here is not a typing-spec obligation",
    );
}

// ───────────────────────────── typeddicts_usage ───────────────────────────
//
// PEP 589 forbids a TypedDict in `isinstance()`. Which callable `isinstance`
// is must come from the binding, not from the callee's spelling.

/// Measured: fires on a bare `isinstance(...)` call, silent when the very same
/// builtin is reached through a local alias.
#[test]
fn typeddict_isinstance_is_rejected_through_an_aliased_builtin() {
    let source = "from typing import TypedDict\n\n\
                  _check = isinstance\n\n\n\
                  class Ledger(TypedDict):\n    balance: int\n\n\n\
                  def audit(entry: object) -> None:\n\
                  \x20   if _check(entry, Ledger):\n        pass\n";
    let diagnostics = run(source).expect("checker ran");
    assert_rule_count(
        &diagnostics,
        "typeddicts_usage",
        1,
        "`_check` is bound to the builtin `isinstance`, so this is an `isinstance` \
         test against a TypedDict and PEP 589 rejects it however the callee is spelt",
    );
}

// ───────────────────────────── tuples_type_form ───────────────────────────
//
// `typing.Tuple[...]` is the same type form as `tuple[...]`. Two unbounded
// components are invalid in either spelling.

/// Measured: fires on `tuple[int, ..., str, ...]`, silent on the identical
/// `typing.Tuple[int, ..., str, ...]`.
#[test]
fn typing_tuple_alias_is_the_same_type_form_as_builtin_tuple() {
    assert_same_verdict(
        "`typing.Tuple[...]` and `tuple[...]` are the same type form",
        "def stow(cargo: tuple[int, ..., str, ...]) -> None: ...\n",
        "import typing as t\n\n\ndef stow(cargo: t.Tuple[int, ..., str, ...]) -> None: ...\n",
    );
}

// ───────────────────────── shadowed builtin spellings ─────────────────────
//
// Python lets any name be rebound, builtins included. A locally-defined class
// named `tuple` is NOT the builtin, and a rule keyed to the spelling will fire
// on it anyway.

/// Measured: `tuples_type_form` fires against a locally-defined `tuple`.
#[test]
fn locally_defined_class_named_tuple_is_not_the_builtin_tuple() {
    let source = "class tuple:\n    def __class_getitem__(cls, item: object) -> object:\n\
                  \x20       return item\n\n\n\
                  def stow(cargo: tuple[int, ..., str, ...]) -> None: ...\n";
    let diagnostics = run(source).expect("checker ran");
    assert_rule_count(
        &diagnostics,
        "tuples_type_form",
        0,
        "`tuple` here resolves to the locally-defined class, not the builtin tuple \
         type form, so the unbounded-component rule does not apply to it",
    );
}
