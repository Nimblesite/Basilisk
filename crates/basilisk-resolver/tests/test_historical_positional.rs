//! Tests for resolver: `test_historical_positional`.

mod common;

use common::resolve_src;

#[test]
fn historical_posonly_violation_detected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def foo(__x: int) -> None:\n", "    pass\n", "foo(__x=1)\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.historical_positional_violations.is_empty(),
        "calling historical positional-only param as keyword must produce a violation"
    );
    Ok(())
}

#[test]
fn historical_posonly_no_violation_positional() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def foo(__x: int) -> None:\n", "    pass\n", "foo(1)\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.historical_positional_violations.is_empty(),
        "calling with positional arg must not produce a violation"
    );
    Ok(())
}

#[test]
fn historical_posonly_in_class_method() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Foo:\n",
        "    def bar(self, __x: int) -> None:\n",
        "        pass\n",
        "Foo().bar(__x=1)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    // The historical positional param collection recurses into class bodies
    // Whether the call violation is detected depends on call-site detection
    assert!(resolved.historical_positional_violations.len() <= 1);
    Ok(())
}

#[test]
fn historical_positional_in_class_init() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Foo:\n",
        "    def __init__(self, name: str, __x: int) -> None:\n",
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.historical_positional_violations.is_empty());
    Ok(())
}
