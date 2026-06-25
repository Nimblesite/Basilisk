//! Tests for [BSK-E0159] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-ownership
// Integration tests for BSK-E0159: @override with no matching ancestor method.

use super::common::*;

#[test]
fn e0159_override_no_ancestor_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import override\nclass P:\n    def m1(self) -> int: return 1\nclass C(P):\n    @override\n    def m3(self) -> int: return 1\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0159"),
        "@override on a method absent from the base must fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0159_valid_override_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import override\nclass P:\n    def m1(self) -> int: return 1\nclass C(P):\n    @override\n    def m1(self) -> int: return 2\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0159"),
        "@override that does override a base method must not fire"
    );
    Ok(())
}

#[test]
fn e0159_parent_derives_from_any_suppressed() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import Any, override\nclass PB(Any):\n    pass\nclass CB(PB):\n    @override\n    def m1(self) -> None:\n        pass\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0159"),
        "a base deriving from Any may supply the method; must not fire"
    );
    Ok(())
}

#[test]
fn e0159_imported_base_suppressed() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import override\nfrom somewhere import Base\nclass C(Base):\n    @override\n    def m(self) -> int: return 1\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0159"),
        "an unseen imported base may supply the method; must not fire"
    );
    Ok(())
}

#[test]
fn e0159_staticmethod_no_ancestor_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import override\nclass P:\n    def m1(self) -> int: return 1\nclass C(P):\n    @staticmethod\n    @override\n    def s() -> int: return 1\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0159"),
        "@override @staticmethod with no ancestor must fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
