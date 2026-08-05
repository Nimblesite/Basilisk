//! Tests for [`names_unbound`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! Integration tests for `names_unbound`: unbound variable on some code paths.
//!
//! [NARROWPLAN-INTEGRATION] Step 8
//! ([#285](https://github.com/Nimblesite/Basilisk/issues/285)): the rule runs
//! a definite-assignment walk with the walker's inference-driven divergence
//! ([NARROWPLAN-FLOW]) — the divergence tests below are mutation-resistant
//! pins: each no-diagnostic case passes ONLY because a diverging branch drops
//! out of the merge, and each is paired with a firing case that keeps the
//! diagnostic alive.

use super::common::*;

#[test]
fn conditionally_assigned_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def maybe_assign(flag: bool) -> int:
    if flag:
        result = 42
    return result
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"names_unbound"),
        "conditionally assigned variable should fire E0019, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn unconditionally_assigned_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def always_assign() -> int:
    result = 42
    return result
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_unbound"),
        "unconditionally assigned variable should not fire E0019"
    );
    Ok(())
}

#[test]
fn assigned_in_try_and_except_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // Regression test for issue #285: `o` is assigned in both the `try` body
    // and the `except` handler, so it is bound on every path that reaches the
    // `return` — no diagnostic.
    let source = r"
def occ(loc: int) -> int:
    try:
        o = loc
    except KeyError:
        o = 0
        print(o)
    else:
        print(o)
    return o
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_unbound"),
        "variable assigned in both try and except is always bound; should not fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn parameter_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def identity(x: int) -> int:\n    return x\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_unbound"),
        "parameter should not fire E0019"
    );
    Ok(())
}

/// [NARROWPLAN-INTEGRATION] Step 8: an `if`/`else` that assigns on both
/// branches binds the name on every path — the merge must intersect, not
/// give up. (Relocated from the deleted resolver-field test.)
#[test]
fn if_else_both_assign_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def choose(flag: bool) -> int:
    if flag:
        x = 1
    else:
        x = 2
    return x
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_unbound"),
        "both branches assign `x` — it is bound on every path, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// An `elif` chain WITHOUT a final `else` leaves a path where nothing was
/// assigned — the merge keeps the implicit fallthrough alive and fires.
#[test]
fn elif_chain_without_else_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def classify(value: int) -> int:
    if value > 0:
        result = 1
    elif value < 0:
        result = -1
    return result
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"names_unbound"),
        "no `else` branch — `result` is unbound when both tests are false, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// A full `if`/`elif`/`else` chain that assigns everywhere is exhaustive.
#[test]
fn elif_chain_with_else_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def classify(value: int) -> int:
    if value > 0:
        result = 1
    elif value < 0:
        result = -1
    else:
        result = 0
    return result
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_unbound"),
        "every branch assigns `result`, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// THE Step 8 pin ([NARROWPLAN-FLOW], #285): the `else` branch DIVERGES, so
/// it never reaches the `return` and cannot leave `result` unbound. The
/// old last-statement idiom had no way to see this. Reverting to a
/// divergence-blind merge makes this fire.
#[test]
fn diverging_else_branch_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def guarded(flag: bool) -> int:
    if flag:
        result = 42
    else:
        return 0
    return result
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_unbound"),
        "the `else` branch returns — the path reaching `return result` always \
         bound `result`, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// The same shape with `raise` instead of `return` — divergence is a
/// property of the statement, not a syntactic `return` match.
#[test]
fn raising_else_branch_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def guarded(flag: bool) -> int:
    if flag:
        result = 42
    else:
        raise ValueError("no")
    return result
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_unbound"),
        "the `else` branch raises — `result` is bound on every live path, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// Inference-driven divergence ([TYPEINF-TARGET-NARROWING]): the else
/// branch calls a `NoReturn` function. Nothing about the CALL is
/// syntactically terminal — only the engine's `Never` verdict proves it.
#[test]
fn noreturn_call_in_else_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NoReturn


def fail(message: str) -> NoReturn:
    raise ValueError(message)


def guarded(flag: bool) -> int:
    if flag:
        result = 42
    else:
        fail("nope")
    return result
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_unbound"),
        "`fail` is `NoReturn` — the engine proves the else branch diverges, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// The paired negative for the divergence pins: a NON-diverging else that
/// leaves the name unassigned must still fire. Deleting the verdict to make
/// the tests above pass breaks this one.
#[test]
fn non_diverging_else_still_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def guarded(flag: bool) -> int:
    if flag:
        result = 42
    else:
        print('nothing')
    return result
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"names_unbound"),
        "the else branch neither assigns nor diverges — must still fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// A `try` whose body assigns but whose handler does NOT leaves a live path
/// where the name is unbound.
#[test]
fn try_assigns_handler_does_not_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def risky(source: str) -> int:
    try:
        value = int(source)
    except ValueError:
        print('bad')
    return value
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"names_unbound"),
        "the handler path leaves `value` unbound, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// ... and a handler that DIVERGES removes that path entirely.
#[test]
fn try_with_diverging_handler_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def risky(source: str) -> int:
    try:
        value = int(source)
    except ValueError:
        return -1
    return value
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_unbound"),
        "the handler returns — every live path bound `value`, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// PEP 572: a walrus in the `if` test binds whenever the statement is
/// reached, so the name is bound past the `if` regardless of the branch.
#[test]
fn walrus_in_if_test_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def parse(raw: str) -> int:
    if (parsed := len(raw)) > 3:
        print(parsed)
    return parsed
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_unbound"),
        "the walrus in the test binds on every path past the `if`, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// A walrus inside a BRANCH body binds only on that branch.
#[test]
fn walrus_inside_branch_body_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def parse(raw: str, flag: bool) -> int:
    if flag:
        print(parsed := len(raw))
    return parsed
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"names_unbound"),
        "the walrus runs only when `flag` is true, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// A loop body may run zero times, so nothing it binds is definite — but the
/// walk abstains there rather than firing (gradual posture,
/// [TYPEINF-TARGET-GRADUAL]); the loop TARGET is accepted past the loop.
#[test]
fn for_target_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def last(values: list[int]) -> int:
    for item in values:
        pass
    return item
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_unbound"),
        "the loop target is accepted past the loop, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// An annotated assignment binds exactly like a plain one. (Relocated from
/// the deleted resolver-field test.)
#[test]
fn annotated_assign_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def greet() -> str:
    result: str = 'hello'
    return result
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_unbound"),
        "an annotated assign binds unconditionally, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// `global` names live in an enclosing scope — never "unbound on some path"
/// as far as this function's flow is concerned.
#[test]
fn global_declared_name_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
counter = 0


def bump(flag: bool) -> int:
    global counter
    if flag:
        counter = counter + 1
    return counter
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_unbound"),
        "`global counter` binds in the module scope, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// Nested functions get their own walk — a conditional assign inside a
/// closure fires on the closure's own flow, not the outer function's.
#[test]
fn nested_function_is_analysed() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def outer(flag: bool) -> int:
    def inner() -> int:
        if flag:
            value = 1
        return value

    return inner()
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"names_unbound"),
        "the nested function's conditional assign must fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// A `with` body always executes when the statement is reached.
#[test]
fn with_body_assign_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def read(path: str) -> str:
    with open(path) as handle:
        content = handle.read()
    return content
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_unbound"),
        "the `with` body runs whenever the statement is reached, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// A `match` with a catch-all `case _:` that assigns in every arm is
/// exhaustive; without one, the no-match fallthrough stays live.
#[test]
fn match_without_catchall_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def describe(value: int) -> str:
    match value:
        case 0:
            label = 'zero'
        case 1:
            label = 'one'
    return label
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"names_unbound"),
        "no catch-all case — `label` is unbound when nothing matches, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// The paired positive: a catch-all arm makes the `match` exhaustive.
#[test]
fn match_with_catchall_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def describe(value: int) -> str:
    match value:
        case 0:
            label = 'zero'
        case _:
            label = 'other'
    return label
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_unbound"),
        "the catch-all arm covers every remaining path, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
