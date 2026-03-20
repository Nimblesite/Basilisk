// Integration tests for BSK-E0086: Multiple `TypeVarTuple`.

use super::common::*;

#[test]
fn e0086_multiple_typevartuple_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic
Ts1 = TypeVarTuple("Ts1")
Ts2 = TypeVarTuple("Ts2")

class Bad(Generic[*Ts1, *Ts2]):
    pass
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0086_single_typevartuple_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic
Ts = TypeVarTuple("Ts")

class Good(Generic[*Ts]):
    pass
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0086"),
        "single TypeVarTuple should not fire E0086"
    );
    Ok(())
}
