//! Tests for [`directives_cast`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
// Integration tests for directives_cast: Invalid `cast()` call.

use super::common::*;

#[test]
fn cast_literal_first_arg_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import cast
x: int = 1
y = cast(1, x)
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"directives_cast"),
        "cast with literal first arg should fire E0031, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn cast_too_few_args_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import cast
y = cast()
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"directives_cast"),
        "cast() with no args should fire E0031, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn cast_valid_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import cast
x: int = 1
y = cast(str, x)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"directives_cast"),
        "valid cast should not fire E0031"
    );
    Ok(())
}

/// A quoted string is a legal first argument to `cast()`: it is the standard
/// forward-reference spelling, and typeshed admits it directly
/// (`cast(typ: type[_T] | str | Any, val)`). Flagging it as a "value literal"
/// is a false positive — and, because ruff's `TC006` actively *requires* the
/// quotes, it puts Basilisk in unsatisfiable conflict with ruff (issue #335).
#[test]
fn cast_string_forward_reference_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import cast
from uuid import UUID


class Local: ...


a = cast("UUID", object())
b = cast("Local", object())
c = cast("int", 1)
d = cast("dict[str, str]", {})
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"directives_cast"),
        "quoted forward-reference casts are legal (PEP 484 / typeshed / ruff TC006) and must not fire directives_cast, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
