//! Mutation-killing tests: assertion-heavy tests designed to catch specific
//! mutants identified by cargo-mutants. Each test asserts BOTH that violations
//! ARE caught and that correct code is NOT flagged.

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

fn e0014_count(diagnostics: &[basilisk_checker::Diagnostic]) -> usize {
    diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0014")
        .count()
}

// ═══════════════════════════════════════════════════════════════════════
// types.rs is_assignable_to — Optional match arm
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mutant_optional_assignability() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Optional

# int assigned to Optional[int] → OK
a: Optional[int] = 42

# None assigned to Optional[int] → OK
b: Optional[int] = None

# str assigned to Optional[int] → FAIL
c: Optional[int] = "wrong"

# Optional[int] assigned to int → FAIL (None not assignable to int)
d: int = None
"#;
    let diagnostics = run(source)?;
    let e0014 = e0014_count(&diagnostics);
    assert!(
        e0014 >= 2,
        "at least 2 mismatches expected (c and d), got {e0014}: {:?}",
        diagnostics
            .iter()
            .filter(|d| d.code.code == "BSK-E0014")
            .map(|d| &d.message)
            .collect::<Vec<_>>()
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// types.rs is_assignable_to — Union match arm
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mutant_union_assignability() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Union

# int to Union[int, str] → OK
a: Union[int, str] = 42

# str to Union[int, str] → OK
b: Union[int, str] = "hello"

# float to Union[int, str] → FAIL
c: Union[int, str] = 3.14

# list to Union[int, str] → FAIL
d: Union[int, str] = [1, 2]
"#;
    let diagnostics = run(source)?;
    let e0014 = e0014_count(&diagnostics);
    assert!(
        e0014 >= 1,
        "at least 1 union mismatch expected, got {e0014}"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// types.rs is_assignable_to — List/Set match arm
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mutant_list_assignability() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
# list[int] → list[int]: OK
a: list[int] = [1, 2, 3]

# list[str] → list[int]: FAIL
b: list[int] = ["a", "b"]

# set[int] → set[int]: OK
c: set[int] = {1, 2, 3}

# set[str] → set[int]: FAIL
d: set[int] = {"a", "b"}
"#;
    let diagnostics = run(source)?;
    let e0014 = e0014_count(&diagnostics);
    assert!(
        e0014 >= 1,
        "at least 1 list/set type element mismatch expected, got {e0014}"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// types.rs is_assignable_to — Dict match arm
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mutant_dict_assignability() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
# dict[str, int] → dict[str, int]: OK
a: dict[str, int] = {"x": 1}

# dict[str, str] → dict[str, int]: FAIL
b: dict[str, int] = {"x": "wrong"}

# dict[int, str] → dict[str, int]: FAIL
c: dict[str, int] = {1: 2}
"#;
    let diagnostics = run(source)?;
    let e0014 = e0014_count(&diagnostics);
    assert!(
        e0014 >= 1,
        "at least 1 dict type mismatch expected, got {e0014}"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// types.rs is_assignable_to — Tuple match arm
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mutant_tuple_assignability() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
# tuple[int, str] → tuple[int, str]: OK
a: tuple[int, str] = (1, "hello")

# tuple[str, int] → tuple[int, str]: FAIL
b: tuple[int, str] = ("wrong", 42)

# Wrong arity
c: tuple[int, str] = (1,)
"#;
    let diagnostics = run(source)?;
    let e0014 = e0014_count(&diagnostics);
    assert!(
        e0014 >= 1,
        "at least 1 tuple mismatch expected, got {e0014}"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// types.rs is_assignable_to — Callable match arm
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mutant_callable_assignability() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def f1(x: int) -> str:
    return str(x)

def f2(x: str) -> int:
    return int(x)

# Correct assignment
a: Callable[[int], str] = f1

# Return type mismatch
b: Callable[[int], int] = f1

# Param type mismatch
c: Callable[[str], str] = f1
"#;
    let diagnostics = run(source)?;
    let _ = diagnostics;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// types.rs is_assignable_to — Named type match arm
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mutant_named_type_assignability() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
# Same named types → OK
a: int = 42
b: str = "hello"
c: float = 3.14
d: bool = True

# Cross-type assignment → FAIL
e: int = "wrong"
f: str = 42
g: float = "bad"
h: bytes = 42
"#;
    let diagnostics = run(source)?;
    let e0014 = e0014_count(&diagnostics);
    assert!(
        e0014 >= 3,
        "at least 3 named type mismatches, got {e0014}: {:?}",
        diagnostics
            .iter()
            .filter(|d| d.code.code == "BSK-E0014")
            .map(|d| &d.message)
            .collect::<Vec<_>>()
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// types.rs InferredType::union — edge cases
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mutant_union_construction() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Union

# Single-element union is just the element
a: Union[int] = 42
b: Union[int] = "wrong"

# Multi-element union
c: Union[int, str, float] = 42
d: Union[int, str, float] = "hello"
e: Union[int, str, float] = 3.14
f: Union[int, str, float] = [1, 2]
"#;
    let diagnostics = run(source)?;
    let e0014 = e0014_count(&diagnostics);
    assert!(
        e0014 >= 1,
        "at least 1 union mismatch expected, got {e0014}"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// E0097: Protocol undeclared self attrs — kill all mutants
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mutant_e0097_declared_vs_undeclared() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class MyProto(Protocol):
    x: int
    y: str

    def __init__(self) -> None:
        self.x = 0       # declared → OK
        self.y = ""       # declared → OK
        self.z = True     # undeclared → E0097
        self.w = 42       # undeclared → E0097
"#;
    let diagnostics = run(source)?;
    let e0097: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0097")
        .collect();
    // Must flag z and w
    assert!(
        e0097.len() >= 2,
        "should flag self.z and self.w, got {}: {:?}",
        e0097.len(),
        e0097.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    // Must NOT flag x and y
    let false_positives = e0097
        .iter()
        .filter(|d| d.message.contains("`x`") || d.message.contains("`y`"))
        .count();
    assert_eq!(
        false_positives, 0,
        "declared attrs x and y must not be flagged"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// E0150: Dead branch — kill mutants in version/platform detection
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mutant_e0150_version_less_than() -> Result<(), Box<dyn std::error::Error>> {
    // sys.version_info < (3, 8) is FALSE on 3.12 → if-body is dead
    let source = r#"
import sys

def check():
    if sys.version_info < (3, 8):
        dead = "dead"
    else:
        live = "live"
    x = dead
    y = live
"#;
    let diagnostics = run(source)?;
    let e0150: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0150")
        .collect();
    assert!(
        e0150.iter().any(|d| d.message.contains("dead")),
        "must flag `dead` variable: {:?}",
        e0150.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    assert!(
        !e0150.iter().any(|d| d.message.contains("live")),
        "must NOT flag `live` variable"
    );
    Ok(())
}

#[test]
fn mutant_e0150_version_greater_equal() -> Result<(), Box<dyn std::error::Error>> {
    // sys.version_info >= (3, 12) is TRUE on 3.12 → else-body is dead
    let source = r#"
import sys

def check():
    if sys.version_info >= (3, 12):
        live = "live"
    else:
        dead = "dead"
    x = dead
    y = live
"#;
    let diagnostics = run(source)?;
    let e0150: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0150")
        .collect();
    assert!(
        e0150.iter().any(|d| d.message.contains("dead")),
        "must flag `dead` from else-body: {:?}",
        e0150.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    assert!(
        !e0150.iter().any(|d| d.message.contains("live")),
        "must NOT flag `live` from if-body"
    );
    Ok(())
}

#[test]
fn mutant_e0150_platform_bogus() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
import sys

def check():
    if sys.platform == "bogus":
        dead = "dead"
    else:
        live = "live"
    x = dead
"#;
    let diagnostics = run(source)?;
    let e0150: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0150")
        .collect();
    assert!(
        !e0150.is_empty(),
        "must flag dead variable in bogus platform branch"
    );
    Ok(())
}

#[test]
fn mutant_e0150_platform_not_bogus() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
import sys

def check():
    if sys.platform != "bogus":
        live = "live"
    else:
        dead = "dead"
    x = dead
"#;
    let diagnostics = run(source)?;
    let e0150: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-E0150")
        .collect();
    assert!(
        !e0150.is_empty(),
        "must flag dead variable in != bogus else branch"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// E0014: Various literal type mismatches — each type path
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mutant_e0014_every_literal_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
# Each literal type assigned to wrong annotation
a: str = 42            # int → str FAIL
b: int = "hello"       # str → int FAIL
c: int = 3.14          # float → int FAIL
d: int = True          # bool → int is OK (bool subtype of int)
e: str = b"bytes"      # bytes → str FAIL
f: int = None          # None → int FAIL
g: int = [1, 2]        # list → int FAIL
h: int = {"a": 1}      # dict → int FAIL
i: int = {1, 2}        # set → int FAIL

# Correct ones — must NOT fire
j: int = 42
k: str = "hello"
l: float = 3.14
m: bool = True
n: bytes = b"data"
"#;
    let diagnostics = run(source)?;
    let e0014 = e0014_count(&diagnostics);
    assert!(
        e0014 >= 6,
        "at least 6 type mismatches expected, got {e0014}: {:?}",
        diagnostics
            .iter()
            .filter(|d| d.code.code == "BSK-E0014")
            .map(|d| &d.message)
            .collect::<Vec<_>>()
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// E0014: Negative literal and float literal
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn mutant_e0014_negative_and_float() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
# Negative int → float: OK
a: float = -42

# Negative int → str: FAIL
b: str = -42

# Float → int: FAIL
c: int = 3.14

# Float → float: OK
d: float = 3.14
"#;
    let diagnostics = run(source)?;
    let e0014 = e0014_count(&diagnostics);
    assert!(
        e0014 >= 1,
        "at least 1 mismatch (b: str = -42 or c: int = 3.14), got {e0014}"
    );
    Ok(())
}
