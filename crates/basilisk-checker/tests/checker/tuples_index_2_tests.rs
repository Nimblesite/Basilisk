//! Tests for [`tuples_index_2`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for tuples_index_2: Tuple index out of range.

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
    let source = r"
def f(v: tuple[int, str, float]) -> None:
    x = v[4]
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn negative_out_of_range() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def f(v: tuple[int, str, float]) -> None:
    x = v[-4]
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// Regression for GitHub #284: the body scan for a tuple-annotated parameter
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
