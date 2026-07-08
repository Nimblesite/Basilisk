//! Tests for [BSK-E0025] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for BSK-E0025: Missing @override decorator.

use super::common::*;

/// Run the checker against a source with an explicit configured target version
/// (mirrors `version_target_tests::run_with_python_version`).
fn run_with_python_version(source: &str, version: &str) -> Vec<Diagnostic> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned()).unwrap();
    let resolved = resolve(&parsed).unwrap();
    let config = basilisk_config::BasiliskConfig {
        python_version: Some(version.to_owned()),
        strict_annotations: true,
        ..Default::default()
    };
    basilisk_checker::check_with_config(&resolved, &config)
}

/// The exact repro from issue #171: an unparameterized override that fires
/// BSK-E0025 on the default target.
const ISSUE_171_SOURCE: &str = r#"
class Test:
    def test() -> None:
        print("test")

class Test2(Test):
    def test() -> None:
        print("test2")
"#;

#[test]
fn missing_override_decorator_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Base:
    def process(self) -> None:
        pass

class Child(Base):
    def process(self) -> None:
        pass
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        codes(&diags).contains(&"BSK-E0025"),
        "overriding method without @override should fire BSK-E0025, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn with_override_decorator_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import override

class Base:
    def process(self) -> None:
        pass

class Child(Base):
    @override
    def process(self) -> None:
        pass
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-E0025"),
        "method with @override should not fire BSK-E0025"
    );
    Ok(())
}

#[test]
fn different_method_name_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Base:
    def process(self) -> None:
        pass

class Child(Base):
    def other_method(self) -> None:
        pass
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-E0025"),
        "different method name should not fire BSK-E0025"
    );
    Ok(())
}

// Issue #171: `@override` (PEP 698 / `typing.override`) only exists from Python
// 3.12. When the configured target is 3.10 or 3.11, suggesting `@override` is a
// false positive — the decorator cannot be imported there. BSK-E0025 must be silent
// below 3.12.

#[test]
fn silent_on_3_11_target() {
    let diags = run_with_python_version(ISSUE_171_SOURCE, "3.11");
    assert!(
        !codes(&diags).contains(&"BSK-E0025"),
        "`@override` requires Python 3.12+ (PEP 698); BSK-E0025 must not fire on a 3.11 target: {:?}",
        codes(&diags)
    );
}

#[test]
fn silent_on_3_10_target() {
    let diags = run_with_python_version(ISSUE_171_SOURCE, "3.10");
    assert!(
        !codes(&diags).contains(&"BSK-E0025"),
        "`@override` requires Python 3.12+ (PEP 698); BSK-E0025 must not fire on a 3.10 target: {:?}",
        codes(&diags)
    );
}

#[test]
fn fires_on_3_12_target() {
    let diags = run_with_python_version(ISSUE_171_SOURCE, "3.12");
    assert!(
        codes(&diags).contains(&"BSK-E0025"),
        "`@override` is native on 3.12; BSK-E0025 must still fire there: {:?}",
        codes(&diags)
    );
}

// Issue #278: `class Client(httpx.Client)` records the base under its
// attribute name `Client`, which collides with the class's own name — the
// class map then resolves the "base" back to the class itself. The
// transitive-Protocol walk must not recurse forever on that self-referential
// chain (stack overflow, SIGABRT crash loop in the LSP), and the class must
// not be treated as overriding its own methods.

#[test]
fn self_named_external_base_does_not_overflow_or_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
import httpx

class Client(httpx.Client):
    def request(self) -> None:
        pass
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-E0025"),
        "a class is not an override of itself; base-name collision with an \
         external attribute base must not fire BSK-E0025: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn protocol_class_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class MyProto(Protocol):
    def method(self) -> None: ...

class Impl(MyProto):
    def method(self) -> None:
        pass
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-E0025"),
        "Protocol implementation should be exempt from BSK-E0025"
    );
    Ok(())
}
