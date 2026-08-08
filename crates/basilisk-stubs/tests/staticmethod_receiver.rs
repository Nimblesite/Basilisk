//! Implements [STUBRES-PYI]: `staticmethod` recognition in stub methods is a
//! binding question, never a spelling question.
//!
//! Pins the 2026-08-08 review finding against `pyi_parser/syntax.rs`: the
//! receiver decision matched the raw identifier `staticmethod`, so an aliased
//! import (`from builtins import staticmethod as sm`) was missed and a stub
//! defining its own `staticmethod` was falsely treated as the builtin. The
//! decorator node must resolve through the module's bindings to
//! `TypingForm::StaticMethod`.
#![allow(clippy::allow_attributes, clippy::expect_used, clippy::panic)]

use std::path::Path;

use basilisk_stubs::types::{StubFunction, StubSource, StubTier};
use basilisk_stubs::{parse_pyi_source, StubModule};

fn parse_stub(source: &str) -> StubModule {
    parse_pyi_source(
        source,
        Path::new("test.pyi"),
        "test",
        StubSource::UserStub,
        StubTier::Tier1,
    )
    .expect("stub should parse")
}

fn method<'m>(stub: &'m StubModule, class: &str, name: &str) -> &'m StubFunction {
    stub.classes
        .get(class)
        .expect("class should exist")
        .methods
        .iter()
        .find(|candidate| candidate.name == name)
        .expect("method should exist")
}

/// `from builtins import staticmethod as sm` is the builtin under another
/// name: the decorated method has no receiver to strip.
#[test]
fn aliased_staticmethod_import_strips_receiver() {
    let stub = parse_stub(
        "
from builtins import staticmethod as sm

class Circle:
    @sm
    def make(radius: float) -> float: ...
",
    );
    let make = method(&stub, "Circle", "make");
    assert_eq!(make.receiver, None);
    assert_eq!(make.params.len(), 1);
    assert_eq!(make.params.first().map(|param| param.name.as_str()), Some("radius"));
}

/// `import builtins` + `@builtins.staticmethod` is the same builtin reached
/// through the module attribute.
#[test]
fn qualified_staticmethod_strips_receiver() {
    let stub = parse_stub(
        "
import builtins

class Circle:
    @builtins.staticmethod
    def make(radius: float) -> float: ...
",
    );
    assert_eq!(method(&stub, "Circle", "make").receiver, None);
}

/// A bare `@staticmethod` with no import is the builtin fallback.
#[test]
fn bare_staticmethod_strips_receiver() {
    let stub = parse_stub(
        "
class Circle:
    @staticmethod
    def make(radius: float) -> float: ...
",
    );
    assert_eq!(method(&stub, "Circle", "make").receiver, None);
}

/// A stub that defines its own `staticmethod` earlier in the module has
/// rebound the name; the decorator is NOT the builtin and the method keeps
/// its receiver.
#[test]
fn module_shadow_keeps_receiver() {
    let stub = parse_stub(
        "
def staticmethod(func): ...

class Circle:
    @staticmethod
    def area(self) -> float: ...
",
    );
    let area = method(&stub, "Circle", "area");
    assert_eq!(
        area.receiver.as_ref().map(|receiver| receiver.name.as_str()),
        Some("self")
    );
}

/// An undecorated method always binds its first parameter as the receiver.
#[test]
fn undecorated_method_binds_receiver() {
    let stub = parse_stub(
        "
class Circle:
    def area(self) -> float: ...
",
    );
    let area = method(&stub, "Circle", "area");
    assert_eq!(
        area.receiver.as_ref().map(|receiver| receiver.name.as_str()),
        Some("self")
    );
    assert!(area.params.is_empty());
}
