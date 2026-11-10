//! Tests for [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
// Tests for resolver: `test_decorators`.

use super::common::resolve_src;

#[test]
fn collects_decorator_with_call_on_attribute() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "import functools\n",
        "class Foo:\n",
        "    @functools.lru_cache(maxsize=128)\n",
        "    def bar(self: 'Foo') -> int:\n",
        "        return 0\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let bar = resolved.functions.iter().find(|f| f.name == "bar");
    assert!(bar.is_some(), "bar method must be resolved");
    let bar = bar.ok_or("bar not found")?;
    assert!(!bar.decorators.is_empty());
    Ok(())
}

#[test]
fn collects_decorator_with_plain_name() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import overload\n@overload\ndef foo(x: int) -> int: ...\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.functions[0]
        .decorators
        .contains(&"overload".to_string()));
    Ok(())
}
