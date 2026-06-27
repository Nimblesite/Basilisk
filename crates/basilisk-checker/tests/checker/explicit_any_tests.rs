//! Tests for [BSK-W0014] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// E2E tests for BSK-W0014: Explicit `Any` annotation warning (split from returns_compatibility).

use super::common::*;

#[test]
fn w0014_explicit_any_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import Any\n\ndef greet(name: Any) -> str:\n    return name\n";
    let diags = run_with_optin_rules(source)?;
    assert!(
        codes(&diags).contains(&"BSK-W0014"),
        "explicit Any param annotation should fire W0014, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn w0014_explicit_any_return_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import Any\n\ndef greet(name: str) -> Any:\n    return name\n";
    let diags = run_with_optin_rules(source)?;
    assert!(
        codes(&diags).contains(&"BSK-W0014"),
        "explicit Any return annotation should fire W0014, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn w0014_explicit_any_fires_even_on_stub() -> Result<(), Box<dyn std::error::Error>> {
    // Explicit Any check fires even on stub bodies (it is not a body-dependent check).
    let source = "from typing import Any\n\ndef greet(name: Any) -> str:\n    ...\n";
    let diags = run_with_optin_rules(source)?;
    assert!(
        codes(&diags).contains(&"BSK-W0014"),
        "explicit Any should fire W0014 even on stub body, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn w0014_explicit_any_is_a_warning_not_e0011() -> Result<(), Box<dyn std::error::Error>> {
    // The split's whole point: the Any nudge no longer rides the E0011 error code,
    // so it can be silenced independently of the genuine return-mismatch error.
    let source = "from typing import Any\n\ndef greet(name: Any) -> str:\n    return name\n";
    let diags = run_with_optin_rules(source)?;
    assert!(
        !codes(&diags).contains(&"returns_compatibility"),
        "explicit Any must NOT fire the E0011 error code anymore, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn w0014_concrete_types_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def greet(name: str) -> str:\n    return name\n";
    let diags = run_with_optin_rules(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-W0014"),
        "concrete annotations should not fire W0014, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
