//! Tests for [`protocols_definition_2`] from [CHKARCH-DIAG-CATEGORIES] / [TYPEINF-SUBTYPING-PROTOCOL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for protocols_definition_2: Protocol conformance violation.

use super::common::*;

#[test]
fn conforming_class() -> Result<(), Box<dyn std::error::Error>> {
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
        !codes(&diags).contains(&"protocols_definition_2"),
        "conforming class should not fire E0121"
    );
    Ok(())
}

#[test]
fn non_conforming_class() -> Result<(), Box<dyn std::error::Error>> {
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
fn missing_instance_var() -> Result<(), Box<dyn std::error::Error>> {
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
        has_code(&diags, "protocols_definition_2"),
        "missing protocol instance variable must fire E0121"
    );
    Ok(())
}

#[test]
fn instance_var_provided_via_self_ok() -> Result<(), Box<dyn std::error::Error>> {
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
        !has_code(&diags, "protocols_definition_2"),
        "a `self`-assigned instance variable satisfies the protocol"
    );
    Ok(())
}

#[test]
fn keyword_only_param_mismatch() -> Result<(), Box<dyn std::error::Error>> {
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
        has_code(&diags, "protocols_definition_2"),
        "keyword-only impl params must fire E0121"
    );
    Ok(())
}

#[test]
fn positional_only_param_mismatch() -> Result<(), Box<dyn std::error::Error>> {
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
        has_code(&diags, "protocols_definition_2"),
        "positional-only impl params must fire E0121"
    );
    Ok(())
}

#[test]
fn matching_param_kinds_ok() -> Result<(), Box<dyn std::error::Error>> {
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
        !has_code(&diags, "protocols_definition_2"),
        "matching positional-or-keyword params must not fire E0121"
    );
    Ok(())
}

#[test]
fn call_arg_protocol_element_mismatch() -> Result<(), Box<dyn std::error::Error>> {
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
        has_code(&diags, "protocols_definition_2"),
        "`int` element cannot satisfy `Iterable[SupportsClose]`"
    );
    Ok(())
}

#[test]
fn call_arg_non_literal_elements_ok() -> Result<(), Box<dyn std::error::Error>> {
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
        !has_code(&diags, "protocols_definition_2"),
        "a container of conforming objects must not fire E0121"
    );
    Ok(())
}

// ── Protocol-typed call arguments ────────────────────────────────────────────
// A parameter annotated with a bare Protocol is checked structurally at the call
// site, exactly as an annotated assignment already is. Before these tests the
// rule only inspected `Container[Protocol]` parameters receiving literal
// displays, so `show(User())` — the shape the website playground demonstrates —
// passed silently while `x: Renderable = User()` was correctly rejected.

#[test]
fn protocol_param_rejects_non_conforming_argument() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Renderable(Protocol):
    def render(self) -> str: ...

class User:
    name: str = 'Ada'

def show(item: Renderable) -> None:
    print(item.render())

show(User())
";
    let diags = run(source)?;
    assert!(
        has_code(&diags, "protocols_definition_2"),
        "passing a class that lacks a protocol method must fire E0121, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn protocol_param_accepts_conforming_argument() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Renderable(Protocol):
    def render(self) -> str: ...

class User:
    def render(self) -> str:
        return 'Ada'

def show(item: Renderable) -> None:
    print(item.render())

show(User())
";
    let diags = run(source)?;
    assert!(
        !has_code(&diags, "protocols_definition_2"),
        "a conforming argument must not fire E0121, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn protocol_param_accepts_member_inherited_from_local_base(
) -> Result<(), Box<dyn std::error::Error>> {
    // The member is supplied by a base class, so the argument does conform.
    let source = r"
from typing import Protocol

class Renderable(Protocol):
    def render(self) -> str: ...

class Base:
    def render(self) -> str:
        return 'base'

class User(Base):
    name: str = 'Ada'

def show(item: Renderable) -> None:
    print(item.render())

show(User())
";
    let diags = run(source)?;
    assert!(
        !has_code(&diags, "protocols_definition_2"),
        "a member inherited from a local base must satisfy the protocol, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn protocol_param_ignores_argument_with_unknown_base() -> Result<(), Box<dyn std::error::Error>> {
    // `Mystery` is imported, so its members are unknown here and it may well
    // supply `render`. Staying silent is the only false-positive-safe answer.
    let source = r"
from typing import Protocol
from elsewhere import Mystery

class Renderable(Protocol):
    def render(self) -> str: ...

class User(Mystery):
    name: str = 'Ada'

def show(item: Renderable) -> None:
    print(item.render())

show(User())
";
    let diags = run(source)?;
    assert!(
        !has_code(&diags, "protocols_definition_2"),
        "a class with an unknown base must never be flagged, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

// ── Call-argument false-positive guards ──────────────────────────────────────
// Every case below is VALID under PEP 544 and was a REAL false positive of the
// first call-argument implementation, found by an adversarial search rather
// than by the conformance suite (none of these shapes appear in its fixtures).
// A false positive on valid code is worse than the miss it replaced, so each
// one is pinned here.

/// A member installed as `self.<name> = ...` satisfies the protocol, exactly as
/// it already does on the annotated-assignment path.
#[test]
fn call_arg_accepts_member_assigned_on_self() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class R(Protocol):
    def render(self) -> str: ...

class U:
    def __init__(self) -> None:
        self.render = lambda: 'x'

def show(i: R) -> None: ...

show(U())
";
    let diags = run(source)?;
    assert!(
        !has_code(&diags, "protocols_definition_2"),
        "a member bound on self must satisfy the protocol, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// `if TYPE_CHECKING:` around a method is idiomatic in typed libraries; the
/// method is present, merely not at the top level of the class body.
#[test]
fn call_arg_accepts_method_under_type_checking() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, TYPE_CHECKING

class R(Protocol):
    def render(self) -> str: ...

class U:
    if TYPE_CHECKING:
        def render(self) -> str: ...
    else:
        def render(self) -> str:
            return 'x'

def show(i: R) -> None: ...

show(U())
";
    let diags = run(source)?;
    assert!(
        !has_code(&diags, "protocols_definition_2"),
        "a method under `if TYPE_CHECKING` must not be treated as absent, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// Version-gated method definitions are ordinary compatibility code.
#[test]
fn call_arg_accepts_version_gated_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
import sys
from typing import Protocol

class R(Protocol):
    def render(self) -> str: ...

class U:
    if sys.version_info >= (3, 8):
        def render(self) -> str:
            return 'a'
    else:
        def render(self) -> str:
            return 'b'

def show(i: R) -> None: ...

show(U())
";
    let diags = run(source)?;
    assert!(
        !has_code(&diags, "protocols_definition_2"),
        "a version-gated method must not be treated as absent, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// A `try`/`except` around a definition is the classic import-fallback shape.
#[test]
fn call_arg_accepts_method_defined_in_try_block() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class R(Protocol):
    def render(self) -> str: ...

class U:
    try:
        def render(self) -> str:
            return 'x'
    except Exception:
        pass

def show(i: R) -> None: ...

show(U())
";
    let diags = run(source)?;
    assert!(
        !has_code(&diags, "protocols_definition_2"),
        "a method defined inside try/except must not be treated as absent, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// `__getattr__` can answer for any member name, so no member is ever provably
/// absent from such a class.
#[test]
fn call_arg_ignores_class_with_dynamic_attribute_lookup() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r"
from typing import Protocol, Any

class R(Protocol):
    def render(self) -> str: ...

class U:
    def __getattr__(self, name: str) -> Any: ...

def show(i: R) -> None: ...

show(U())
";
    let diags = run(source)?;
    assert!(
        !has_code(&diags, "protocols_definition_2"),
        "a class with __getattr__ can supply any member, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// The opacity rule must follow the inheritance chain, not just the leaf class.
#[test]
fn call_arg_accepts_member_hidden_in_a_local_base() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class R(Protocol):
    def render(self) -> str: ...

class Base:
    if True:
        def render(self) -> str:
            return 'x'

class U(Base):
    pass

def show(i: R) -> None: ...

show(U())
";
    let diags = run(source)?;
    assert!(
        !has_code(&diags, "protocols_definition_2"),
        "a member hidden inside a local base's conditional must not be flagged, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// A name rebound in an inner scope no longer means what it meant at module
/// level, so the module-level tables cannot resolve it. `U = V` constructs `V`.
#[test]
fn call_arg_ignores_class_name_rebound_in_an_inner_scope() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r"
from typing import Protocol

class R(Protocol):
    def render(self) -> str: ...

class U:
    pass

class V:
    def render(self) -> str:
        return 'x'

def show(i: R) -> None: ...

def go() -> None:
    U = V
    show(U())
";
    let diags = run(source)?;
    assert!(
        !has_code(&diags, "protocols_definition_2"),
        "a rebound class name must not be resolved against the module table, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// The same hazard via a parameter rather than an assignment.
#[test]
fn call_arg_ignores_class_name_shadowed_by_a_parameter() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class R(Protocol):
    def render(self) -> str: ...

class U:
    pass

def show(i: R) -> None: ...

def go(U: type) -> None:
    show(U())
";
    let diags = run(source)?;
    assert!(
        !has_code(&diags, "protocols_definition_2"),
        "a class name shadowed by a parameter must not be resolved, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
