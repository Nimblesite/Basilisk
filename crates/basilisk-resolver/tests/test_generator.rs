mod common;

use common::{resolve_src};

#[test]
fn generator_with_valid_return_type_no_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Generator\n",
        "def gen() -> Generator[int, None, None]:\n",
        "    yield 1\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.generator_violations.is_empty(),
        "Generator return type must not produce a violation"
    );
    Ok(())
}

#[test]
fn non_generator_func_no_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def regular() -> int:\n", "    return 42\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.generator_violations.is_empty());
    Ok(())
}

#[test]
fn generator_with_non_generator_return_type() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def gen() -> int:\n", "    yield 1\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.generator_violations.is_empty());
    Ok(())
}

#[test]
fn generator_with_valid_generator_return() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Generator\n",
        "def gen() -> Generator[int, None, None]:\n",
        "    yield 1\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.generator_violations.is_empty());
    Ok(())
}

#[test]
fn generator_with_iterator_return() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Iterator\n",
        "def gen() -> Iterator[int]:\n",
        "    yield 1\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.generator_violations.is_empty());
    Ok(())
}

#[test]
fn async_generator_with_wrong_return_type() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("async def agen() -> int:\n", "    yield 1\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.generator_violations.is_empty());
    Ok(())
}

#[test]
fn async_generator_with_valid_return() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import AsyncGenerator\n",
        "async def agen() -> AsyncGenerator[int, None]:\n",
        "    yield 1\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.generator_violations.is_empty());
    Ok(())
}

#[test]
fn generator_function_is_generator_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def gen() -> int:\n", "    yield 1\n",).to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "gen");
    assert!(func.is_some_and(|f| f.is_generator));
    Ok(())
}
