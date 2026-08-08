//! Tests for [`tuples_index_2`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
//!
//! Regression coverage for [#284](https://github.com/Nimblesite/Basilisk/issues/284),
//! grounded in [PEP 484 tuple types](https://peps.python.org/pep-0484/#the-typing-module)
//! and [PEP 585 builtin generics](https://peps.python.org/pep-0585/). Import aliases
//! denote the same builtin generic and therefore must produce the same verdict.

use super::common::*;

#[test]
fn valid_tuple_index() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(v: tuple[int, str, float]) -> None:
    x = v[0]
    y = v[1]
    z = v[2]
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"tuples_index_2"),
        "valid tuple indices should not fire E0127"
    );
    Ok(())
}

#[test]
fn out_of_range_index() -> Result<(), Box<dyn std::error::Error>> {
    let variants = [
        r"
def f(v: tuple[int, str, float]) -> None:
    valid = v[2]
    x = v[4]
",
        r"
def inspect(
    sediment: tuple[
        int,
        str,
        float,
    ],
) -> None:
    valid = sediment[2]
    selected = sediment[
        4
    ]
",
        r"
import builtins as runtime_types

def inspect(
    sediment: runtime_types.tuple[
        runtime_types.int,
        runtime_types.str,
        runtime_types.float,
    ],
) -> None:
    valid = sediment[2]
    selected = sediment[4]
",
        r"
from builtins import float as DecimalValue
from builtins import int as WholeValue
from builtins import str as TextValue
from builtins import tuple as FixedSequence

def inspect(sediment: FixedSequence[WholeValue, TextValue, DecimalValue]) -> None:
    valid = sediment[2]
    selected = sediment[4]
",
    ];
    for source in variants {
        let diags = run(source)?;
        assert_rule_count(
            &diags,
            "tuples_index_2",
            1,
            "renaming, reformatting, and resolved aliases cannot change the AST tuple-index verdict",
        );
        let messages = messages_for(&diags, "tuples_index_2");
        assert!(
            messages
                .iter()
                .all(|message| message.contains('4') && message.contains('3')),
            "the sole diagnostic must describe index 4 against a three-element tuple: {messages:?}"
        );
    }
    Ok(())
}

#[test]
fn negative_out_of_range() -> Result<(), Box<dyn std::error::Error>> {
    let variants = [
        r"
def f(v: tuple[int, str, float]) -> None:
    valid = v[-3]
    x = v[-4]
",
        r"
def inspect(
    sediment: tuple[int, str, float],
) -> None:
    valid = sediment[-3]
    selected = sediment[
        -(
            4
        )
    ]
",
        r"
import builtins as runtime_types

def inspect(
    sediment: runtime_types.tuple[
        runtime_types.int,
        runtime_types.str,
        runtime_types.float,
    ],
) -> None:
    valid = sediment[-3]
    selected = sediment[-4]
",
        r"
from builtins import float as DecimalValue
from builtins import int as WholeValue
from builtins import str as TextValue
from builtins import tuple as FixedSequence

def inspect(sediment: FixedSequence[WholeValue, TextValue, DecimalValue]) -> None:
    valid = sediment[-3]
    selected = sediment[-(4)]
",
    ];
    for source in variants {
        let diags = run(source)?;
        assert_rule_count(
            &diags,
            "tuples_index_2",
            1,
            "renaming, aliasing, and parenthesized unary syntax cannot change the AST tuple-index verdict",
        );
        let messages = messages_for(&diags, "tuples_index_2");
        assert!(
            messages
                .iter()
                .all(|message| message.contains("-4") && message.contains('3')),
            "the sole diagnostic must describe index -4 against a three-element tuple: {messages:?}"
        );
    }
    Ok(())
}

// Regression for https://github.com/Nimblesite/Basilisk/issues/284: the body scan for a tuple-annotated parameter
// ran from the function's `def` to the next column-0 line — inside a class
// every method is indented, so the scan bled into LATER methods. A 2-tuple
// parameter named `pair` in one method's nested function flagged `pair[2]`
// on an unrelated lambda parameter in a different method.
#[test]
fn tuple_param_scan_does_not_bleed_into_later_methods() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Shell:
    def with_tuple_param(self) -> None:
        def sort_key(pair: tuple[str, int]) -> int:
            return pair[1]

        print(sort_key(('a', 1)))

    def by_num_missing(self) -> None:
        items = [('a', 1, 2), ('b', 3, 4)]
        for taxon, num_missing, total in sorted(
            items, key=lambda pair: (pair[1], pair[2], pair[0])
        ):
            print(taxon, num_missing, total)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"tuples_index_2"),
        "indexing a 3-tuple lambda param in a later method must not be checked \
         against another function's 2-tuple parameter: {diags:?}"
    );
    Ok(())
}
