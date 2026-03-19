// Integration tests for BSK-E0094: Self type in invalid location.

use super::common::*;

#[test]
fn e0094_self_in_method_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

class Foo:
    def clone(self) -> Self:
        return self
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0094"),
        "Self in method return should not fire E0094"
    );
    Ok(())
}

#[test]
fn e0094_self_outside_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

def standalone() -> Self:
    pass
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
