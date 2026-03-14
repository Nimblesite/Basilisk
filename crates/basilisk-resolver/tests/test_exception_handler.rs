#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Tests for resolver: `test_exception_handler`.

mod common;

use common::resolve_src;

#[test]
fn except_handler_functions_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "try:\n",
        "    pass\n",
        "except Exception:\n",
        "    def handler() -> None:\n",
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let names: Vec<&str> = resolved.functions.iter().map(|f| f.name.as_str()).collect();
    assert!(
        names.contains(&"handler"),
        "functions in except handlers must be collected"
    );
    Ok(())
}
