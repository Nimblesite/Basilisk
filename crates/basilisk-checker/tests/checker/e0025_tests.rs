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
        ..Default::default()
    };
    basilisk_checker::check_with_config(&resolved, &config)
}

/// The exact repro from issue #171: an unparameterized override that fires
/// E0025 on the default target.
const ISSUE_171_SOURCE: &str = r#"
class Test:
    def test() -> None:
        print("test")

class Test2(Test):
    def test() -> None:
        print("test2")
"#;

#[test]
fn e0025_missing_override_decorator_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Base:
    def process(self) -> None:
        pass

class Child(Base):
    def process(self) -> None:
        pass
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0025"),
        "overriding method without @override should fire E0025, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0025_with_override_decorator_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
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
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0025"),
        "method with @override should not fire E0025"
    );
    Ok(())
}

#[test]
fn e0025_different_method_name_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Base:
    def process(self) -> None:
        pass

class Child(Base):
    def other_method(self) -> None:
        pass
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0025"),
        "different method name should not fire E0025"
    );
    Ok(())
}

// Issue #171: `@override` (PEP 698 / `typing.override`) only exists from Python
// 3.12. When the configured target is 3.10 or 3.11, suggesting `@override` is a
// false positive — the decorator cannot be imported there. E0025 must be silent
// below 3.12.

#[test]
fn e0025_silent_on_3_11_target() {
    let diags = run_with_python_version(ISSUE_171_SOURCE, "3.11");
    assert!(
        !codes(&diags).contains(&"BSK-E0025"),
        "`@override` requires Python 3.12+ (PEP 698); E0025 must not fire on a 3.11 target: {:?}",
        codes(&diags)
    );
}

#[test]
fn e0025_silent_on_3_10_target() {
    let diags = run_with_python_version(ISSUE_171_SOURCE, "3.10");
    assert!(
        !codes(&diags).contains(&"BSK-E0025"),
        "`@override` requires Python 3.12+ (PEP 698); E0025 must not fire on a 3.10 target: {:?}",
        codes(&diags)
    );
}

#[test]
fn e0025_fires_on_3_12_target() {
    let diags = run_with_python_version(ISSUE_171_SOURCE, "3.12");
    assert!(
        codes(&diags).contains(&"BSK-E0025"),
        "`@override` is native on 3.12; E0025 must still fire there: {:?}",
        codes(&diags)
    );
}

#[test]
fn e0025_protocol_class_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class MyProto(Protocol):
    def method(self) -> None: ...

class Impl(MyProto):
    def method(self) -> None:
        pass
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0025"),
        "Protocol implementation should be exempt from E0025"
    );
    Ok(())
}
