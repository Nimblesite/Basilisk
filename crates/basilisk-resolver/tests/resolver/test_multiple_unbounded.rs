//! Tests for [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
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

// ---------------------------------------------------------------------------
// The `tuple` head is RESOLVED, not spelled
//
// Pins the 2026-08-09 review finding: `is_unbounded_component` and
// `annotation_has_multiple_unbounded` recognised an unpacked unbounded tuple
// with `expr_simple_name(...) == "tuple"`, which grants builtin meaning to the
// final token of any expression.
// ---------------------------------------------------------------------------

#[test]
fn multiple_unbounded_is_flagged_through_an_aliased_import(
) -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from builtins import tuple as ordered_pair\n",
        "def f(x: ordered_pair[*ordered_pair[str, ...], *ordered_pair[int, ...]]) -> None:\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(
        resolved.multiple_unbounded_tuple_spans.len(),
        1,
        "`ordered_pair` IS `builtins.tuple`; renaming the import does not make \
         two unbounded unpacks legal"
    );
    Ok(())
}

#[test]
fn multiple_unbounded_is_flagged_through_the_typing_alias() -> Result<(), Box<dyn std::error::Error>>
{
    let src = concat!(
        "from typing import Tuple\n",
        "def f(x: Tuple[*Tuple[str, ...], *Tuple[int, ...]]) -> None:\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(
        resolved.multiple_unbounded_tuple_spans.len(),
        1,
        "PEP 585's `typing.Tuple` is the same class as `tuple`"
    );
    Ok(())
}

#[test]
fn multiple_unbounded_is_flagged_through_the_qualified_builtin(
) -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "import builtins\n",
        "def f(x: builtins.tuple[*builtins.tuple[str, ...], *builtins.tuple[int, ...]]) -> None:\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(
        resolved.multiple_unbounded_tuple_spans.len(),
        1,
        "`builtins.tuple` is the builtin, written the long way"
    );
    Ok(())
}

#[test]
fn a_user_class_named_tuple_is_not_the_builtin() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class tuple:\n",
        "    pass\n",
        "def f(x: tuple[..., int]) -> None:\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.multiple_unbounded_tuple_spans.is_empty(),
        "the module defines its own `tuple`; the builtin's variadic rules do \
         not apply to an unrelated class that borrowed its name"
    );
    Ok(())
}
