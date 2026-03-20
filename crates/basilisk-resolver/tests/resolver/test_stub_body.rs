// Tests for resolver: `test_stub_body`.

use super::common::resolve_src;

#[test]
fn ellipsis_body_is_stub() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import overload\n",
        "@overload\n",
        "def foo(x: int) -> int:\n",
        "    ...\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(func.is_some());
    assert!(
        func.is_some_and(|f| f.is_stub_body),
        "function body with only ... must be a stub"
    );
    Ok(())
}

#[test]
fn pass_body_not_stub() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def foo() -> None:\n", "    pass\n",).to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved.functions.iter().find(|f| f.name == "foo");
    assert!(func.is_some());
    assert!(
        !func.is_none_or(|f| f.is_stub_body),
        "function body with pass is not a stub"
    );
    Ok(())
}
