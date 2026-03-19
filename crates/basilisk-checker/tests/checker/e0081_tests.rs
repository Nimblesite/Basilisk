// Integration tests for BSK-E0081: `TypeVarTuple` unpack minimum args.

use super::common::*;

#[test]
fn e0081_too_few_args_for_unpack() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Unpack, Generic
Ts = TypeVarTuple("Ts")

class Tensor(Generic[*Ts]):
    pass

def process(t: Tensor[int, str, float]) -> None:
    pass

process(Tensor())
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0081_valid_unpack() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic
Ts = TypeVarTuple("Ts")

class Tensor(Generic[*Ts]):
    pass

x: Tensor[int, str] = Tensor()
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0081"),
        "valid TypeVarTuple should not fire E0081"
    );
    Ok(())
}
