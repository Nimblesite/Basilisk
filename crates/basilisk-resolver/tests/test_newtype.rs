mod common;

use common::resolve_src;

#[test]
fn newtype_call_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import NewType\n",
        "UserId = NewType('UserId', int)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.newtype_calls.len(), 1);
    Ok(())
}

#[test]
fn newtype_call_int_base() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import NewType\n",
        "UserId = NewType('UserId', int)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.newtype_calls.is_empty());
    Ok(())
}
