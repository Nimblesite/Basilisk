// Integration tests for BSK-E0071: Historical positional-only syntax.

use super::common::*;

#[test]
fn e0071_positional_only_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(x: int, /, y: int) -> int:
    return x + y

func(1, y=2)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0071_keyword_for_positional_only() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(x: int, /) -> int:
    return x

func(x=1)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
