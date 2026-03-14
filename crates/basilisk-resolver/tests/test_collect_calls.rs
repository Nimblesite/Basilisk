//! Tests for resolver: `test_collect_calls`.

mod common;

use common::resolve_src;

#[test]
fn calls_collected_from_for_loop() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    pass\n",
        "for i in range(3):\n",
        "    foo()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.calls.is_empty());
    Ok(())
}

#[test]
fn calls_collected_from_try_except() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    pass\n",
        "try:\n",
        "    foo()\n",
        "except:\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.calls.is_empty());
    Ok(())
}

#[test]
fn calls_collected_from_while_loop() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None: ...\n",
        "while True:\n",
        "    foo()\n",
        "    break\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.calls.iter().any(|c| c.callee == "foo"));
    Ok(())
}

#[test]
fn calls_collected_from_with_block() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None: ...\n",
        "with open('f') as fh:\n",
        "    foo()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.calls.iter().any(|c| c.callee == "foo"));
    Ok(())
}
