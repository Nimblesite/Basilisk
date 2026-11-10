//! Tests for [`directives_cast`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
//!
//! The behavior is defined by [PEP 484 casts](https://peps.python.org/pep-0484/#casts),
//! and the every-expression-position regressions reference
//! [#335](https://github.com/Nimblesite/Basilisk/issues/335).

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

/// A `cast()` in return position is the same call in a different statement —
/// it must be validated identically. Part 2 of issue #335: the rule only ever
/// saw casts reachable from an assignment RHS, a bare expression statement, or
/// an `if` test, so `return cast(1, x)` went unchecked.
#[test]
fn cast_literal_first_arg_in_return_position_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import cast


def f(x: object) -> int:
    return cast(1, x)
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"directives_cast"),
        "a value-literal cast in return position must fire directives_cast, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// Arity errors are position-independent too — `return cast(int, x, x)` is as
/// invalid as `y = cast(int, x, x)` (issue #335).
#[test]
fn cast_wrong_arity_in_return_position_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import cast


def f(x: object) -> int:
    return cast(int, x, x)
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"directives_cast"),
        "a three-argument cast in return position must fire directives_cast, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// A `cast()` nested inside another call's arguments is never the outermost
/// expression of its statement, so the statement-level scan never reached it
/// (issue #335).
#[test]
fn cast_literal_first_arg_in_argument_position_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import cast


def f(x: object) -> None:
    print(cast(1, x))
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"directives_cast"),
        "a value-literal cast in argument position must fire directives_cast, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// Every invalid cast is reported exactly once, and reaching new positions must
/// not double-report the positions that already worked. Four invalid casts in
/// four distinct positions yield four diagnostics — no more, no fewer.
#[test]
fn every_invalid_cast_position_reported_exactly_once() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import cast


def f(x: object) -> int:
    y = cast(1, x)
    cast(2, x)
    print(cast(3, x))
    return cast(4, x)
";
    let diags = run(source)?;
    let cast_diags = codes(&diags)
        .into_iter()
        .filter(|c| *c == "directives_cast")
        .count();
    assert_eq!(
        cast_diags,
        4,
        "four invalid casts in four positions must yield exactly four diagnostics, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// Widening the positions the rule sees must not make valid casts fire. Every
/// position exercised above, with a legal type expression, stays silent.
#[test]
fn valid_casts_in_all_positions_stay_silent() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import cast


def f(x: object) -> int:
    y = cast(int, x)
    cast(str, x)
    print(cast("int", x))
    for _ in range(cast(int, x)):
        pass
    return cast(int, y)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"directives_cast"),
        "valid casts must stay silent in every statement position, got: {:?}",
        codes(&diags)
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
