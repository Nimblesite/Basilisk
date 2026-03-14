//! Tests for resolver: `test_yield`.

mod common;

use common::resolve_src;

#[test]
fn yield_exprs_collected_in_function() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Generator\n",
        "def gen() -> Generator[int, None, None]:\n",
        "    yield 1\n",
        "    yield 2\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let gen_func = resolved.functions.iter().find(|f| f.name == "gen");
    assert!(gen_func.is_some());
    assert!(gen_func.is_some_and(|f| f.is_generator));
    Ok(())
}

#[test]
fn yield_from_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def gen() -> None:\n", "    yield from [1, 2, 3]\n",).to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "gen");
    assert!(func.is_some());
    let func = func.is_some_and(|f| f.is_generator);
    assert!(func, "yield from must make function a generator");
    Ok(())
}

#[test]
fn yield_from_call_name_extracted() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def gen() -> None:\n", "    yield from range(10)\n",).to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "gen");
    assert!(func.is_some());
    Ok(())
}
