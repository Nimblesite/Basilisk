// Tests for resolver: `test_multiple_unbounded`.

use super::common::resolve_src;

#[test]
fn multiple_unbounded_tuple_starred_unpacks() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def f(x: tuple[*tuple[str, ...], *tuple[int, ...]]) -> None:\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.multiple_unbounded_tuple_spans.is_empty(),
        "two unbounded starred unpacks in tuple must produce a violation"
    );
    Ok(())
}

#[test]
fn tuple_bare_ellipsis_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def f(x: tuple[...]) -> None:\n", "    pass\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.multiple_unbounded_tuple_spans.is_empty(),
        "tuple[...] must produce a violation"
    );
    Ok(())
}

#[test]
fn tuple_valid_homogeneous_no_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def f(x: tuple[int, ...]) -> None:\n", "    pass\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.multiple_unbounded_tuple_spans.is_empty(),
        "tuple[int, ...] must not produce a violation"
    );
    Ok(())
}

#[test]
fn tuple_ellipsis_wrong_position() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def f(x: tuple[..., int]) -> None:\n", "    pass\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.multiple_unbounded_tuple_spans.is_empty(),
        "tuple[..., int] must produce a violation"
    );
    Ok(())
}

#[test]
fn tuple_multiple_non_ellipsis_before_ellipsis() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def f(x: tuple[int, str, ...]) -> None:\n", "    pass\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.multiple_unbounded_tuple_spans.is_empty(),
        "tuple[int, str, ...] must produce a violation"
    );
    Ok(())
}

#[test]
fn tuple_starred_before_ellipsis() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def f(x: tuple[*tuple[str], ...]) -> None:\n", "    pass\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.multiple_unbounded_tuple_spans.is_empty(),
        "tuple[*tuple[str], ...] must produce a violation"
    );
    Ok(())
}
