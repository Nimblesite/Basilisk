//! Tests for [typeddicts_extra_items] from [CHKARCH-DIAG-TYPEDDICT-EXTRA-ITEMS]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPEDDICT-EXTRA-ITEMS
// Integration tests for PEP 728 closed / extra_items subclass checks.

use super::common::*;

#[test]
fn e0156_closed_inherited_extra_key_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import TypedDict\nclass BaseMovie(TypedDict, closed=True):\n    name: str\nclass MovieA(BaseMovie):\n    pass\nclass MovieC(MovieA):\n    age: int\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"typeddicts_extra_items"),
        "adding a key under inherited closure must fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0156_closed_inherited_no_new_key_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import TypedDict\nclass BaseMovie(TypedDict, closed=True):\n    name: str\nclass MovieA(BaseMovie):\n    pass\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"typeddicts_extra_items"),
        "inheriting closure without adding a key must not fire"
    );
    Ok(())
}

#[test]
fn e0156_inherited_required_extra_item_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import TypedDict\nclass MovieBase2(TypedDict, extra_items=int | None):\n    name: str\nclass MovieRequiredYear(MovieBase2):\n    year: int | None\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"typeddicts_extra_items"),
        "a Required key under an inherited extra_items must fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0156_inherited_notrequired_inconsistent_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import TypedDict, NotRequired\nclass MovieBase2(TypedDict, extra_items=int | None):\n    name: str\nclass MovieNotRequiredYear(MovieBase2):\n    year: NotRequired[int]\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"typeddicts_extra_items"),
        "an item type inconsistent with extra_items must fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0156_inherited_extra_item_consistent_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import TypedDict, NotRequired\nclass MovieBase2(TypedDict, extra_items=int | None):\n    name: str\nclass MovieWithYear(MovieBase2):\n    year: NotRequired[int | None]\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"typeddicts_extra_items"),
        "a NotRequired item consistent with extra_items must not fire"
    );
    Ok(())
}

#[test]
fn e0156_readonly_extra_item_not_assignable_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import TypedDict, ReadOnly\nclass BookBase(TypedDict, extra_items=ReadOnly[int | None]):\n    name: str\nclass BookWithPublisher(BookBase):\n    publisher: str\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"typeddicts_extra_items"),
        "a key not assignable to a read-only extra_items must fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
