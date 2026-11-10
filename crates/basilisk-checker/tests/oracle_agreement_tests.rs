//! Hover/inlay displays and checker diagnostics answer from the SAME oracle —
//! [NARROWPLAN-INTEGRATION] Step 5, [TYPEINF-TARGET-BIDIRECTIONAL]. See
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-INTEGRATION
//! and `crates/basilisk-checker/src/expr_type.rs`.
//!
//! The display surfaces read [`ModuleSpanTypes`], which wraps the very
//! `ModuleTypes`/`BidirEngine` the rules judge with. This suite proves the
//! agreement *observably*, from the outside: for each fixture it asks the
//! public display oracle what a right-hand side is, and asks the CHECKER
//! whether an assignment of that RHS to that exact type is accepted. A type
//! the hover shows must be one the diagnostics agree with — byte for byte,
//! because there is only one answer to disagree about.
//!
//! This is a real seam, not a tautology: a second inference path for
//! displays (the deleted `infer_rhs`/`collection_inference` tables) would
//! show one type while the rules judged another, and every case below would
//! catch it.
#![allow(clippy::allow_attributes, clippy::expect_used, missing_docs, dead_code)]

mod common;

use basilisk_checker::expr_type::ModuleSpanTypes;
use basilisk_resolver::Span;
use common::run;

/// Byte span of `needle`'s LAST occurrence in `source`.
fn span_of(source: &str, needle: &str) -> Span {
    let start = source
        .rfind(needle)
        .expect("fixture must contain the probe expression");
    Span {
        start: u32::try_from(start).expect("fixture fits in u32"),
        end: u32::try_from(start + needle.len()).expect("fixture fits in u32"),
    }
}

/// What the display surfaces (hover, inlay hints) render for `expression`
/// as a module-level right-hand side.
fn displayed_type(expression: &str) -> Result<String, Box<dyn std::error::Error>> {
    let source = format!("value = {expression}\n");
    let parsed = basilisk_parser::parse_source(source.clone(), "agreement.py".to_owned())?;
    let module = basilisk_resolver::resolve(&parsed)?;
    let types = ModuleSpanTypes::build(&module);
    Ok(types.display_at(span_of(&source, expression)))
}

/// Whether the CHECKER accepts `value: <displayed> = <expression>` — i.e.
/// whether the diagnostics agree with what hover just rendered.
fn checker_accepts(expression: &str, declared: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let source = format!("value: {declared} = {expression}\n");
    let diags = run(&source)?;
    Ok(!diags
        .iter()
        .any(|d| d.code.code == "assignment_compatibility"))
}

/// The core agreement: whatever the display oracle renders for an
/// expression, the checker must accept as that expression's declared type.
/// A display surface that answered from a different inference path would
/// eventually render a type the rules reject — that is exactly the
/// disagreement [NARROWPLAN-INTEGRATION] Step 5 removed.
#[test]
fn displayed_type_is_a_type_the_checker_accepts() -> Result<(), Box<dyn std::error::Error>> {
    for expression in [
        "1",
        "'text'",
        "3.5",
        "True",
        "None",
        "[1, 2, 3]",
        "{'k': 'v'}",
        "{1, 2}",
        "(1, 'two')",
        "[[1], [2]]",
        "{'k': [1, 2]}",
    ] {
        let displayed = displayed_type(expression)?;
        assert!(
            !displayed.is_empty(),
            "the display oracle must render a type for `{expression}`"
        );
        assert!(
            checker_accepts(expression, &displayed)?,
            "hover renders `{displayed}` for `{expression}`, but the checker \
             rejects `value: {displayed} = {expression}` — the display surface \
             and the diagnostics are not reading the same oracle"
        );
    }
    Ok(())
}

/// The paired negative — without it the test above would pass for a display
/// oracle that rendered `Any` (or anything else universally accepted) for
/// everything. A DIFFERENT type must be rejected, so the acceptance above
/// carries information.
#[test]
fn a_disagreeing_type_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    for (expression, wrong) in [("1", "str"), ("'text'", "int"), ("[1, 2]", "list[str]")] {
        assert!(
            !checker_accepts(expression, wrong)?,
            "`value: {wrong} = {expression}` must fire — otherwise the \
             agreement assertion proves nothing"
        );
    }
    Ok(())
}

/// The display oracle and the checker agree about CALL results too — the
/// surface Step 5 opened (the legacy display path could not type a call at
/// all, so hover fell silent while the rules judged it).
#[test]
fn call_results_agree() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def make() -> int:\n    return 1\n\n\nvalue = make()\n";
    let parsed = basilisk_parser::parse_source(source.to_owned(), "agreement.py".to_owned())?;
    let module = basilisk_resolver::resolve(&parsed)?;
    let types = ModuleSpanTypes::build(&module);
    assert_eq!(
        types.display_at(span_of(source, "make()")),
        "int",
        "hover must type a call from the callee's declared return"
    );

    let mismatched = "def make() -> int:\n    return 1\n\n\nvalue: str = make()\n";
    let diags = run(mismatched)?;
    assert!(
        diags
            .iter()
            .any(|d| d.code.code == "assignment_compatibility"),
        "the checker must reject `value: str = make()` — the same `int` hover \
         renders. Got: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

/// PEP 675 provenance survives display widening: a string literal renders
/// `LiteralString`, not `str` — pinning the #290 hover regression that
/// motivated sharing the oracle in the first place.
#[test]
fn literal_string_provenance_survives_display() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(displayed_type("'text'")?, "LiteralString");
    assert!(
        checker_accepts("'text'", "LiteralString")?,
        "the checker must accept the LiteralString hover renders"
    );
    Ok(())
}
