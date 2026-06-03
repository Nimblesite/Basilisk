//! Tests for [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-typesafety
//! Coarse e2e tests locking in the BSK-E0014 false-positive eliminations from
//! `docs/plans/CHECK-ELIMINATE-FALSE-POSITIVES.md`.
//!
//! Each test asserts BOTH directions: the valid (`# OK`) forms are NOT flagged
//! (the eliminated false positive), and the genuinely-incompatible forms ARE
//! still flagged (the true positive that must keep firing). This pairing is what
//! kills mutants that would otherwise silently re-introduce a false positive or
//! drop a real diagnostic.
#![allow(
    clippy::allow_attributes,
    clippy::uninlined_format_args,
    clippy::needless_raw_string_hashes,
    missing_docs,
    dead_code
)]

mod common;

use common::{has_code, run};

const E0014: &str = "BSK-E0014";

fn e0014_count(diags: &[common::Diagnostic]) -> usize {
    diags.iter().filter(|d| d.code.code == E0014).count()
}

// ── Recursive value aliases (e0014/alias_match.rs) ────────────────────────

#[test]
fn recursive_union_alias_accepts_valid_and_rejects_invalid(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Union

Json = Union[None, int, str, float, list["Json"], dict[str, "Json"]]

ok1: Json = [1, {"a": 1}]
ok2: Json = 3.4
ok3: Json = [1.2, None, [1.2, [""]]]
bad1: Json = {"a": 1, "b": 3j}
bad2: Json = [2, 3j]
"#;
    let diags = run(source)?;
    // Valid recursive-alias assignments must NOT be flagged.
    assert!(
        !has_code(&diags, E0014) || e0014_count(&diags) == 2,
        "expected exactly the two invalid Json assignments to be flagged, got {} E0014",
        e0014_count(&diags)
    );
    assert_eq!(
        e0014_count(&diags),
        2,
        "the two complex-number assignments (3j) must still fire E0014"
    );
    Ok(())
}

#[test]
fn recursive_tuple_and_mapping_aliases() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Mapping

RecursiveTuple = str | int | tuple["RecursiveTuple", ...]
RecursiveMapping = str | int | Mapping[str, "RecursiveMapping"]

t_ok1: RecursiveTuple = (1, 1)
t_ok2: RecursiveTuple = (1, "1", 1, "2")
t_bad: RecursiveTuple = (1, [1])

m_ok1: RecursiveMapping = {"1": "1", "2": 1}
m_ok2: RecursiveMapping = {"1": "1", "3": {}}
m_bad: RecursiveMapping = {"1": [1]}
"#;
    let diags = run(source)?;
    assert_eq!(
        e0014_count(&diags),
        2,
        "only the two list-bearing assignments must fire E0014, got {}",
        e0014_count(&diags)
    );
    Ok(())
}

// ── Variadic tuples (types.rs tuple_assignable_with_star) ──────────────────

#[test]
fn variadic_tuple_star_targets() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def f(a: tuple[str, str]):
    ok1: tuple[int, *tuple[str]] = (1, "")
    ok2: tuple[int, *tuple[str, ...]] = (1,)
    ok3: tuple[int, *tuple[str, ...], int] = (1, 2)
    ok4: tuple[str, str, *tuple[int, ...]] = a
    ok5: tuple[*tuple[str, ...], str] = a
    bad1: tuple[*tuple[str, ...], str, str, str] = a
"#;
    let diags = run(source)?;
    assert_eq!(
        e0014_count(&diags),
        1,
        "only the too-short `bad1` assignment must fire E0014, got {}",
        e0014_count(&diags)
    );
    Ok(())
}

#[test]
fn gradual_variadic_tuple_source_is_assignable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Any

def f(any_tuple: tuple[Any, ...], int_tuple: tuple[int, ...]):
    ok: tuple[int, int] = any_tuple
    bad: tuple[int, int] = int_tuple
"#;
    let diags = run(source)?;
    assert_eq!(
        e0014_count(&diags),
        1,
        "`tuple[Any, ...]` is gradual (OK); `tuple[int, ...]`→fixed must fire, got {}",
        e0014_count(&diags)
    );
    Ok(())
}

// ── Bare generics & special forms (types_parsing.rs / types.rs) ────────────

#[test]
fn bare_callable_and_type_are_any_like() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable, Any

def f(val1: Callable, v: type):
    t1: Callable[[], Any] = val1
    t2: Callable[[int, str], None] = val1
    x1: Callable[..., Any] = v
    x2: Callable[[int, int], int] = v
"#;
    let diags = run(source)?;
    assert_eq!(
        e0014_count(&diags),
        0,
        "bare Callable/type assignments must not be flagged, got {}",
        e0014_count(&diags)
    );
    Ok(())
}

#[test]
fn none_is_hashable_but_not_iterable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Hashable, Iterable

none1: Hashable = None
none2: Iterable = None
"#;
    let diags = run(source)?;
    assert_eq!(
        e0014_count(&diags),
        1,
        "None→Hashable is OK; None→Iterable must fire, got {}",
        e0014_count(&diags)
    );
    Ok(())
}

#[test]
fn empty_tuple_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t10: tuple[()] = ()
"#;
    let diags = run(source)?;
    assert!(
        !has_code(&diags, E0014),
        "`tuple[()] = ()` must not be flagged"
    );
    Ok(())
}

#[test]
fn non_decimal_literal_equivalence() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

def f(c: Literal[0b10100]):
    ok: Literal[0x14] = c
    bad: Literal[3] = 4
"#;
    let diags = run(source)?;
    assert_eq!(
        e0014_count(&diags),
        1,
        "0x14 == 0b10100 (both 20) is OK; Literal[3]=4 must fire, got {}",
        e0014_count(&diags)
    );
    Ok(())
}

#[test]
fn quoted_forward_ref_annotation_is_skipped() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal
from enum import Enum

class Color(Enum):
    RED = 1

def f(a: Literal[Color.RED]):
    x2: "Literal[Color.RED]" = a
"#;
    let diags = run(source)?;
    assert!(
        !has_code(&diags, E0014),
        "whole-quoted annotations must be skipped by E0014"
    );
    Ok(())
}
