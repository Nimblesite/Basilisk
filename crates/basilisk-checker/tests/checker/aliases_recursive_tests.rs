//! Tests for [`aliases_recursive`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
//!
//! Recursive `type` aliases are specified by
//! [PEP 695](https://peps.python.org/pep-0695/#generic-type-alias).

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use super::common::*;

/// Ceiling for a full check that must be effectively instant. Generous so slow
/// CI machines never flake, yet far below the effectively-infinite hang it
/// guards against.
const CHECK_DEADLINE: Duration = Duration::from_secs(30);

/// Run the full checker on `source` in a worker thread and fail the test if it
/// does not finish within [`CHECK_DEADLINE`] — a hung checker must fail fast,
/// not stall the suite. Checker-level twin of the resolver's #398 harness
/// (`basilisk-resolver/tests/resolver/test_recursive_bases.rs`).
fn check_within_deadline(source: &'static str) {
    let (sender, receiver) = mpsc::channel();
    // The handle is deliberately dropped: a hung worker cannot be joined, and
    // the process exiting after the failed test reaps it.
    drop(thread::spawn(move || {
        // Stringify the error: `Box<dyn Error>` is not `Send`, so the raw
        // check result cannot cross the channel.
        let outcome = run(source).map(|_| ()).map_err(|e| e.to_string());
        drop(sender.send(outcome));
    }));
    let checked = receiver
        .recv_timeout(CHECK_DEADLINE)
        .unwrap_or_else(|_| panic!("checker hung on:\n{source}"));
    assert!(checked.is_ok(), "check failed: {:?}", checked.err());
}

/// The recursive-alias definitions from #371 and the genuinely cyclical
/// rejections must both complete promptly under the full checker: the alias
/// expander and the circularity walk are the same recursion shape the #398
/// class-bases hang came from, so every spelling gets the same wall-clock
/// bound (plan box: deadline-guard the hang-class regressions).
#[test]
fn recursive_alias_definitions_check_within_deadline() {
    check_within_deadline(
        "type JsonValue = str | int | float | bool | None | list[JsonValue] | dict[str, JsonValue]\n",
    );
    check_within_deadline("type RecursiveUnion = RecursiveUnion | int\n");
    check_within_deadline(
        "type MutualReference1 = MutualReference2 | int\n\
         type MutualReference2 = MutualReference1 | str\n",
    );
    check_within_deadline("class C(C[int], C[bool]):\n    pass\n");
}

#[test]
fn non_cyclical_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias

IntList: TypeAlias = list[int]
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"aliases_recursive"),
        "non-cyclical alias should not fire E0104"
    );
    Ok(())
}

/// PEP 695 `type`-statement counterparts of every recursive alias DEFINITION
/// in upstream `conformance/tests/aliases_recursive.py` — the upstream file
/// contains zero `type` statements, so this syntax gap survived a 100%
/// conformance score ([#371](https://github.com/Nimblesite/Basilisk/issues/371),
/// plan box in docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md Stage 0.5).
/// PEP 695 formally mandates that recursive aliases work: none of these may
/// draw a circularity diagnostic from any rule. Value-level assignability
/// through these aliases lands with [TYPEINF-ANNOTATION-RESOLUTION].
#[test]
fn upstream_recursive_definitions_as_type_statements_are_clean(
) -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        // Json / Json2 (the equivalent pair, upstream lines 14/24).
        "type Json = None | int | str | float | list[Json] | dict[str, Json]\n\
         type Json2 = None | int | str | float | list[Json2] | dict[str, Json2]\n",
        // RecursiveTuple (upstream line 30).
        "type RecursiveTuple = str | int | tuple[RecursiveTuple, ...]\n",
        // RecursiveMapping (upstream line 42) — a Named constructor guards.
        "from typing import Mapping\n\
         type RecursiveMapping = str | int | Mapping[str, RecursiveMapping]\n",
        // GenericTypeAlias1 + its specialization (upstream lines 58-59); the
        // old-style constrained TypeVar becomes a PEP 695 constrained param.
        "type GenericTypeAlias1[T1: (str, int)] = list[GenericTypeAlias1[T1] | T1]\n\
         type SpecializedTypeAlias1 = GenericTypeAlias1[str]\n",
        // GenericTypeAlias2 (upstream line 65).
        "type GenericTypeAlias2[T1: (str, int), T2] = list[GenericTypeAlias2[T1, T2] | T1 | T2]\n",
    ];
    for source in cases {
        let diags = run(source)?;
        for rule in ["aliases_recursive", "generics_syntax_scoping"] {
            assert!(
                !codes(&diags).contains(&rule),
                "recursive `type` alias definition must not fire {rule}.\n\
                 source:\n{source}\ngot: {:?}",
                messages_for(&diags, rule)
            );
        }
    }
    Ok(())
}

/// The upstream file's two `# E: cyclical reference` cases, as `type`
/// statements: a self-reference in a union arm never reaches a constructor
/// head, and a bare mutual pair is the same non-termination split across two
/// names. Both must still be rejected in the PEP 695 spelling — including
/// through the transparent `Union[..]` operator.
#[test]
fn upstream_cyclical_cases_as_type_statements_still_fire() -> Result<(), Box<dyn std::error::Error>>
{
    let cases = [
        // RecursiveUnion (upstream line 72), `|` and Union[..] spellings.
        "type RecursiveUnion = RecursiveUnion | int\n",
        "from typing import Union\ntype RecursiveUnion = Union[RecursiveUnion, int]\n",
        // MutualReference1 / MutualReference2 (upstream line 75).
        "type MutualReference1 = MutualReference2 | int\n\
         type MutualReference2 = MutualReference1 | str\n",
    ];
    for source in cases {
        let diags = run(source)?;
        assert!(
            codes(&diags).contains(&"generics_syntax_scoping"),
            "cyclical `type` alias must fire generics_syntax_scoping.\n\
             source:\n{source}\ngot: {:?}",
            codes(&diags)
        );
    }
    Ok(())
}

/// Regression for [#383](https://github.com/Nimblesite/Basilisk/issues/383).
/// [PEP 695](https://peps.python.org/pep-0695/#generic-type-alias) supports
/// recursive aliases and imposes no "same type arguments" restriction.
/// Basilisk may offer a separately named opt-in policy for non-regular
/// expansion, but the default PEP scoping rule must not invent that error.
#[test]
fn non_regular_recursive_alias_is_not_a_pep_scoping_error() -> Result<(), Box<dyn std::error::Error>>
{
    let source = "type R[T] = set[R[T | R[T]]]\n";
    let diagnostics = run(source)?;
    assert_rule_count(
        &diagnostics,
        "generics_syntax_scoping",
        0,
        "PEP 695 does not make non-regular recursion a type-parameter scoping violation",
    );
    Ok(())
}
