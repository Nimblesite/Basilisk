//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for basilisk-checker.

use basilisk_checker::{check_with_config, Diagnostic, Severity};
use basilisk_config::BasiliskConfig;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

// Basilisk has **no modes** — no `basic`/`standard`/`strict`. The checker does
// exactly what the configuration says: config in, diagnostics out. These three
// helpers are the only ways to run it. See [CHKARCH-CONFIGURATION-ONLY].

/// Run with the default configuration: every PEP rule, no `BSK-`prefixed house
/// rules — the pure-PEP-conformance, out-of-the-box experience.
fn run(src: &str) -> Result<Vec<Diagnostic>, Box<dyn std::error::Error>> {
    run_with_config(src, &BasiliskConfig::default())
}

/// Run honoring an explicit project configuration — exactly what a project's
/// `basilisk.toml` would produce, nothing more.
fn run_with_config(
    src: &str,
    config: &BasiliskConfig,
) -> Result<Vec<Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(src.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check_with_config(&resolved, config))
}

/// Configuration with explicit native severities for the annotation rules.
fn annotation_rules_config() -> BasiliskConfig {
    use basilisk_config::RuleSeverity::{Error, Warning};

    BasiliskConfig::with_rule_entries(
        [
            ("BSK-0001", Error),
            ("BSK-0002", Error),
            ("BSK-0003", Error),
            ("BSK-0004", Error),
            ("BSK-0005", Error),
            ("BSK-0025", Error),
            ("BSK-0014", Warning),
            ("BSK-0040", Warning),
            ("BSK-0050", Warning),
        ]
        .into_iter()
        .map(|(code, severity)| (code.to_owned(), severity))
        .collect(),
    )
}

#[test]
fn no_diagnostics_for_fully_annotated_function() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("def greet(name: str) -> str:\n    return name\n")?;
    assert!(
        diags.is_empty(),
        "fully annotated function should produce no diagnostics"
    );
    Ok(())
}

/// [STUBRES-CUSTOM-TYPESHED] checker-crate kill-test for the config→context flag
/// mapping (`context.rs`: `custom_typeshed_configured = config.typeshed_path.is_some()`).
///
/// `resolve` leaves imports `Unresolved` (path resolution is a separate step),
/// so a stdlib import like `fractions` is surfaced by `imports_unresolved` iff
/// the bundled name-set no longer rescues it — which is exactly when a custom
/// typeshed is configured. Driving `check_with_config` with vs. without
/// `typeshed_path` pins that `config.typeshed_path.is_some()` reaches the rule:
/// a mutant flipping it (`is_none` / `true` / `false`) breaks one of the two
/// assertions. The `imports_unresolved` unit tests set the flag directly and the
/// other checker-crate typeshed tests set `typeshed_path` on `ImportSearchPaths`
/// — none exercise this `BasiliskConfig`→context mapping through the rule.
#[test]
fn custom_typeshed_config_flag_reaches_imports_unresolved() -> Result<(), Box<dyn std::error::Error>>
{
    let src = "from fractions import Fraction\n\nvalue = Fraction(1, 2)\n";

    // No typeshed-path: the bundled name-set rescues the stdlib module, so the
    // import is NOT flagged.
    let default_diags = run(src)?;
    assert!(
        !default_diags
            .iter()
            .any(|d| d.code.code == "imports_unresolved"),
        "without typeshed-path, a bundled stdlib import must not be flagged: {default_diags:?}"
    );

    // With typeshed-path configured, the custom typeshed is canonical for step 3;
    // `fractions` (left Unresolved by `resolve`) is no longer rescued, so
    // `imports_unresolved` fires. The path need not exist — the flag is
    // `typeshed_path.is_some()`, evaluated without touching disk.
    let typeshed_config = BasiliskConfig {
        typeshed_path: Some(std::path::PathBuf::from("/nonexistent/custom-typeshed")),
        ..BasiliskConfig::default()
    };
    let typeshed_diags = run_with_config(src, &typeshed_config)?;
    assert!(
        typeshed_diags
            .iter()
            .any(|d| d.code.code == "imports_unresolved"),
        "with typeshed-path set, an absent stdlib import must surface as imports_unresolved: \
         {typeshed_diags:?}"
    );
    assert!(
        typeshed_diags
            .iter()
            .any(|d| d.message.contains("fractions")),
        "the diagnostic must name the unresolved module: {typeshed_diags:?}"
    );

    Ok(())
}

#[test]
fn emits_e0001_for_missing_parameter_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_with_config(
        "def process(data) -> None:\n    pass\n",
        &annotation_rules_config(),
    )?;
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code.code, "BSK-0001");
    assert_eq!(diags[0].severity, Severity::Error);
    Ok(())
}

#[test]
fn emits_e0002_for_missing_return_annotation() -> Result<(), Box<dyn std::error::Error>> {
    // The returned method-call result is NOT inferable by the current engine,
    // so the annotation is required ([TYPEINF-FUNC-RETURN]). A body that never
    // returns a value would infer `-> None` and stay silent.
    let diags = run_with_config(
        "def process(data: str):\n    return data.upper()\n",
        &annotation_rules_config(),
    )?;
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code.code, "BSK-0002");
    assert_eq!(diags[0].severity, Severity::Error);
    Ok(())
}

#[test]
fn emits_both_for_unannotated_function() -> Result<(), Box<dyn std::error::Error>> {
    // `data` has no default to infer from (BSK-0001) and `return data` is a
    // name reference the current engine cannot infer (BSK-0002) — both fire.
    let diags = run_with_config(
        "def process(data):\n    return data\n",
        &annotation_rules_config(),
    )?;
    assert_eq!(diags.len(), 2, "should emit BSK-0001 and BSK-0002");

    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(codes.contains(&"BSK-0001"));
    assert!(codes.contains(&"BSK-0002"));
    Ok(())
}

#[test]
fn emits_one_e0001_per_unannotated_parameter() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_with_config(
        "def multi(a, b, c) -> None:\n    pass\n",
        &annotation_rules_config(),
    )?;
    let count = diags.iter().filter(|d| d.code.code == "BSK-0001").count();
    assert_eq!(
        count, 3,
        "three unannotated params should produce three E0001s"
    );
    Ok(())
}

#[test]
fn handles_empty_file() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("")?;
    assert!(diags.is_empty());
    Ok(())
}

#[test]
fn all_diagnostics_have_nonempty_message() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_with_config("def bad(x):\n    pass\n", &annotation_rules_config())?;
    for d in &diags {
        assert!(!d.message.is_empty());
    }
    Ok(())
}

#[test]
fn all_diagnostics_have_docs_url() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_with_config("def bad(x):\n    pass\n", &annotation_rules_config())?;
    for d in &diags {
        assert!(d.code.docs_url.starts_with("https://"));
    }
    Ok(())
}

#[test]
fn severity_error_displays_as_error() {
    assert_eq!(format!("{}", Severity::Error), "error");
}

#[test]
fn severity_warning_displays_as_warning() {
    assert_eq!(format!("{}", Severity::Warning), "warning");
}

#[test]
fn severity_error_greater_than_warning() {
    assert!(Severity::Error > Severity::Warning);
}

// ---------------------------------------------------------------------------
// Missing variable type: branch coverage
// ---------------------------------------------------------------------------

#[test]
fn annotated_empty_list_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    // Annotated variable: BSK-0003 must NOT fire (has_annotation = true)
    let diags = run_with_config("items: list[int] = []\n", &annotation_rules_config())?;
    let e3: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0003").collect();
    assert!(
        e3.is_empty(),
        "annotated empty-list variable must not trigger BSK-0003"
    );
    Ok(())
}

#[test]
fn unannotated_str_literal_suppressed() -> Result<(), Box<dyn std::error::Error>> {
    // Unannotated str literal — BSK-0003 must NOT fire (type is trivially `str`)
    let diags = run_with_config("name = \"hello\"\n", &annotation_rules_config())?;
    let e3: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0003").collect();
    assert!(
        e3.is_empty(),
        "str literal should not fire BSK-0003 — type is trivially inferrable, got: {:?}",
        e3.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn fires_for_all_three_unresolvable_rhs_kinds() -> Result<(), Box<dyn std::error::Error>> {
    // Covers EmptyList, EmptyDict, and NoneValue branches in make_diagnostic
    let diags = run_with_config("a = []\nb = {}\nc = None\n", &annotation_rules_config())?;
    let e3: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0003").collect();
    assert_eq!(
        e3.len(),
        3,
        "all three unresolvable kinds must fire BSK-0003"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Argument type mismatch: all match arms
// ---------------------------------------------------------------------------

#[test]
fn bool_param_receives_str_literal() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: bool) -> None: pass\nfoo(\"hello\")\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "calls_argument_type")
        .collect();
    assert_eq!(e12.len(), 1, "bool param + str literal must fire E0012");
    Ok(())
}

#[test]
fn float_param_receives_str_literal() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: float) -> None: pass\nfoo(\"hello\")\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "calls_argument_type")
        .collect();
    assert_eq!(e12.len(), 1, "float param + str literal must fire E0012");
    Ok(())
}

#[test]
fn bytes_param_receives_str_literal() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: bytes) -> None: pass\nfoo(\"hello\")\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "calls_argument_type")
        .collect();
    assert_eq!(e12.len(), 1, "bytes param + str literal must fire E0012");
    Ok(())
}

#[test]
fn int_param_receives_bytes_literal() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: int) -> None: pass\nfoo(b\"raw\")\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "calls_argument_type")
        .collect();
    assert_eq!(e12.len(), 1, "int param + bytes literal must fire E0012");
    Ok(())
}

#[test]
fn str_param_receives_bytes_literal() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: str) -> None: pass\nfoo(b\"raw\")\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "calls_argument_type")
        .collect();
    assert_eq!(e12.len(), 1, "str param + bytes literal must fire E0012");
    Ok(())
}

#[test]
fn float_param_receives_bytes_literal() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: float) -> None: pass\nfoo(b\"raw\")\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "calls_argument_type")
        .collect();
    assert_eq!(e12.len(), 1, "float param + bytes literal must fire E0012");
    Ok(())
}

#[test]
fn int_param_receives_float_literal() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: int) -> None: pass\nfoo(3.14)\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "calls_argument_type")
        .collect();
    assert_eq!(e12.len(), 1, "int param + float literal must fire E0012");
    Ok(())
}

#[test]
fn str_param_receives_float_literal() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: str) -> None: pass\nfoo(3.14)\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "calls_argument_type")
        .collect();
    assert_eq!(e12.len(), 1, "str param + float literal must fire E0012");
    Ok(())
}

#[test]
fn bool_param_receives_float_literal() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: bool) -> None: pass\nfoo(3.14)\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "calls_argument_type")
        .collect();
    assert_eq!(e12.len(), 1, "bool param + float literal must fire E0012");
    Ok(())
}

#[test]
fn str_param_receives_int_literal() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: str) -> None: pass\nfoo(42)\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "calls_argument_type")
        .collect();
    assert_eq!(e12.len(), 1, "str param + int literal must fire E0012");
    Ok(())
}

#[test]
fn bytes_param_receives_int_literal() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: bytes) -> None: pass\nfoo(42)\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "calls_argument_type")
        .collect();
    assert_eq!(e12.len(), 1, "bytes param + int literal must fire E0012");
    Ok(())
}

#[test]
fn compatible_int_arg_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    // int param + int literal: compatible → no E0012
    let src = "def foo(x: int) -> None: pass\nfoo(42)\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "calls_argument_type")
        .collect();
    assert!(e12.is_empty(), "compatible int arg must not fire E0012");
    Ok(())
}

#[test]
fn unknown_callee_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    // Callee not defined in same module — no diagnostic
    let src = "unknown_func(42, \"hello\")\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "calls_argument_type")
        .collect();
    assert!(
        e12.is_empty(),
        "call to unknown function must not fire E0012"
    );
    Ok(())
}

#[test]
fn extra_args_beyond_params_does_not_crash() -> Result<(), Box<dyn std::error::Error>> {
    // More positional args than declared params: checker must handle gracefully (break path)
    let src = "def foo(x: int) -> None: pass\nfoo(1, \"extra\", b\"more\")\n";
    let diags = run(src)?;
    // Only the first arg is checked (x: int, arg=1) — no E0012
    let e12: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "calls_argument_type")
        .collect();
    assert!(
        e12.is_empty(),
        "extra args beyond params must not fire E0012 for out-of-range args"
    );
    Ok(())
}

#[test]
fn unannotated_param_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    // Param has no annotation: E0012 must NOT fire
    let src = "def foo(x) -> None: pass\nfoo(\"hello\")\n";
    let diags = run(src)?;
    let e12: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "calls_argument_type")
        .collect();
    assert!(e12.is_empty(), "unannotated param must not fire E0012");
    Ok(())
}

// ---------------------------------------------------------------------------
// Assignment type mismatch: branch coverage
// ---------------------------------------------------------------------------

#[test]
fn bool_annotation_with_str_literal() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("flag: bool = \"yes\"\n")?;
    let e14: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "assignment_compatibility")
        .collect();
    assert_eq!(e14.len(), 1, "bool: str mismatch must fire E0014");
    Ok(())
}

#[test]
fn float_annotation_with_str_literal() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("ratio: float = \"1.5\"\n")?;
    let e14: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "assignment_compatibility")
        .collect();
    assert_eq!(e14.len(), 1, "float: str mismatch must fire E0014");
    Ok(())
}

#[test]
fn compatible_annotation_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    // int annotation with int literal — compatible
    let diags = run("count: int = 42\n")?;
    let e14: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "assignment_compatibility")
        .collect();
    assert!(e14.is_empty(), "compatible int=int must not fire E0014");
    Ok(())
}

#[test]
fn annotation_at_end_of_file_no_newline() -> Result<(), Box<dyn std::error::Error>> {
    // Line without trailing newline — extract_annotation uses source.len() as line_end
    let diags = run("x: int = \"str\"")?;
    let e14: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "assignment_compatibility")
        .collect();
    assert_eq!(
        e14.len(),
        1,
        "annotation at end of file (no trailing newline) must still fire"
    );
    Ok(())
}

#[test]
fn annotation_without_space_after_colon_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    // `x:str = 42` — colon with no space means `find(": ")` returns None → no E0014
    // This exercises the `?` early-return path in extract_annotation.
    let diags = run("x:str = 42\n")?;
    let e14: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "assignment_compatibility")
        .collect();
    assert!(
        e14.is_empty(),
        "annotation without space after colon must not fire E0014 (unparseable annotation text)"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Invalid type arg count: branch coverage
// ---------------------------------------------------------------------------

#[test]
fn frozenset_with_two_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: frozenset[int, str]) -> None: pass\n";
    let diags = run(src)?;
    let e15: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "callables_annotation")
        .collect();
    assert_eq!(e15.len(), 1, "frozenset[int, str] must fire E0015");
    Ok(())
}

#[test]
fn set_with_two_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: set[int, str]) -> None: pass\n";
    let diags = run(src)?;
    let e15: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "callables_annotation")
        .collect();
    assert_eq!(e15.len(), 1, "set[int, str] must fire E0015");
    Ok(())
}

#[test]
fn dict_with_one_arg() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: dict[str]) -> None: pass\n";
    let diags = run(src)?;
    let e15: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "callables_annotation")
        .collect();
    assert_eq!(e15.len(), 1, "dict[str] (one arg) must fire E0015");
    Ok(())
}

#[test]
fn list_empty_brackets() {
    // `list[]` is invalid Python syntax — the ruff parser rejects it before
    // E0015 can run.  Basilisk correctly reports a parse error for this input.
    let src = "def foo(x: list[]) -> None: pass\n";
    assert!(
        run(src).is_err(),
        "list[] (invalid Python syntax) must be rejected as a parse error"
    );
}

#[test]
fn deeply_nested_source_rejected_not_crashed() {
    // [CHKARCH-ARCH-PARSEDEPTH]: a 5000-deep bracket file overflows the recursive
    // parser/visitors. The parse guard must turn it into a parse error so the
    // whole pipeline (parse -> resolve -> check) returns cleanly instead of
    // aborting the process.
    let src = format!("x = {}1{}\n", "(".repeat(5000), ")".repeat(5000));
    let err = run(&src).expect_err("pathologically nested source must be rejected, not crash");
    assert!(
        err.to_string().contains("too many nested parentheses"),
        "the depth-guard rejection must propagate through the pipeline, got: {err}"
    );
}

#[test]
fn correct_list_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: list[int]) -> None: pass\n";
    let diags = run(src)?;
    let e15: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "callables_annotation")
        .collect();
    assert!(e15.is_empty(), "correct list[int] must not fire E0015");
    Ok(())
}

#[test]
fn param_annotation_without_space_after_colon_does_not_fire(
) -> Result<(), Box<dyn std::error::Error>> {
    // `x:list[int, str]` — no space after colon means `find(": ")` returns None
    // → extract_param_annotation returns None → no E0015 emitted.
    let src = "def foo(x:list[int, str]) -> None: pass\n";
    let diags = run(src)?;
    let e15: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "callables_annotation")
        .collect();
    assert!(
        e15.is_empty(),
        "param annotation without space after colon must not fire E0015"
    );
    Ok(())
}

#[test]
fn vararg_with_invalid_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(*args: list[int, str]) -> None: pass\n";
    let diags = run(src)?;
    let e15: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "callables_annotation")
        .collect();
    assert_eq!(e15.len(), 1, "vararg with list[int, str] must fire E0015");
    Ok(())
}

#[test]
fn kwarg_with_invalid_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(**kwargs: dict[str]) -> None: pass\n";
    let diags = run(src)?;
    let e15: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "callables_annotation")
        .collect();
    assert_eq!(e15.len(), 1, "kwarg with dict[str] must fire E0015");
    Ok(())
}

#[test]
fn nested_generic_correct_count() -> Result<(), Box<dyn std::error::Error>> {
    // dict[list[int], str] has 2 top-level args — must NOT fire
    let src = "def foo(x: dict[list[int], str]) -> None: pass\n";
    let diags = run(src)?;
    let e15: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "callables_annotation")
        .collect();
    assert!(e15.is_empty(), "dict[list[int], str] has correct arg count");
    Ok(())
}

// ---------------------------------------------------------------------------
// Incompatible override: branch coverage
// ---------------------------------------------------------------------------

#[test]
fn different_param_count_fires() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import override\n",
        "class Base:\n",
        "    def method(self: 'Base', x: int) -> None: pass\n",
        "class Child(Base):\n",
        "    @override\n",
        "    def method(self: 'Child') -> None: pass\n",
    );
    let diags = run(src)?;
    let e16: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "classes_override")
        .collect();
    assert_eq!(e16.len(), 1, "different param count must fire E0016");
    Ok(())
}

#[test]
fn only_return_type_differs_fires() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import override\n",
        "class Base:\n",
        "    def method(self: 'Base') -> int: pass\n",
        "class Child(Base):\n",
        "    @override\n",
        "    def method(self: 'Child') -> str: pass\n",
    );
    let diags = run(src)?;
    let e16: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "classes_override")
        .collect();
    assert_eq!(e16.len(), 1, "different return type must fire E0016");
    Ok(())
}

#[test]
fn compatible_override_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import override\n",
        "class Base:\n",
        "    def method(self: 'Base', x: int) -> int: pass\n",
        "class Child(Base):\n",
        "    @override\n",
        "    def method(self: 'Child', x: int) -> int: pass\n",
    );
    let diags = run(src)?;
    let e16: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "classes_override")
        .collect();
    assert!(e16.is_empty(), "compatible override must not fire E0016");
    Ok(())
}

#[test]
fn base_not_in_module_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    // Base class defined externally — cannot check, must not fire
    let src = concat!(
        "from typing import override\n",
        "class Child(SomeExternalBase):\n",
        "    @override\n",
        "    def method(self: 'Child') -> None: pass\n",
    );
    let diags = run(src)?;
    let e16: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "classes_override")
        .collect();
    assert!(e16.is_empty(), "external base must not fire E0016");
    Ok(())
}

#[test]
fn method_without_override_decorator_not_checked() -> Result<(), Box<dyn std::error::Error>> {
    // Method overrides base but has no @override — E0016 must NOT fire (that's BSK-0025)
    let src = concat!(
        "class Base:\n",
        "    def method(self: 'Base', x: int) -> int: pass\n",
        "class Child(Base):\n",
        "    def method(self: 'Child') -> str: pass\n",
    );
    let diags = run(src)?;
    let e16: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "classes_override")
        .collect();
    assert!(
        e16.is_empty(),
        "method without @override must not fire E0016"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Incompatible variable override: branch coverage
// ---------------------------------------------------------------------------

#[test]
fn unannotated_child_attr_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Base:\n",
        "    count: int = 0\n",
        "class Child(Base):\n",
        "    count = 0\n",
    );
    let diags = run(src)?;
    let e17: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "classes_override_2")
        .collect();
    assert!(
        e17.is_empty(),
        "unannotated child attribute must not fire E0017"
    );
    Ok(())
}

#[test]
fn base_attr_not_annotated_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Base:\n",
        "    count = 0\n",
        "class Child(Base):\n",
        "    count: str = \"zero\"\n",
    );
    let diags = run(src)?;
    let e17: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "classes_override_2")
        .collect();
    assert!(
        e17.is_empty(),
        "unannotated base attribute must not fire E0017"
    );
    Ok(())
}

#[test]
fn same_annotation_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Base:\n",
        "    count: int = 0\n",
        "class Child(Base):\n",
        "    count: int = 1\n",
    );
    let diags = run(src)?;
    let e17: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "classes_override_2")
        .collect();
    assert!(e17.is_empty(), "same annotation must not fire E0017");
    Ok(())
}

#[test]
fn attr_only_in_child_not_in_base_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    // Attribute declared in child but NOT in base — not an override, must not fire
    let src = concat!(
        "class Base:\n",
        "    x: int = 0\n",
        "class Child(Base):\n",
        "    y: str = \"new\"\n",
    );
    let diags = run(src)?;
    let e17: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "classes_override_2")
        .collect();
    assert!(
        e17.is_empty(),
        "attr only in child (not base) must not fire E0017"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Missing overload impl: branch coverage
// ---------------------------------------------------------------------------

#[test]
fn single_overload_function_fires() -> Result<(), Box<dyn std::error::Error>> {
    // A lone @overload with no implementation is invalid: the typing spec
    // requires at least two @overload definitions (python/typing conformance
    // `overloads_definitions_stub::func1`).
    let src = "from typing import overload\n@overload\ndef foo(x: int) -> int: ...\n";
    let diags = run(src)?;
    let hits: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "overloads_definitions")
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "a single @overload with no implementation must fire overloads_definitions"
    );
    Ok(())
}

#[test]
fn overloads_with_impl_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import overload\n",
        "@overload\n",
        "def foo(x: int) -> int: ...\n",
        "@overload\n",
        "def foo(x: str) -> str: ...\n",
        "def foo(x: int | str) -> int | str: return x\n",
    );
    let diags = run(src)?;
    let e20: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "overloads_definitions")
        .collect();
    assert!(
        e20.is_empty(),
        "@overload group with impl must not fire E0020"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Overlapping overloads: branch coverage
// ---------------------------------------------------------------------------

#[test]
fn three_overlapping_overloads_emits_one_per_later() -> Result<(), Box<dyn std::error::Error>> {
    // When there are 3 overlapping overloads, only emit one diagnostic per later overload.
    // The `break` in check_group ensures at most one diag per later overload even if it
    // overlaps multiple earlier ones.
    let src = concat!(
        "from typing import overload\n",
        "@overload\n",
        "def foo(x) -> int: ...\n",
        "@overload\n",
        "def foo(x) -> str: ...\n",
        "@overload\n",
        "def foo(x) -> bytes: ...\n",
    );
    let diags = run(src)?;
    let e21: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "overloads_consistency")
        .collect();
    // overload[1] overlaps overload[0], overload[2] overlaps overload[0] → 2 E0021
    assert_eq!(
        e21.len(),
        2,
        "three overlapping overloads must emit two E0021 diagnostics"
    );
    Ok(())
}

#[test]
fn different_param_count_does_not_overlap() -> Result<(), Box<dyn std::error::Error>> {
    // Overloads with different param count cannot overlap
    let src = concat!(
        "from typing import overload\n",
        "@overload\n",
        "def foo(x: int) -> int: ...\n",
        "@overload\n",
        "def foo(x: int, y: int) -> int: ...\n",
    );
    let diags = run(src)?;
    let e21: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "overloads_consistency")
        .collect();
    assert!(e21.is_empty(), "different param count must not fire E0021");
    Ok(())
}

#[test]
fn same_param_count_different_names_does_not_overlap() -> Result<(), Box<dyn std::error::Error>> {
    // Overloads with same param count but DIFFERENT param names are not similar —
    // exercises the `if !names_match { return false; }` branch.
    let src = concat!(
        "from typing import overload\n",
        "@overload\n",
        "def foo(x: int) -> int: ...\n",
        "@overload\n",
        "def foo(y: str) -> str: ...\n",
    );
    let diags = run(src)?;
    let e21: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "overloads_consistency")
        .collect();
    assert!(
        e21.is_empty(),
        "same count but different param names must not fire E0021"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Missing @override: branch coverage
// ---------------------------------------------------------------------------

#[test]
fn method_with_override_decorator_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import override\n",
        "class Base:\n",
        "    def method(self: 'Base') -> None: pass\n",
        "class Child(Base):\n",
        "    @override\n",
        "    def method(self: 'Child') -> None: pass\n",
    );
    let diags = run_with_config(src, &annotation_rules_config())?;
    let e25: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0025").collect();
    assert!(
        e25.is_empty(),
        "method with @override must not fire BSK-0025"
    );
    Ok(())
}

#[test]
fn missing_override_base_not_in_module_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Child(ExternalBase):\n",
        "    def method(self: 'Child') -> None: pass\n",
    );
    let diags = run_with_config(src, &annotation_rules_config())?;
    let missing_override: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0025").collect();
    assert!(
        missing_override.is_empty(),
        "external base class must not fire BSK-0025"
    );
    Ok(())
}

#[test]
fn new_method_not_in_base_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    // Child adds a NEW method not present in base — not an override
    let src = concat!(
        "class Base:\n",
        "    def existing(self: 'Base') -> None: pass\n",
        "class Child(Base):\n",
        "    def brand_new(self: 'Child') -> None: pass\n",
    );
    let diags = run_with_config(src, &annotation_rules_config())?;
    let e25: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0025").collect();
    assert!(
        e25.is_empty(),
        "new method not in base must not fire BSK-0025"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// additional branches
// ---------------------------------------------------------------------------

#[test]
fn override_when_base_has_no_methods_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    // Base class has no methods — base_func lookup returns None → the override
    // is not checked and no E0016 is emitted.  This exercises the
    // `else { continue }` branch when `base_func` is missing.
    let src = concat!(
        "class Base:\n",
        "    x: int = 0\n",
        "class Child(Base):\n",
        "    @override\n",
        "    def process(self: 'Child', data: str) -> str: return data\n",
    );
    let diags = run(src)?;
    let e16: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "classes_override")
        .collect();
    assert!(
        e16.is_empty(),
        "base with no methods must not fire E0016, got: {e16:#?}"
    );
    Ok(())
}

#[test]
fn override_without_self_param_compatible_does_not_fire() -> Result<(), Box<dyn std::error::Error>>
{
    // Both base and child method have no `self` parameter — exercises the
    // `_ => params` arm of `skip_self_param` (first param name is neither
    // "self" nor "cls").
    let src = concat!(
        "class Base:\n",
        "    def process(data: str) -> str: return data\n",
        "class Child(Base):\n",
        "    @override\n",
        "    def process(data: str) -> str: return data\n",
    );
    let diags = run(src)?;
    let e16: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "classes_override")
        .collect();
    assert!(
        e16.is_empty(),
        "compatible override without self must not fire E0016, got: {e16:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// lib.rs: line_has_type_ignore — missed mutants (FnValue→false, BinaryOperator)
// ---------------------------------------------------------------------------

/// `line_has_type_ignore` — `FnValue → false` at lib.rs:22.
/// A diagnostic on a `# type: ignore` line must be suppressed.
#[test]
fn type_ignore_suppresses_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // BSK-0001 on `x` is suppressed by `# type: ignore`
    let diags = run_with_config(
        "def foo(x) -> None:  # type: ignore\n    pass\n",
        &annotation_rules_config(),
    )?;
    let e1: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0001").collect();
    assert!(
        e1.is_empty(),
        "type: ignore must suppress BSK-0001, got: {e1:#?}"
    );
    Ok(())
}

/// `line_has_type_ignore` — `BinaryOperator` `-`/`*` mutants at lib.rs:23.
/// Diagnostics on lines WITHOUT `# type: ignore` must NOT be suppressed.
#[test]
fn type_ignore_does_not_suppress_other_lines() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_with_config(
        "def foo(x) -> None:\n    pass\n",
        &annotation_rules_config(),
    )?;
    let e1: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0001").collect();
    assert!(
        !e1.is_empty(),
        "BSK-0001 must not be suppressed on lines without type: ignore"
    );
    Ok(())
}

/// Multi-line: ignore only suppresses the specific line, not all diagnostics.
#[test]
fn type_ignore_suppresses_only_its_line() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo(x) -> None:  # type: ignore\n",
        "    pass\n",
        "def bar(y) -> None:\n",
        "    pass\n",
    );
    let diags = run_with_config(src, &annotation_rules_config())?;
    let e1: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0001").collect();
    // foo is suppressed, bar is not → exactly 1 BSK-0001
    assert_eq!(
        e1.len(),
        1,
        "only non-ignored line must produce BSK-0001, got: {e1:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// guards.rs: is_stub_context (FnValue→false, BinaryOperator !=)
// ---------------------------------------------------------------------------

/// `is_stub_context` — `FnValue → false` at guards.rs:21.
/// If the function always returned false, Protocol methods would get BSK-0001/BSK-0002.
#[test]
fn guards_protocol_method_no_annotation_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Protocol\n",
        "class Runnable(Protocol):\n",
        "    def run(self) -> None: ...\n",
    );
    let diags = run_with_config(src, &annotation_rules_config())?;
    let e1: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0001").collect();
    assert!(
        e1.is_empty(),
        "Protocol method must not fire BSK-0001, got: {e1:#?}"
    );
    Ok(())
}

/// `is_stub_context` — `!=` mutant at guards.rs:30.
/// `decorators.iter().any(|d| d == "abstractmethod")` — if `==` becomes `!=`
/// it fires on everything except abstractmethod. This test checks abstractmethod
/// IS treated as stub context.
#[test]
fn guards_abstractmethod_is_stub_context() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from abc import abstractmethod, ABC\n",
        "class Base(ABC):\n",
        "    @abstractmethod\n",
        "    def compute(self) -> int:\n",
        "        pass\n",
    );
    let diags = run_with_config(src, &annotation_rules_config())?;
    let e2: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0002").collect();
    assert!(
        e2.is_empty(),
        "@abstractmethod must suppress BSK-0002, got: {e2:#?}"
    );
    Ok(())
}

/// `is_stub_context` — `!=` mutant at guards.rs:37.
/// Protocol class lookup: `find(|c| &c.name == cls_name)` — if `==` becomes `!=`
/// it finds the wrong class. This test verifies the right class is matched.
#[test]
fn guards_protocol_class_name_match() -> Result<(), Box<dyn std::error::Error>> {
    // Two classes: one Protocol, one not. Methods in Protocol must be exempt;
    // methods in the non-Protocol class must NOT be exempt.
    // NotProto.unannotated returns an attribute access — not inferable, so
    // BSK-0002 must fire there (a `pass` body would infer `-> None`).
    let src = concat!(
        "from typing import Protocol\n",
        "class MyProto(Protocol):\n",
        "    def required(self) -> None: ...\n",
        "class NotProto:\n",
        "    def unannotated(self): return self.value\n",
    );
    let diags = run_with_config(src, &annotation_rules_config())?;
    let e2: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0002").collect();
    // NotProto.unannotated has no return annotation → BSK-0002
    assert!(
        !e2.is_empty(),
        "non-Protocol class must fire BSK-0002, got: {e2:#?}"
    );
    // MyProto.required must not fire
    let proto_e2: Vec<_> = e2
        .iter()
        .filter(|d| d.message.contains("required"))
        .collect();
    assert!(
        proto_e2.is_empty(),
        "Protocol method must not fire BSK-0002"
    );
    Ok(())
}

/// `is_enum_class` — `FnValue → false` at guards.rs:47.
/// Enum subclass attributes must NOT fire BSK-0005.
#[test]
fn guards_enum_class_no_e0005() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from enum import Enum\n",
        "class Color(Enum):\n",
        "    RED = 1\n",
        "    GREEN = 2\n",
    );
    let diags = run_with_config(src, &annotation_rules_config())?;
    let e5: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0005").collect();
    assert!(
        e5.is_empty(),
        "Enum class must not fire BSK-0005, got: {e5:#?}"
    );
    Ok(())
}

/// `is_protocol_class` — `FnValue → false` at guards.rs:58.
/// Protocol class is the gateway for several exemptions — it must return true.
#[test]
fn guards_protocol_class_is_detected() -> Result<(), Box<dyn std::error::Error>> {
    // If is_protocol_class always returned false, E0020 would fire on Protocol
    // overload stubs. Verify Protocol overload stubs don't fire E0020.
    let src = concat!(
        "from typing import Protocol, overload\n",
        "class Processor(Protocol):\n",
        "    @overload\n",
        "    def process(self, x: int) -> int: ...\n",
        "    @overload\n",
        "    def process(self, x: str) -> str: ...\n",
    );
    let diags = run(src)?;
    let e20: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "overloads_definitions")
        .collect();
    assert!(
        e20.is_empty(),
        "Protocol overloads must not fire E0020, got: {e20:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// e0005.rs: BinaryOperator || → && (line 33)
// ---------------------------------------------------------------------------

/// `&&` mutant at line 33: class has no annotation AND is not in enum.
/// If `||` becomes `&&`, un-annotated non-enum attrs get suppressed.
/// This test ensures unannotated class attrs with non-inferrable RHS DO fire BSK-0005.
#[test]
fn unannotated_attr_fires() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("class Config:\n", "    debug = some_func()\n",);
    let diags = run_with_config(src, &annotation_rules_config())?;
    let e5: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0005").collect();
    assert!(
        !e5.is_empty(),
        "unannotated class attr with non-inferrable RHS must fire BSK-0005"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// e0013.rs: BinaryOperator || → && (line 37)
// ---------------------------------------------------------------------------

/// `&&` mutant at line 37 in `check_function`.
/// E0013 fires for `-> None` functions that return a value.
/// The `&&` mutant would suppress when `has_value && !value_is_call` is changed.
/// Test both sides: a valued non-call return fires; a call return does not.
#[test]
fn none_annotated_with_valued_return_fires() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def get_zero() -> None:\n",
        "    return 0\n", // has_value=true, value_is_call=false → should fire
    );
    let diags = run(src)?;
    let e13: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "returns_compatibility_2")
        .collect();
    assert!(!e13.is_empty(), "-> None with `return 0` must fire E0013");
    Ok(())
}

/// `&&` mutant: `!value_is_call` side.
/// `return f()` inside `-> None` must NOT fire because we can't prove the callee
/// returns non-None without full inference.
#[test]
fn none_annotated_with_call_return_does_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def helper() -> None: pass\n",
        "def wrapper() -> None:\n",
        "    return helper()\n", // value_is_call=true → must not fire
    );
    let diags = run(src)?;
    let e13: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "returns_compatibility_2")
        .collect();
    assert!(
        e13.is_empty(),
        "-> None with `return call()` must not fire E0013, got: {e13:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// e0017.rs: is_typed_dict_hierarchy (FnValue→false), check_class &&, uses_typed_dict_qualifier
// ---------------------------------------------------------------------------

/// `is_typed_dict_hierarchy` — `FnValue → false` at e0017.rs:29.
/// `TypedDict` subclasses must NOT fire E0017.
#[test]
fn typed_dict_hierarchy_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Base(TypedDict):\n",
        "    x: int\n",
        "class Child(Base):\n",
        "    x: str\n", // would be E0017 if not exempted
    );
    let diags = run(src)?;
    let e17: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "classes_override_2")
        .collect();
    assert!(
        e17.is_empty(),
        "TypedDict hierarchy must not fire E0017, got: {e17:#?}"
    );
    Ok(())
}

/// E0017 `check_class` — `&&` → `||` mutant at line 72.
/// The rule fires only when child and base BOTH have annotations AND they differ.
/// When annotations match, no diagnostic.
#[test]
fn matching_annotations_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Base:\n",
        "    count: int = 0\n",
        "class Child(Base):\n",
        "    count: int = 1\n", // same annotation — no E0017
    );
    let diags = run(src)?;
    let e17: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "classes_override_2")
        .collect();
    assert!(
        e17.is_empty(),
        "matching annotations must not fire E0017, got: {e17:#?}"
    );
    Ok(())
}

/// E0017 `check_class` — `&&` → `||` mutant at line 126.
/// When only the child has an annotation but the base does not, no E0017.
#[test]
fn base_unannotated_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Base:\n",
        "    count = 0\n", // no annotation
        "class Child(Base):\n",
        "    count: int = 0\n",
    );
    let diags = run(src)?;
    let e17: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "classes_override_2")
        .collect();
    assert!(
        e17.is_empty(),
        "base without annotation must not fire E0017, got: {e17:#?}"
    );
    Ok(())
}

/// `uses_typed_dict_qualifier` — `FnValue → false` at e0017.rs:151.
/// An attr annotated with `ReadOnly[int]` must not fire E0017.
#[test]
fn readonly_qualifier_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import ReadOnly\n",
        "class Base:\n",
        "    x: int = 0\n",
        "class Child(Base):\n",
        "    x: ReadOnly[int] = 0\n",
    );
    let diags = run(src)?;
    let e17: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "classes_override_2")
        .collect();
    assert!(
        e17.is_empty(),
        "ReadOnly qualifier must exempt from E0017, got: {e17:#?}"
    );
    Ok(())
}

/// `uses_typed_dict_qualifier` — `&&` → `||` mutants at lines 153/154.
/// `Required` in annotation also exempts.
#[test]
fn required_qualifier_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Required\n",
        "class Base:\n",
        "    x: int = 0\n",
        "class Child(Base):\n",
        "    x: Required[int] = 0\n",
    );
    let diags = run(src)?;
    let e17: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "classes_override_2")
        .collect();
    assert!(
        e17.is_empty(),
        "Required qualifier must exempt from E0017, got: {e17:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// e0020.rs: BinaryOperator && and != mutants (lines 37, 40)
// ---------------------------------------------------------------------------

/// E0020 `check` — `&&` → `||` mutant at line 37 (`exempt_classes` filter).
/// ABC class methods with @overload + no impl must NOT fire E0020.
#[test]
fn abc_class_overload_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from abc import ABC, abstractmethod\n",
        "from typing import overload\n",
        "class Shape(ABC):\n",
        "    @overload\n",
        "    @abstractmethod\n",
        "    def area(self, scale: int) -> int: ...\n",
        "    @overload\n",
        "    @abstractmethod\n",
        "    def area(self, scale: float) -> float: ...\n",
    );
    let diags = run(src)?;
    let e20: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "overloads_definitions")
        .collect();
    assert!(
        e20.is_empty(),
        "ABC @abstractmethod overloads must not fire E0020, got: {e20:#?}"
    );
    Ok(())
}

/// `&&` → `||` and `!=` mutants at line 40 (`overloaded.len()` < 2).
/// A lone `@overload` with an implementation present fires E0020 (single overload).
#[test]
fn single_overload_with_impl_fires() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import overload\n",
        "class Calc:\n",
        "    @overload\n",
        "    def add(self, x: int) -> int: ...\n",
        "    def add(self, x):\n",
        "        return x\n",
    );
    let diags = run(src)?;
    let e20: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "overloads_definitions")
        .collect();
    assert!(
        !e20.is_empty(),
        "single @overload with impl must fire E0020"
    );
    Ok(())
}

/// 2+ @overloads with implementation must NOT fire E0020.
/// Kills `!=` (count >= 2 check) and `&&` mutants.
#[test]
fn two_overloads_with_impl_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import overload\n",
        "class Calc:\n",
        "    @overload\n",
        "    def add(self, x: int) -> int: ...\n",
        "    @overload\n",
        "    def add(self, x: str) -> str: ...\n",
        "    def add(self, x):\n",
        "        return x\n",
    );
    let diags = run(src)?;
    let e20: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "overloads_definitions")
        .collect();
    assert!(
        e20.is_empty(),
        "2 @overloads + impl must not fire E0020, got: {e20:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// e0021.rs: signatures_overlap BinaryOperator && and UnaryOperator mutants
// ---------------------------------------------------------------------------

/// `signatures_overlap` — `&&` → `||` mutant at line 90.
/// Two overloads with DIFFERENT parameter counts must NOT overlap.
#[test]
fn different_param_count_no_overlap() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import overload\n",
        "@overload\n",
        "def f(x: int) -> int: ...\n",
        "@overload\n",
        "def f(x: int, y: int) -> int: ...\n",
        "def f(*args): pass\n",
    );
    let diags = run(src)?;
    let e21: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "overloads_consistency")
        .collect();
    assert!(
        e21.is_empty(),
        "different param count must not fire E0021, got: {e21:#?}"
    );
    Ok(())
}

/// `signatures_overlap` — `!` to empty (`UnaryOperator` remove) mutants at lines 93/94.
/// When all non-self/cls params ARE annotated on both sides, they DON'T overlap.
#[test]
fn both_fully_annotated_no_overlap() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import overload\n",
        "@overload\n",
        "def process(x: int) -> int: ...\n",
        "@overload\n",
        "def process(x: str) -> str: ...\n",
        "def process(x): return x\n",
    );
    let diags = run(src)?;
    let e21: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "overloads_consistency")
        .collect();
    assert!(
        e21.is_empty(),
        "fully annotated overloads must not fire E0021, got: {e21:#?}"
    );
    Ok(())
}

/// `signatures_overlap` — `&&` → `||` mutant at line 98.
/// Two overloads with same name, count, AND unannotated params DO overlap.
#[test]
fn same_names_unannotated_overlaps() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import overload\n",
        "@overload\n",
        "def f(x) -> int: ...\n",
        "@overload\n",
        "def f(x) -> str: ...\n",
        "def f(x): return x\n",
    );
    let diags = run(src)?;
    let e21: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "overloads_consistency")
        .collect();
    assert!(!e21.is_empty(), "same unannotated params must fire E0021");
    Ok(())
}

// ---------------------------------------------------------------------------
// e0025.rs: is_protocol_transitively (FnValue→false), check_class (||→&&), method_has_decorator
// ---------------------------------------------------------------------------

/// `is_protocol_transitively` — `FnValue → false` at e0025.rs:77.
/// A class implementing a transitive Protocol must not fire BSK-0025.
#[test]
fn transitive_protocol_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Protocol\n",
        "class Base(Protocol):\n",
        "    def run(self) -> None: ...\n",
        "class Extended(Base):\n", // Extended is also protocol-like
        "    def run(self) -> None: ...\n",
        "    def stop(self) -> None: ...\n",
        "class Impl(Extended):\n",
        "    def run(self) -> None: pass\n",
        "    def stop(self) -> None: pass\n",
    );
    let diags = run_with_config(src, &annotation_rules_config())?;
    let e25: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0025").collect();
    assert!(
        e25.is_empty(),
        "Protocol implementation must not fire BSK-0025, got: {e25:#?}"
    );
    Ok(())
}

/// BSK-0025 `check_class` — `||` → `&&` mutant at line 126.
/// An unrelated class (no base methods) must NOT fire BSK-0025.
#[test]
fn no_base_methods_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Standalone:\n",
        "    def compute(self) -> int:\n",
        "        return 42\n",
    );
    let diags = run_with_config(src, &annotation_rules_config())?;
    let e25: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0025").collect();
    assert!(
        e25.is_empty(),
        "class with no base must not fire BSK-0025, got: {e25:#?}"
    );
    Ok(())
}

/// `method_has_decorator` — `FnValue → false` at e0025.rs:170.
/// If always false, methods WITH @override would still fire BSK-0025.
#[test]
fn override_decorator_suppresses() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import override\n",
        "class Base:\n",
        "    def compute(self) -> int:\n",
        "        return 0\n",
        "class Child(Base):\n",
        "    @override\n",
        "    def compute(self) -> int:\n",
        "        return 1\n",
    );
    let diags = run_with_config(src, &annotation_rules_config())?;
    let e25: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0025").collect();
    assert!(
        e25.is_empty(),
        "@override must suppress BSK-0025, got: {e25:#?}"
    );
    Ok(())
}

/// `method_has_decorator` — `!=` mutant at e0025.rs:172.
/// The `name == method_name` filter: if `==` becomes `!=`, the wrong method's
/// decorators are checked. Verify correct method name matching.
#[test]
fn override_on_different_method_does_not_suppress_other() -> Result<(), Box<dyn std::error::Error>>
{
    let src = concat!(
        "from typing import override\n",
        "class Base:\n",
        "    def a(self) -> int: return 0\n",
        "    def b(self) -> int: return 0\n",
        "class Child(Base):\n",
        "    @override\n",
        "    def a(self) -> int: return 1\n",
        "    def b(self) -> int: return 1\n", // no @override — should fire BSK-0025
    );
    let diags = run_with_config(src, &annotation_rules_config())?;
    let e25: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0025").collect();
    assert!(
        !e25.is_empty(),
        "method without @override must fire BSK-0025"
    );
    let messages: Vec<&str> = e25.iter().map(|d| d.message.as_str()).collect();
    assert!(
        messages.iter().any(|m| m.contains('b')),
        "BSK-0025 must point to method b, not a"
    );
    Ok(())
}

/// `method_has_decorator` — `&&` → `||` mutant at e0025.rs:174.
/// `d == decorator || d.ends_with(".decorator")` — the qualified form must also match.
#[test]
fn qualified_override_suppresses() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "import typing\n",
        "class Base:\n",
        "    def go(self) -> None: pass\n",
        "class Child(Base):\n",
        "    @typing.override\n",
        "    def go(self) -> None: pass\n",
    );
    let diags = run_with_config(src, &annotation_rules_config())?;
    let e25: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0025").collect();
    assert!(
        e25.is_empty(),
        "@typing.override must suppress BSK-0025, got: {e25:#?}"
    );
    Ok(())
}

/// `method_has_decorator` — `!=` mutant at e0025.rs:174 (`ends_with` check).
/// A decorator with a different name must NOT suppress BSK-0025.
#[test]
fn unrelated_decorator_does_not_suppress() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Base:\n",
        "    def go(self) -> None: pass\n",
        "class Child(Base):\n",
        "    def go(self) -> None: pass\n", // no @override at all
    );
    let diags = run_with_config(src, &annotation_rules_config())?;
    let e25: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0025").collect();
    assert!(
        !e25.is_empty(),
        "method override without @override must fire BSK-0025"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// e0026.rs: FnValue→() and != mutants (lines 23/24)
// ---------------------------------------------------------------------------

/// `FnValue → ()` at line 23: rule must emit for `constraint_count` == 1.
#[test]
fn single_constraint_fires() -> Result<(), Box<dyn std::error::Error>> {
    let src = "T = TypeVar('T', int)\n";
    let diags = run(src)?;
    let e26: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_basic")
        .collect();
    assert!(!e26.is_empty(), "TypeVar with 1 constraint must fire E0026");
    Ok(())
}

/// `!=` mutant at line 24: `constraint_count` == 1 must fire, count == 2 must not.
#[test]
fn two_constraints_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let src = "T = TypeVar('T', int, str)\n";
    let diags = run(src)?;
    let e26: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_basic")
        .collect();
    assert!(
        e26.is_empty(),
        "TypeVar with 2 constraints must not fire E0026"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// e0027.rs: FnValue→() (line 22)
// ---------------------------------------------------------------------------

/// `FnValue → ()` at line 22: rule must detect duplicate `TypeVar` in Generic.
#[test]
fn duplicate_typevar_fires() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypeVar, Generic\n",
        "T = TypeVar('T')\n",
        "class Box(Generic[T, T]):\n",
        "    pass\n",
    );
    let diags = run(src)?;
    let e27: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_base_class")
        .collect();
    assert!(
        !e27.is_empty(),
        "duplicate TypeVar in Generic must fire E0027"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// e0029.rs: FnValue→(), ||→&&, !=  (lines 22, 35)
// ---------------------------------------------------------------------------

/// `FnValue → ()` at line 22: rule must fire for method in `TypedDict`.
#[test]
fn method_in_typed_dict_fires() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Config(TypedDict):\n",
        "    x: int\n",
        "    def helper(self) -> None: pass\n",
    );
    let diags = run(src)?;
    let e29: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_class_syntax")
        .collect();
    assert!(!e29.is_empty(), "method in TypedDict must fire E0029");
    Ok(())
}

/// `||` → `&&` at line 35 (__`init_subclass`__ / __`class_getitem`__ filter).
/// These synthesised dunders must NOT fire E0029.
#[test]
fn init_subclass_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Config(TypedDict):\n",
        "    x: int\n",
    );
    let diags = run(src)?;
    let e29: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_class_syntax")
        .collect();
    assert!(
        e29.is_empty(),
        "TypedDict with only fields must not fire E0029, got: {e29:#?}"
    );
    Ok(())
}

/// `!=` mutant at line 35: `method_name` must equal the exempted string exactly.
/// A method named `custom_method` must fire; `__init_subclass__` must not.
#[test]
fn only_exempt_dunders_are_skipped() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Config(TypedDict):\n",
        "    x: int\n",
        "    def validate(self) -> bool: return True\n",
    );
    let diags = run(src)?;
    let e29: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_class_syntax")
        .collect();
    assert!(
        !e29.is_empty(),
        "non-exempt method in TypedDict must fire E0029"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// e0030.rs: FnValue→() (line 25)
// ---------------------------------------------------------------------------

/// `FnValue → ()` at line 25: non-default after default `TypeVar` must fire.
#[test]
fn non_default_after_default_fires() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypeVar, Generic\n",
        "T = TypeVar('T', default=int)\n",
        "S = TypeVar('S')\n",          // no default
        "class Box(Generic[T, S]):\n", // T has default, S does not → E0030
        "    pass\n",
    );
    let diags = run(src)?;
    let e30: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_defaults")
        .collect();
    assert!(
        !e30.is_empty(),
        "non-default TypeVar after default must fire E0030"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// e0031.rs: FnValue→(), != (lines 26)
// ---------------------------------------------------------------------------

/// `FnValue → ()` at line 26: wrong arg count must fire.
#[test]
fn wrong_arg_count_fires() -> Result<(), Box<dyn std::error::Error>> {
    let src = "x = cast(int)\n";
    let diags = run(src)?;
    let e31: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "directives_cast")
        .collect();
    assert!(
        !e31.is_empty(),
        "cast() with wrong arg count must fire E0031"
    );
    Ok(())
}

/// `!=` mutant at line 26: cast(int, val) with exactly 2 args must NOT fire
/// (unless first arg is a literal).
#[test]
fn valid_cast_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let src = "x = 42\ny = cast(int, x)\n";
    let diags = run(src)?;
    let e31: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "directives_cast")
        .collect();
    assert!(
        e31.is_empty(),
        "valid cast(int, x) must not fire E0031, got: {e31:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// e0032.rs: FnValue→(), UnaryOperator remove (lines 28, 30)
// ---------------------------------------------------------------------------

/// `FnValue → ()` at line 28: unknown keyword in `TypedDict` must fire.
#[test]
fn unknown_keyword_fires() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Config(TypedDict, metaclass=type):\n",
        "    x: int\n",
    );
    let diags = run(src)?;
    let e32: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_class_syntax_2")
        .collect();
    assert!(
        !e32.is_empty(),
        "unknown keyword in TypedDict must fire E0032"
    );
    Ok(())
}

/// `!` to empty (`UnaryOperator` remove) at line 30.
/// A known keyword (`total`) must NOT fire.
#[test]
fn known_keyword_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Config(TypedDict, total=False):\n",
        "    x: int\n",
    );
    let diags = run(src)?;
    let e32: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_class_syntax_2")
        .collect();
    assert!(
        e32.is_empty(),
        "total= keyword must not fire E0032, got: {e32:#?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// e0035.rs: annotation_text (FnValue→Some("xyzzy")), is_in_typed_dict_hierarchy
//           (FnValue→true / FnValue→false), check FnValue→()
// ---------------------------------------------------------------------------

/// `FnValue → ()` at line 93: rule must actually emit diagnostics.
/// `Required` outside `TypedDict` must fire E0035.
#[test]
fn required_outside_typed_dict_fires() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Required\n",
        "class NotADict:\n",
        "    x: Required[int] = 0\n",
    );
    let diags = run(src)?;
    let e35: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_required")
        .collect();
    assert!(
        !e35.is_empty(),
        "Required outside TypedDict must fire E0035"
    );
    Ok(())
}

/// `NotRequired` outside `TypedDict` must also fire.
#[test]
fn not_required_outside_typed_dict_fires() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import NotRequired\n",
        "class Regular:\n",
        "    y: NotRequired[str] = ''\n",
    );
    let diags = run(src)?;
    let e35: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_required")
        .collect();
    assert!(
        !e35.is_empty(),
        "NotRequired outside TypedDict must fire E0035"
    );
    Ok(())
}

/// `is_in_typed_dict_hierarchy` `FnValue → true` at line 78.
/// If always true, the rule would only check for NESTED usage even outside `TypedDict`.
/// A non-`TypedDict` class with Required must still fire E0035.
#[test]
fn non_typed_dict_class_not_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Required\n",
        "class Config:\n",
        "    x: Required[int] = 0\n",
    );
    let diags = run(src)?;
    let e35: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_required")
        .collect();
    assert!(
        !e35.is_empty(),
        "non-TypedDict class must fire E0035 for Required, not be exempt"
    );
    Ok(())
}

/// `is_in_typed_dict_hierarchy` `FnValue → false` at line 78.
/// If always false, `TypedDict` fields with Required would fire even though they're valid.
/// Valid `TypedDict` usage with Required must NOT fire.
#[test]
fn required_inside_typed_dict_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict, Required\n",
        "class Config(TypedDict):\n",
        "    x: Required[int]\n",
    );
    let diags = run(src)?;
    let e35: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_required")
        .collect();
    assert!(
        e35.is_empty(),
        "Required inside TypedDict must not fire E0035, got: {e35:#?}"
    );
    Ok(())
}

/// `annotation_text` `FnValue → Some("xyzzy")` at line 51.
/// If `annotation_text` always returned `Some("xyzzy")`, "xyzzy" doesn't contain
/// "Required[" so no E0035 would fire on ANYTHING.
/// This test verifies that an attribute with Required IS detected.
#[test]
fn annotation_text_reads_actual_source() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Required\n",
        "class Bad:\n",
        "    field: Required[int] = 0\n",
    );
    let diags = run(src)?;
    let e35: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_required")
        .collect();
    // If annotation_text returned "xyzzy", has_required_wrapper("xyzzy") = false → no E0035.
    assert!(
        !e35.is_empty(),
        "annotation_text must read real source to detect Required"
    );
    Ok(())
}

/// transitive `TypedDict` hierarchy: child of `TypedDict` must also be exempt.
/// Tests `is_in_typed_dict_hierarchy` recursion.
#[test]
fn transitive_typed_dict_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict, Required, NotRequired\n",
        "class Base(TypedDict):\n",
        "    x: int\n",
        "class Child(Base):\n",
        "    y: NotRequired[str]\n", // child inherits TypedDict — must be exempt
    );
    let diags = run(src)?;
    let e35: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_required")
        .collect();
    assert!(
        e35.is_empty(),
        "NotRequired in TypedDict subclass must not fire E0035, got: {e35:#?}"
    );
    Ok(())
}

/// nested Required inside `TypedDict` MUST fire.
/// Exercises the `in_typed_dict` branch: `has_nested_required` check.
#[test]
fn nested_required_in_typed_dict_fires() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict, Required\n",
        "class Config(TypedDict):\n",
        "    x: Required[Required[int]]\n",
    );
    let diags = run(src)?;
    let e35: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_required")
        .collect();
    assert!(
        !e35.is_empty(),
        "nested Required inside TypedDict must fire E0035"
    );
    Ok(())
}

/// Required on a function parameter must fire.
#[test]
fn required_on_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Required\n",
        "def func(x: Required[int]) -> None: pass\n",
    );
    let diags = run(src)?;
    let e35: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "typeddicts_required")
        .collect();
    assert!(
        !e35.is_empty(),
        "Required on function param must fire E0035"
    );
    Ok(())
}

#[test]
fn debug_e0045_qualifiers_annotated() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"from typing import Annotated, Any, TypeVar

T = TypeVar("T")
var1 = 1

Bad1: Annotated[[int, str], ""]
Bad2: Annotated[((int, str),), ""]
Bad3: Annotated[[int for i in range(1)], ""]
Bad4: Annotated[{"a": "b"}, ""]
Bad5: Annotated[(lambda: int)(), ""]
Bad6: Annotated[[int][0], ""]
Bad7: Annotated[int if 1 < 3 else str, ""]
Bad8: Annotated[var1, ""]
Bad9: Annotated[True, ""]
Bad10: Annotated[1, ""]
Bad11: Annotated[list or set, ""]
Bad12: Annotated[f"{'int'}", ""]
Bad13: Annotated[int]
"#;
    let diags = run(src)?;
    let e45: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "qualifiers_annotated")
        .collect();
    eprintln!("E0045 diagnostics: {}", e45.len());
    for d in &e45 {
        let line = src[..d.span.start as usize]
            .chars()
            .filter(|&c| c == '\n')
            .count()
            + 1;
        eprintln!("  Line {}: {}", line, d.message);
    }
    Ok(())
}

#[test]
fn debug_e0047_qualifiers_annotated_fp() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_parser::parse_file;
    use basilisk_resolver::resolve;
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/tests/qualifiers_annotated.py"
    );
    if !std::path::Path::new(path).exists() {
        eprintln!("Skipping: conformance file not present (local-only)");
        return Ok(());
    }
    let parsed = parse_file(path).map_err(|e| format!("{e:?}"))?;
    let resolved = resolve(&parsed)?;
    let diags = basilisk_checker::check(&resolved);
    let src = &resolved.source;
    for d in &diags {
        let line = src[..d.span.start as usize]
            .chars()
            .filter(|&c| c == '\n')
            .count()
            + 1;
        eprintln!("Line {}: {} - {}", line, d.code.code, d.message);
    }
    Ok(())
}

#[test]
fn debug_all_diags_qualifiers_annotated() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_parser::parse_file;
    use basilisk_resolver::resolve;
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/tests/qualifiers_annotated.py"
    );
    if !std::path::Path::new(path).exists() {
        eprintln!("Skipping: conformance file not present (local-only)");
        return Ok(());
    }
    let parsed = parse_file(path).map_err(|e| format!("{e:?}"))?;
    let resolved = resolve(&parsed)?;
    let diags = basilisk_checker::check(&resolved);
    let src = &resolved.source;
    for d in &diags {
        let line = src[..d.span.start as usize]
            .chars()
            .filter(|&c| c == '\n')
            .count()
            + 1;
        eprintln!("Line {}: {} - {}", line, d.code.code, d.message);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// §4.4 — Self/Cls Inference: BSK-0001 does NOT fire for self/cls parameters
// ---------------------------------------------------------------------------

#[test]
fn self_param_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let src = "class MyClass:\n    def method(self) -> None:\n        pass\n";
    let diags = run_with_config(src, &annotation_rules_config())?;
    let e1: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0001").collect();
    assert!(
        e1.is_empty(),
        "self parameter must not trigger BSK-0001, got: {e1:#?}"
    );
    Ok(())
}

#[test]
fn cls_param_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let src = "class MyClass:\n    @classmethod\n    def method(cls) -> None:\n        pass\n";
    let diags = run_with_config(src, &annotation_rules_config())?;
    let e1: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0001").collect();
    assert!(
        e1.is_empty(),
        "cls parameter must not trigger BSK-0001, got: {e1:#?}"
    );
    Ok(())
}

#[test]
fn regular_params_still_fire() -> Result<(), Box<dyn std::error::Error>> {
    let src = "class MyClass:\n    def method(self, data) -> None:\n        pass\n";
    let diags = run_with_config(src, &annotation_rules_config())?;
    let e1: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0001").collect();
    assert!(
        !e1.is_empty(),
        "regular unannotated parameters must still fire BSK-0001"
    );
    // Should only fire for 'data', not 'self'
    let messages: Vec<&str> = e1.iter().map(|d| d.message.as_str()).collect();
    assert!(
        messages.iter().any(|m| m.contains("data")),
        "BSK-0001 should point to 'data' parameter"
    );
    assert!(
        !messages.iter().any(|m| m.contains("self")),
        "BSK-0001 should NOT point to 'self' parameter"
    );
    Ok(())
}

#[test]
fn self_and_cls_do_not_fire_e0001() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class MyClass:\n",
        "    def instance_method(self) -> None:\n",
        "        pass\n",
        "    @classmethod\n",
        "    def class_method(cls) -> None:\n",
        "        pass\n",
        "    def regular_method(self, data) -> None:\n",
        "        pass\n"
    );
    let diags = run_with_config(src, &annotation_rules_config())?;
    let e1: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0001").collect();

    // Should only fire for 'data', not 'self' or 'cls'
    assert!(!e1.is_empty(), "should have BSK-0001 diagnostics");

    let messages: Vec<&str> = e1.iter().map(|d| d.message.as_str()).collect();
    assert!(
        messages.iter().any(|m| m.contains("data")),
        "BSK-0001 should point to 'data' parameter"
    );
    assert!(
        !messages.iter().any(|m| m.contains("self")),
        "BSK-0001 should NOT point to 'self' parameter"
    );
    assert!(
        !messages.iter().any(|m| m.contains("cls")),
        "BSK-0001 should NOT point to 'cls' parameter"
    );

    // Should have exactly 1 BSK-0001 (for 'data')
    let data_e1: Vec<_> = e1.iter().filter(|d| d.message.contains("data")).collect();
    assert_eq!(
        data_e1.len(),
        1,
        "should have exactly 1 BSK-0001 for 'data' parameter"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Return type mismatch: branch coverage
// ---------------------------------------------------------------------------

#[test]
fn int_return_for_str_annotation_fires() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> str:\n    return 42\n";
    let diags = run(src)?;
    let e11: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "returns_compatibility")
        .collect();
    assert!(
        !e11.is_empty(),
        "int return for str annotation must fire E0011"
    );
    Ok(())
}

#[test]
fn str_return_for_int_annotation_fires() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> int:\n    return \"hello\"\n";
    let diags = run(src)?;
    let e11: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "returns_compatibility")
        .collect();
    assert!(
        !e11.is_empty(),
        "str return for int annotation must fire E0011"
    );
    Ok(())
}

#[test]
fn compatible_return_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> int:\n    return 42\n";
    let diags = run(src)?;
    let e11: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "returns_compatibility")
        .collect();
    assert!(
        e11.is_empty(),
        "compatible int return for int annotation must not fire E0011"
    );
    Ok(())
}

#[test]
fn call_return_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def helper() -> int: return 42\ndef foo() -> str:\n    return helper()\n";
    let diags = run(src)?;
    let e11: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "returns_compatibility")
        .collect();
    assert!(
        e11.is_empty(),
        "call return without full inference must not fire E0011"
    );
    Ok(())
}

#[test]
fn unannotated_return_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo():\n    return 42\n";
    let diags = run(src)?;
    let e11: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "returns_compatibility")
        .collect();
    assert!(e11.is_empty(), "unannotated return must not fire E0011");
    Ok(())
}

// ---------------------------------------------------------------------------
// Lambda function missing type annotations
// ---------------------------------------------------------------------------

#[test]
fn lambda_assigned_to_unannotated_var_fires() -> Result<(), Box<dyn std::error::Error>> {
    let src = "f = lambda x: x + 1\n";
    let diags = run_with_config(src, &annotation_rules_config())?;
    let w40: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0040").collect();
    assert!(
        !w40.is_empty(),
        "lambda assigned to unannotated variable must fire BSK-0040"
    );
    assert_eq!(
        w40[0].severity,
        Severity::Warning,
        "BSK-0040 must be a warning, not an error"
    );
    Ok(())
}

#[test]
fn lambda_assigned_to_annotated_var_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let src = "f: Callable[[int], int] = lambda x: x + 1\n";
    let diags = run_with_config(src, &annotation_rules_config())?;
    let w40: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0040").collect();
    assert!(
        w40.is_empty(),
        "lambda assigned to annotated variable must not fire BSK-0040"
    );
    Ok(())
}

#[test]
fn lambda_class_attribute_fires() -> Result<(), Box<dyn std::error::Error>> {
    let src = "class Config:\n    handler = lambda x: x + 1\n";
    let diags = run_with_config(src, &annotation_rules_config())?;
    let w40: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0040").collect();
    assert!(
        !w40.is_empty(),
        "lambda assigned to unannotated class attribute must fire BSK-0040"
    );
    Ok(())
}

#[test]
fn annotated_class_attribute_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let src = "class Config:\n    handler: Callable[[int], int] = lambda x: x + 1\n";
    let diags = run_with_config(src, &annotation_rules_config())?;
    let w40: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0040").collect();
    assert!(
        w40.is_empty(),
        "lambda assigned to annotated class attribute must not fire BSK-0040"
    );
    Ok(())
}
