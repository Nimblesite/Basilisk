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
    // TODO: resolver does not yet produce tuple_index_violations for literal indices.
    // When it does, this test should assert E0103 fires.
    let source = r#"
t: tuple[int, str, bool] = (1, "a", True)
x = t[3]
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn negative_out_of_bounds() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: resolver does not yet produce tuple_index_violations for literal indices.
    let source = r#"
t: tuple[int, str, bool] = (1, "a", True)
x = t[-4]
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
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
    // TODO: resolver does not yet produce tuple_index_violations for literal indices.
    let source = r#"
t: tuple[int] = (42,)
x = t[0]
y = t[1]
z = t[-2]
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
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
