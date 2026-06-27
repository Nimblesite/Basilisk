//! Tests for [dataclasses_inheritance] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-ownership
// Integration tests for dataclasses_inheritance: dataclass field without a default after one with a default.

use super::common::*;

#[test]
fn no_default_after_default_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source =
        "from dataclasses import dataclass\n@dataclass\nclass C:\n    a: int = 0\n    b: int\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"dataclasses_inheritance"),
        "no-default field after a defaulted one must fire E0157, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn field_default_call_counts_as_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from dataclasses import dataclass, field\n@dataclass\nclass C:\n    a: int = field(default=1)\n    b: int\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"dataclasses_inheritance"),
        "field(default=...) is a default; following no-default field must fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn initvar_with_default_counts() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from dataclasses import dataclass, InitVar\n@dataclass\nclass C:\n    a: InitVar[int] = 0\n    b: int\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"dataclasses_inheritance"),
        "InitVar participates in __init__; no-default field after it must fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn correct_order_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source =
        "from dataclasses import dataclass\n@dataclass\nclass C:\n    a: int\n    b: int = 0\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"dataclasses_inheritance"),
        "no-default before default is valid; must not fire"
    );
    Ok(())
}

#[test]
fn init_false_excluded() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from dataclasses import dataclass, field\n@dataclass\nclass C:\n    a: int = field(init=False)\n    b: int\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"dataclasses_inheritance"),
        "field(init=False) is not a constructor param; must not fire"
    );
    Ok(())
}

#[test]
fn classvar_excluded() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from dataclasses import dataclass\nfrom typing import ClassVar\n@dataclass\nclass C:\n    a: ClassVar[int] = 0\n    b: int\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"dataclasses_inheritance"),
        "ClassVar is not a dataclass field; must not fire"
    );
    Ok(())
}

#[test]
fn kw_only_excluded() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from dataclasses import dataclass, field\n@dataclass\nclass C:\n    a: int = field(kw_only=True, default=3)\n    b: int\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"dataclasses_inheritance"),
        "kw_only fields are exempt from positional ordering; must not fire"
    );
    Ok(())
}
