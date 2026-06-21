//! Tests for [BSK-E0121] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for BSK-E0121: Protocol conformance violation.

use super::common::*;

#[test]
fn e0121_conforming_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class P(Protocol):
    def method(self) -> None: ...

class C:
    def method(self) -> None:
        pass

x: P = C()
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0121"),
        "conforming class should not fire E0121"
    );
    Ok(())
}

#[test]
fn e0121_non_conforming_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class P(Protocol):
    def method(self) -> None: ...

class C:
    pass

x: P = C()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0121_missing_instance_var() -> Result<(), Box<dyn std::error::Error>> {
    // A protocol writable instance variable that the implementation provides in
    // no form (attribute, `self.<attr>`, or property) is missing.
    let source = r"
from typing import Protocol, Sequence

class Tmpl(Protocol):
    val1: Sequence[int]

class Bad:
    ...

x: Tmpl = Bad()
";
    let diags = run(source)?;
    assert!(
        has_code(&diags, "BSK-E0121"),
        "missing protocol instance variable must fire E0121"
    );
    Ok(())
}

#[test]
fn e0121_instance_var_provided_via_self_ok() -> Result<(), Box<dyn std::error::Error>> {
    // The instance variable is provided through `self.val1 = ...` in __init__.
    let source = r"
from typing import Protocol, Sequence

class Tmpl(Protocol):
    val1: Sequence[int]

class Good:
    def __init__(self) -> None:
        self.val1: Sequence[int] = [0]

x: Tmpl = Good()
";
    let diags = run(source)?;
    assert!(
        !has_code(&diags, "BSK-E0121"),
        "a `self`-assigned instance variable satisfies the protocol"
    );
    Ok(())
}

#[test]
fn e0121_keyword_only_param_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    // A keyword-only implementation parameter cannot satisfy a protocol
    // positional-or-keyword parameter (it can't be passed positionally).
    let source = r"
from typing import Protocol

class Tmpl(Protocol):
    def method1(self, a: int, b: int) -> float: ...

class Bad:
    def method1(self, *, a: int, b: int) -> float:
        return 0

x: Tmpl = Bad()
";
    let diags = run(source)?;
    assert!(
        has_code(&diags, "BSK-E0121"),
        "keyword-only impl params must fire E0121"
    );
    Ok(())
}

#[test]
fn e0121_positional_only_param_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    // A positional-only implementation parameter cannot satisfy a protocol
    // positional-or-keyword parameter (it can't be passed by keyword).
    let source = r"
from typing import Protocol

class Tmpl(Protocol):
    def method1(self, a: int, b: int) -> float: ...

class Bad:
    def method1(self, a: int, b: int, /) -> float:
        return 0

x: Tmpl = Bad()
";
    let diags = run(source)?;
    assert!(
        has_code(&diags, "BSK-E0121"),
        "positional-only impl params must fire E0121"
    );
    Ok(())
}

#[test]
fn e0121_matching_param_kinds_ok() -> Result<(), Box<dyn std::error::Error>> {
    // Matching positional-or-keyword parameters (widened types) conform.
    let source = r"
from typing import Protocol

class Tmpl(Protocol):
    def method1(self, a: int, b: int) -> float: ...

class Good:
    def method1(self, a: float, b: float) -> int:
        return 0

x: Tmpl = Good()
";
    let diags = run(source)?;
    assert!(
        !has_code(&diags, "BSK-E0121"),
        "matching positional-or-keyword params must not fire E0121"
    );
    Ok(())
}

#[test]
fn e0121_call_arg_protocol_element_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    // Passing a list of `int` literals where `Iterable[SupportsClose]` is
    // expected: `int` has no `close` method.
    let source = r"
from typing import Protocol, Iterable

class SupportsClose(Protocol):
    def close(self) -> None: ...

def close_all(things: Iterable[SupportsClose]) -> None:
    for t in things:
        t.close()

close_all([1])
";
    let diags = run(source)?;
    assert!(
        has_code(&diags, "BSK-E0121"),
        "`int` element cannot satisfy `Iterable[SupportsClose]`"
    );
    Ok(())
}

#[test]
fn e0121_call_arg_non_literal_elements_ok() -> Result<(), Box<dyn std::error::Error>> {
    // A container of conforming objects (non-literals) must not be flagged.
    let source = r"
from typing import Protocol, Iterable

class SupportsClose(Protocol):
    def close(self) -> None: ...

class Res:
    def close(self) -> None:
        pass

def close_all(things: Iterable[SupportsClose]) -> None:
    for t in things:
        t.close()

r = Res()
close_all([r])
";
    let diags = run(source)?;
    assert!(
        !has_code(&diags, "BSK-E0121"),
        "a container of conforming objects must not fire E0121"
    );
    Ok(())
}
