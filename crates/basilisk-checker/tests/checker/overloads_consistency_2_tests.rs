//! Tests for [overloads_consistency_2] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-ownership
// Integration tests for overloads_consistency_2: inconsistent decorators across an overload group.

use super::common::*;

#[test]
fn static_inconsistent_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import overload\nclass C:\n    @overload\n    @staticmethod\n    def f(x: int) -> int: ...\n    @overload\n    @staticmethod\n    def f(x: str) -> str: ...\n    def f(x): return x\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"overloads_consistency_2"),
        "impl missing @staticmethod that the overloads have must fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn consistent_static_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import overload\nclass C:\n    @overload\n    @staticmethod\n    def f(x: int) -> int: ...\n    @overload\n    @staticmethod\n    def f(x: str) -> str: ...\n    @staticmethod\n    def f(x): return x\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"overloads_consistency_2"),
        "uniform @staticmethod across overloads + impl must not fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn final_on_overload_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import overload, final\nclass C:\n    @overload\n    @final\n    def f(self, x: int) -> int: ...\n    @overload\n    def f(self, x: str) -> str: ...\n    def f(self, x): return x\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"overloads_consistency_2"),
        "@final on an overload signature must fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn final_on_impl_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import overload, final\nclass C:\n    @overload\n    def f(self, x: int) -> int: ...\n    @overload\n    def f(self, x: str) -> str: ...\n    @final\n    def f(self, x): return x\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"overloads_consistency_2"),
        "@final on the implementation only is correct; must not fire"
    );
    Ok(())
}

#[test]
fn override_on_overload_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import overload, override\nclass B:\n    def f(self, x): ...\nclass C(B):\n    @overload\n    @override\n    def f(self, x: int) -> int: ...\n    @overload\n    def f(self, x: str) -> str: ...\n    def f(self, x): return x\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"overloads_consistency_2"),
        "@override on an overload signature must fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
