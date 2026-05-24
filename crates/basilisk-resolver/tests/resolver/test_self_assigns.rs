//! Tests for [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
// Tests for resolver: `test_self_assigns`.

use super::common::resolve_src;

#[test]
fn class_final_with_init_no_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Final\n",
        "class Foo:\n",
        "    x: Final[int]\n",
        "    def __init__(self) -> None:\n",
        "        self.x = 42\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.final_violations.is_empty(),
        "Final with __init__ assignment must not produce a violation"
    );
    Ok(())
}
