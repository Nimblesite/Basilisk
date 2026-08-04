//! Tests for [CHKARCH-DIAG-TYPESAFETY] call collection completeness. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//
// A call is a call wherever it appears. The resolver's call collector fed
// `module.calls` from statement-outermost expressions only, so
// `C(1).method()` silently skipped the SAME constructor-arity error that the
// bare statement `C(1)` reports (Refs #381). These tests pin every expression
// position to the bare-statement behaviour, span included.

use super::common::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A dataclass with one `int` field: `C(1, 2)` is one positional too many,
/// which `dataclasses_kwonly`'s arity check reports on the bare statement.
const CLASS: &str = "from dataclasses import dataclass\n\n@dataclass\nclass C:\n    a: int\n";

/// The arity diagnostics drawn by `source`, as `(code, span)` pairs.
fn arity_spans(source: &str) -> Result<Vec<(String, (u32, u32))>, Box<dyn std::error::Error>> {
    let diags = run(source)?;
    Ok(diags
        .iter()
        .filter(|d| d.message.contains("positional argument"))
        .map(|d| (d.code.code.to_owned(), (d.span.start, d.span.end)))
        .collect())
}

/// `wrapped` must report exactly the arity diagnostic the bare `C(1, 2)`
/// statement reports, anchored at the same place within the `C(1, 2)` call.
///
/// "Same span" is measured RELATIVE to the call text: the bare baseline's
/// span is translated from its `C(1, 2)` occurrence to the wrapped one, so
/// the assertion pins the rule's own anchoring (the offending argument)
/// without hard-coding it.
fn assert_same_arity_error(wrapped_stmt: &str, why: &str) -> TestResult {
    let bare = format!("{CLASS}\nC(1, 2)\n");
    let bare_offset = u32::try_from(bare.find("C(1, 2)").ok_or("bare fixture broken")?)?;
    let bare_errors = arity_spans(&bare)?;
    let (bare_code, (bare_start, bare_end)) = bare_errors
        .first()
        .ok_or("the bare statement must report an arity error to pin against")?;

    let wrapped = format!("{CLASS}\n{wrapped_stmt}\n");
    let offset = u32::try_from(wrapped.find("C(1, 2)").ok_or("fixture must contain C(1, 2)")?)?;
    let expected_span = (
        offset + (bare_start - bare_offset),
        offset + (bare_end - bare_offset),
    );

    let errors = arity_spans(&wrapped)?;
    assert!(
        errors
            .iter()
            .any(|(code, span)| code == bare_code && *span == expected_span),
        "{why}: expected {bare_code} at {expected_span:?}, got: {errors:?}",
    );
    Ok(())
}

#[test]
fn bare_statement_reports_constructor_arity() -> TestResult {
    let errors = arity_spans(&format!("{CLASS}\nC(1, 2)\n"))?;
    assert!(
        !errors.is_empty(),
        "the bare `C(1, 2)` statement is the baseline and must report, got none"
    );
    Ok(())
}

#[test]
fn method_call_receiver_reports_constructor_arity() -> TestResult {
    assert_same_arity_error(
        "C(1, 2).method()",
        "a constructor call does not stop being wrong because a method is called on it (#381)",
    )
}

#[test]
fn call_argument_reports_constructor_arity() -> TestResult {
    assert_same_arity_error(
        "print(C(1, 2))",
        "a constructor call inside an argument list is still a call (#381)",
    )
}

#[test]
fn list_element_reports_constructor_arity() -> TestResult {
    assert_same_arity_error(
        "xs = [C(1, 2)]",
        "a constructor call inside a list literal is still a call (#381)",
    )
}

#[test]
fn conditional_expression_reports_constructor_arity() -> TestResult {
    assert_same_arity_error(
        "p = True\nx = C(1, 2) if p else None",
        "a constructor call inside a conditional expression is still a call (#381)",
    )
}

#[test]
fn correct_constructor_stays_silent_everywhere() -> TestResult {
    let diags = run(&format!(
        "{CLASS}\nok = [C(1)]\nprint(C(2))\ny = C(3) if True else None\n"
    ))?;
    let arity: Vec<_> = diags
        .iter()
        .filter(|d| d.message.contains("positional argument"))
        .collect();
    assert!(
        arity.is_empty(),
        "correct constructor calls must stay silent in every position, got: {:?}",
        arity.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}
