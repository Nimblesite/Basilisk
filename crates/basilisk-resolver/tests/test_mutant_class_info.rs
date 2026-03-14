mod common;

use common::resolve_src;

#[test]
fn body_is_stub_ellipsis_only_is_stub() -> Result<(), Box<dyn std::error::Error>> {
    // @overload functions with `...` bodies are stubs — E0001/E0002 must not fire.
    let src = concat!(
        "from typing import overload\n",
        "@overload\n",
        "def process(x: int) -> int: ...\n",
        "@overload\n",
        "def process(x: str) -> str: ...\n",
        "def process(x):\n",
        "    return x\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    // The overload stubs must have is_stub_body = true
    let overloads: Vec<_> = resolved
        .functions
        .iter()
        .filter(|f| f.decorators.iter().any(|d| d == "overload"))
        .collect();
    assert!(!overloads.is_empty(), "overloads must be resolved");
    for f in &overloads {
        assert!(f.is_stub_body, "overload with `...` body must be stub");
    }
    Ok(())
}

#[test]
fn body_is_stub_real_body_is_not_stub() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def process(x: int) -> int:\n    return x + 1\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    assert!(!func.is_stub_body, "real body must not be stub");
    Ok(())
}
