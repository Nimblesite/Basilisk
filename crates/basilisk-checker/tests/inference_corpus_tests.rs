//! Tests for the [NARROWPLAN-CHECKLIST] Stage 2 item "Build the curated
//! container/comprehension/lambda benchmark and the targeted higher-order
//! (`map`/`filter`/decorators/`ParamSpec`) benchmark". See
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md.
//!
//! Each case is a tiny module whose LAST definition's inferred type
//! (through the definition-level Salsa queries — the same engine everything
//! else consumes) must be EQUIVALENT to the expected annotation (mutual
//! assignability, so `list[Literal[1]]` vs `list[int]` counts as a miss —
//! precision is the whole game).
//!
//! The suite is a RATCHET, not an all-pass gate: `PRECISION_FLOOR` is the
//! self-measured number of currently-passing cases; it may only go UP.
//! Higher-order cases the engine cannot answer yet (generic `map`/`filter`,
//! decorators, `ParamSpec`) are in the corpus on purpose — they document the
//! gap the generic-constraints work closes, and the floor rises as it does.
#![expect(
    clippy::expect_used,
    reason = "test-only lookups over fixed corpus fixtures"
)]

use basilisk_checker::incremental_defs::{definition_type, definitions};
use basilisk_checker::types::InferredType;
use basilisk_db::SourceFile;
use basilisk_test_utils::EventDb;

/// Cases that currently pass may never regress; new engine strength moves
/// this floor UP (self-measured over `CORPUS`).
const PRECISION_FLOOR: usize = 18;

/// `(case name, module source, expected annotation for the LAST definition)`.
const CORPUS: &[(&str, &str, &str)] = &[
    // ── containers ────────────────────────────────────────────────────
    (
        "list_literal",
        "x = [1, 2]\n",
        "list[Literal[1] | Literal[2]]",
    ),
    (
        "nested_list",
        "x = [[1], [2]]\n",
        "list[list[Literal[1] | Literal[2]]]",
    ),
    (
        "dict_literal",
        "x = {\"a\": 1}\n",
        "dict[Literal[\"a\"], Literal[1]]",
    ),
    (
        "set_literal",
        "x = {1, 2}\n",
        "set[Literal[1] | Literal[2]]",
    ),
    (
        "tuple_literal",
        "x = (1, \"s\")\n",
        "tuple[Literal[1], Literal[\"s\"]]",
    ),
    ("empty_list", "x = []\n", "list[Never]"),
    (
        "mixed_list",
        "x = [1, \"s\"]\n",
        "list[Literal[1] | Literal[\"s\"]]",
    ),
    // ── comprehensions ────────────────────────────────────────────────
    (
        "list_comp",
        "x = [n for n in [1, 2]]\n",
        "list[Literal[1] | Literal[2]]",
    ),
    ("set_comp", "x = {n for n in [1]}\n", "set[Literal[1]]"),
    (
        "dict_comp",
        "x = {k: 1 for k in [\"a\"]}\n",
        "dict[Literal[\"a\"], Literal[1]]",
    ),
    ("str_call_comp", "x = [str(n) for n in [1]]\n", "list[str]"),
    // ── calls, constructors, attributes, subscripts ───────────────────
    ("builtin_len", "x = len([1])\n", "int"),
    (
        "sibling_return",
        "def f() -> str:\n    return \"s\"\n\nx = f()\n",
        "str",
    ),
    (
        "synthesized_return",
        "def f():\n    return 42\n\nx = f()\n",
        "Literal[42]",
    ),
    (
        "constructor_attr",
        "class P:\n    y: float\n\nx = P().y\n",
        "float",
    ),
    (
        "subscript_list",
        "xs = [1, 2]\nx = xs[0]\n",
        "Literal[1] | Literal[2]",
    ),
    ("method_split", "x = \"a b\".split()\n", "list[str]"),
    (
        "ternary",
        "x = 1 if c else \"s\"\n",
        "Literal[1] | Literal[\"s\"]",
    ),
    ("binop_int", "x = 1 + 2\n", "int"),
    ("division", "x = 1 / 2\n", "float"),
    // ── lambdas ───────────────────────────────────────────────────────
    // Unconstrained lambda: parameters and body stay unknown-gradual.
    ("lambda_id", "x = lambda v: v\n", "Callable[..., Any]"),
    // ── higher-order gap (documents what generics work must close) ────
    ("map_builtin", "x = list(map(str, [1]))\n", "list[str]"),
    (
        "filter_builtin",
        "x = list(filter(None, [1, 2]))\n",
        "list[int]",
    ),
    (
        "decorator_identity",
        "def dec(f):\n    return f\n\n@dec\ndef g() -> int:\n    return 1\n\nx = g()\n",
        "int",
    ),
];

/// Equivalence up to the lattice: mutual assignability — PLUS a gradual
/// honesty check. `Unknown` is bidirectionally assignable, so without this a
/// `list[Unknown]` answer would "match" `list[str]` and inflate the score;
/// the actual type may carry no more gradual holes than the expectation
/// licenses (`Any`/`Unknown` in the expected text).
fn equivalent(actual: &InferredType, expected: &InferredType) -> bool {
    actual.is_assignable_to(expected)
        && expected.is_assignable_to(actual)
        && gradual_holes(actual) <= gradual_holes(expected)
}

/// Count `Any`/`Unknown` leaves, recursively.
fn gradual_holes(ty: &InferredType) -> usize {
    match ty {
        InferredType::Any | InferredType::Unknown => 1,
        InferredType::List(inner)
        | InferredType::Set(inner)
        | InferredType::Optional(inner)
        | InferredType::TypeForm(inner) => gradual_holes(inner),
        InferredType::Dict(key, value) => gradual_holes(key) + gradual_holes(value),
        InferredType::Tuple(items) | InferredType::Union(items) => {
            items.iter().map(gradual_holes).sum()
        }
        InferredType::Callable(info) => {
            info.param_types.iter().map(gradual_holes).sum::<usize>()
                + gradual_holes(&info.return_type)
        }
        _ => 0,
    }
}

/// [NARROWPLAN-CHECKLIST]: the curated inference-precision corpus ratchet.
#[test]
fn inference_precision_corpus_holds_the_floor() {
    let db = EventDb::default();
    let mut passed = Vec::new();
    let mut missed = Vec::new();
    for (name, source, expected_text) in CORPUS {
        let file = SourceFile::new(&db, format!("{name}.py"), (*source).to_owned());
        let defs = definitions(&db, file);
        let last = defs.last().copied().expect("corpus case defines a name");
        let actual = definition_type(&db, last);
        let expected = InferredType::from_annotation(expected_text);
        if equivalent(&actual, &expected) {
            passed.push(*name);
        } else {
            missed.push(format!("{name}: expected {expected:?}, got {actual:?}"));
        }
    }
    println!("inference corpus: {} / {} pass", passed.len(), CORPUS.len());
    println!("missed: {missed:#?}");
    assert!(
        passed.len() >= PRECISION_FLOOR,
        "precision ratchet violated: {} passing < floor {PRECISION_FLOOR}; missed: {missed:#?}",
        passed.len()
    );
}
