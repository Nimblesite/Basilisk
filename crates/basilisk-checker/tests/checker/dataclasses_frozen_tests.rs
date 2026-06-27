//! Tests for [dataclasses_frozen] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
// Integration tests for dataclasses_frozen: Frozen dataclass violations.

use super::common::*;

#[test]
fn e0052_frozen_inherits_nonfrozen_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass
class Base:
    x: int = 0

@dataclass(frozen=True)
class Sub(Base):
    y: int = 0
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "dataclasses_frozen");
    assert!(
        msgs.iter()
            .any(|m| m.contains("Frozen") && m.contains("non-frozen")),
        "frozen inheriting non-frozen should fire E0052, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0052_nonfrozen_inherits_frozen_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(frozen=True)
class Base:
    x: int = 0

@dataclass
class Sub(Base):
    y: int = 0
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "dataclasses_frozen");
    assert!(
        msgs.iter()
            .any(|m| m.contains("Non-frozen") && m.contains("frozen")),
        "non-frozen inheriting frozen should fire E0052, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0052_both_frozen_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(frozen=True)
class Base:
    x: int = 0

@dataclass(frozen=True)
class Sub(Base):
    y: int = 0
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "dataclasses_frozen");
    assert!(
        msgs.is_empty(),
        "both frozen should not fire E0052, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0052_assign_frozen_instance_attr_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(frozen=True)
class Point:
    x: float = 0.0

p = Point(1.0)
p.x = 2.0
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "dataclasses_frozen");
    assert!(
        msgs.iter().any(|m| m.contains("Cannot assign")),
        "assigning to frozen instance should fire E0052, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0052_non_frozen_instance_assign_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass
class Point:
    x: float = 0.0

p = Point(1.0)
p.x = 2.0
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "dataclasses_frozen");
    assert!(
        msgs.is_empty(),
        "assigning to non-frozen instance should not fire E0052"
    );
    Ok(())
}
