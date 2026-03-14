#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Tests for resolver: `test_self_assigns`.

mod common;

use common::resolve_src;

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
