// Integration tests for BSK-E0085: `TypeVarTuple` arg count.

use super::common::*;

#[test]
fn e0085_arg_count_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic
Ts = TypeVarTuple("Ts")

class Tensor(Generic[*Ts]):
    pass

def takes_3d(t: Tensor[int, int, int]) -> None:
    pass

x: Tensor[int, int] = Tensor()
takes_3d(x)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
