#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Tests for resolver: `test_pep695`.

mod common;

use common::resolve_src;

#[test]
fn pep695_list_bound_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = "class Foo[T: [str, int]]:\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.pep695_bound_violations.is_empty(),
        "list literal as PEP 695 bound must produce a violation"
    );
    Ok(())
}

#[test]
fn pep695_empty_tuple_constraint_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = "class Foo[T: ()]:\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.pep695_bound_violations.is_empty(),
        "empty tuple constraint must produce a violation"
    );
    Ok(())
}

#[test]
fn pep695_single_element_tuple_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = "class Foo[T: (str,)]:\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.pep695_bound_violations.is_empty(),
        "single-element constraint tuple must produce a violation"
    );
    Ok(())
}

#[test]
fn pep695_valid_bound_no_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = "class Foo[T: str]:\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.pep695_bound_violations.is_empty(),
        "valid PEP 695 bound must not produce a violation"
    );
    Ok(())
}

#[test]
fn pep695_valid_constraint_tuple_no_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = "class Foo[T: (str, bytes)]:\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.pep695_bound_violations.is_empty(),
        "valid constraint tuple must not produce a violation"
    );
    Ok(())
}

#[test]
fn pep695_invalid_constraint_element() -> Result<(), Box<dyn std::error::Error>> {
    let src = "class Foo[T: (3, bytes)]:\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.pep695_bound_violations.is_empty(),
        "integer literal in constraint tuple must produce a violation"
    );
    Ok(())
}

#[test]
fn pep695_variable_constraint() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "bounds = (str, bytes)\n",
        "class Foo[T: bounds]:\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.pep695_bound_violations.is_empty(),
        "variable as constraint must produce a violation"
    );
    Ok(())
}

#[test]
fn pep695_bound_violation_in_function() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("class Inner[T: [str, int]]:\n", "    pass\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.pep695_bound_violations.is_empty());
    Ok(())
}

#[test]
fn pep695_empty_tuple_bound() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("class Foo[T: ()]:\n", "    pass\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.pep695_bound_violations.is_empty());
    Ok(())
}

#[test]
fn pep695_single_element_tuple_bound() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("class Foo[T: (str,)]:\n", "    pass\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.pep695_bound_violations.is_empty());
    Ok(())
}

#[test]
fn pep695_valid_two_element_constraint() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("class Foo[T: (str, int)]:\n", "    pass\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.pep695_bound_violations.is_empty());
    Ok(())
}

#[test]
fn pep695_non_literal_constraint_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("MyType = int\n", "class Foo[T: MyType]:\n", "    pass\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.pep695_bound_violations.is_empty());
    Ok(())
}

#[test]
fn pep695_outer_typevar_nested_class() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Outer[V]:\n",
        "    class Inner[T: V]:\n",
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.pep695_bound_violations.is_empty());
    Ok(())
}
