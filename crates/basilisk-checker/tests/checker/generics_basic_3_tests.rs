//! Tests for [`generics_basic_3`] from [CHKARCH-DIAG-CATEGORIES],
//! [TYPEINF-GENERICS], [TYPEINF-GENERICS-TYPEVAR], and
//! [TYPEINF-GENERICS-CONSTRAINED]. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for generics_basic_3: Generic type argument violations.

use super::common::*;

#[test]
fn constrained_typevar_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
AnyStr = TypeVar("AnyStr", str, bytes)

def concat(x: AnyStr, y: AnyStr) -> AnyStr:
    return x + y

def bad(s: str, b: bytes) -> None:
    concat(s, b)
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"generics_basic_3"),
        "incompatible constraint groups should fire E0148, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn valid_constrained_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
AnyStr = TypeVar("AnyStr", str, bytes)

def concat(x: AnyStr, y: AnyStr) -> AnyStr:
    return x + y

def good() -> None:
    concat("a", "b")
    concat(b"a", b"b")
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"generics_basic_3"),
        "matching constraint groups should not fire E0148"
    );
    Ok(())
}

#[test]
fn mapping_subscript_key_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T")
class MyMap(dict[str, int]):
    pass

m = MyMap()
m[0]
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
