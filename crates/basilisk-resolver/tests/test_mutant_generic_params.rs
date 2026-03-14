//! Tests for resolver: `test_mutant_generic_params`.

mod common;

use common::resolve_src;

#[test]
fn extract_generic_params_collects_multiple_params() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypeVar, Generic\n",
        "T = TypeVar('T')\n",
        "S = TypeVar('S')\n",
        "class Pair(Generic[T, S]):\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved
        .classes
        .iter()
        .find(|c| c.name == "Pair")
        .ok_or("Pair not found")?;
    assert_eq!(
        cls.generic_params.len(),
        2,
        "Generic[T, S] must produce 2 params"
    );
    Ok(())
}

#[test]
fn extract_generic_params_non_generic_subscript_ignored() -> Result<(), Box<dyn std::error::Error>>
{
    let src = concat!(
        "from typing import TypeVar\n",
        "T = TypeVar('T')\n",
        "class Wrapper(list[T]):\n", // list[T] is not Generic[T]
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved
        .classes
        .iter()
        .find(|c| c.name == "Wrapper")
        .ok_or("Wrapper not found")?;
    // list[T] is a subscript but NOT Generic[...] — no params should be extracted
    assert_eq!(
        cls.generic_params.len(),
        0,
        "non-Generic subscript must not produce generic_params"
    );
    Ok(())
}

#[test]
fn extract_generic_params_single_param() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypeVar, Generic\n",
        "T = TypeVar('T')\n",
        "class Box(Generic[T]):\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved
        .classes
        .iter()
        .find(|c| c.name == "Box")
        .ok_or("Box not found")?;
    assert_eq!(
        cls.generic_params.len(),
        1,
        "Generic[T] must produce 1 param"
    );
    assert_eq!(cls.generic_params[0].name, "T");
    Ok(())
}
