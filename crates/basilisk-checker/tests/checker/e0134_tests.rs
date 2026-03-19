// Integration tests for BSK-E0134: Invariant generic type mismatch.

use super::common::*;

#[test]
fn e0134_subclass_invariant_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Node: ...

class SymbolTable(dict[str, list[Node]]): ...

def takes(x: dict[str, list[object]]) -> None: ...

def test(s: SymbolTable) -> None:
    takes(s)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0134_valid_invariant_match() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class SymbolTable(dict[str, list[int]]): ...

def takes(x: dict[str, list[int]]) -> None: ...

def test(s: SymbolTable) -> None:
    takes(s)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0134"),
        "exact invariant match should not fire E0134"
    );
    Ok(())
}
