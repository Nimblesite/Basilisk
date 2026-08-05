//! Tests for [`tuples_index`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for tuples_index: Tuple index out of bounds.

use super::common::*;

#[test]
fn valid_tuple_index() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t: tuple[int, str] = (1, "a")
x = t[0]
y = t[1]
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"tuples_index"),
        "valid tuple index should not fire E0103"
    );
    Ok(())
}

#[test]
fn positive_out_of_bounds() -> Result<(), Box<dyn std::error::Error>> {
    // Module-level miss found by the torture corpus (tuple_index.py, GitHub
    // #284 family): the spec's tuples chapter requires an error for an
    // out-of-range literal index on a fixed-length tuple, at every scope.
    let source = r#"
t: tuple[int, str, bool] = (1, "a", True)
x = t[3]
"#;
    let diags = run(source)?;
    let hits: Vec<&str> = messages_for(&diags, "tuples_index");
    assert_eq!(
        hits.len(),
        1,
        "module-level `t[3]` on a 3-tuple must fire exactly once, got: {hits:?}"
    );
    assert!(
        hits[0].contains("index 3") && hits[0].contains("length 3"),
        "diagnostic must name index 3 and tuple length 3: {}",
        hits[0]
    );
    Ok(())
}

#[test]
fn negative_out_of_bounds() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t: tuple[int, str, bool] = (1, "a", True)
x = t[-4]
"#;
    let diags = run(source)?;
    let hits: Vec<&str> = messages_for(&diags, "tuples_index");
    assert_eq!(
        hits.len(),
        1,
        "module-level `t[-4]` on a 3-tuple must fire exactly once, got: {hits:?}"
    );
    Ok(())
}

#[test]
fn valid_negative_index() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t: tuple[int, str, bool] = (1, "a", True)
x = t[-1]
y = t[-2]
z = t[-3]
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"tuples_index"),
        "valid negative indices should not fire E0103"
    );
    Ok(())
}

#[test]
fn single_element_tuple() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
t: tuple[int] = (42,)
x = t[0]
y = t[1]
z = t[-2]
"#;
    let diags = run(source)?;
    let hits: Vec<&str> = messages_for(&diags, "tuples_index");
    assert_eq!(
        hits.len(),
        2,
        "`t[1]` and `t[-2]` are out of range for a 1-tuple; `t[0]` is not: {hits:?}"
    );
    Ok(())
}

// Follow-up to GitHub #284: an out-of-range index on a `key=` lambda's
// parameter was never actually checked — the old textual scan only ever
// matched it by accident (against the wrong function's annotation). The
// lambda parameter's tuple length comes from the iterable argument.
#[test]
fn key_lambda_index_out_of_range_annotated_iterable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def f() -> None:
    items: list[tuple[str, int, int]] = [("a", 1, 2), ("b", 3, 4)]
    for taxon, num_missing, total in sorted(
        items, key=lambda pair: (pair[1], pair[2], pair[0], pair[4])
    ):
        print(taxon, num_missing, total)
"#;
    let diags = run(source)?;
    let hits: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "tuples_index")
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "exactly the `pair[4]` subscript must fire (valid 0..2 and index 1/2/0 must not): {hits:?}"
    );
    assert!(
        hits[0].message.contains("index 4") && hits[0].message.contains("length 3"),
        "diagnostic must name index 4 and tuple length 3: {}",
        hits[0].message
    );
    Ok(())
}

#[test]
fn key_lambda_index_out_of_range_inferred_iterable() -> Result<(), Box<dyn std::error::Error>> {
    // No annotation: the 3-tuple length is inferred from the list literal.
    let source = r#"
def f() -> None:
    items = [("a", 1, 2), ("b", 3, 4)]
    print(sorted(items, key=lambda pair: pair[3]))
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"tuples_index"),
        "pair[3] on an inferred list of 3-tuples must fire"
    );
    Ok(())
}

#[test]
fn key_lambda_valid_and_unknown_indices_not_flagged() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def f() -> None:
    items = [("a", 1, 2), ("b", 3, 4)]
    mixed = [("a", 1), ("b", 3, 4)]
    opaque = load()
    print(sorted(items, key=lambda pair: (pair[2], pair[-3])))
    print(sorted(mixed, key=lambda pair: pair[5]))
    print(sorted(opaque, key=lambda pair: pair[9]))

def load() -> list:
    return []
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"tuples_index"),
        "valid indices, mixed-length lists, and unknown iterables must not fire: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "tuples_index")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn local_annotated_tuple_out_of_range_fires() -> Result<(), Box<dyn std::error::Error>> {
    // The direct-subscript miss is scope-wide: an annotated LOCAL is not a
    // parameter (tuples_index_2's territory), so it was never checked either.
    let source = r#"
def f() -> None:
    two: tuple[int, str] = (1, "a")
    bad = two[2]
"#;
    let diags = run(source)?;
    let hits: Vec<&str> = messages_for(&diags, "tuples_index");
    assert_eq!(
        hits.len(),
        1,
        "`two[2]` on an annotated local 2-tuple must fire exactly once, got: {hits:?}"
    );
    Ok(())
}

#[test]
fn variadic_and_shadowed_tuples_stay_clean() -> Result<(), Box<dyn std::error::Error>> {
    // `tuple[int, ...]` has no fixed length — any literal index is in range.
    let source = r#"
t: tuple[int, ...] = (1, 2, 3)
x = t[5]
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"tuples_index"),
        "variadic tuples must never fire: {:?}",
        messages_for(&diags, "tuples_index")
    );

    // A function-local rebinding shadows the module annotation — the local
    // `two` is a different, unannotated variable (the exact #284 bleed shape).
    let source = r#"
two: tuple[int, str] = (1, "a")


def f() -> int:
    two = (1, 2, 3)
    return two[2]
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"tuples_index"),
        "a shadowing local rebind must not be checked against the module annotation: {:?}",
        messages_for(&diags, "tuples_index")
    );
    Ok(())
}
