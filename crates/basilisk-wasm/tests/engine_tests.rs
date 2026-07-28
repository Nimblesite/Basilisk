//! Verifies [WASM-TESTING]. See docs/specs/WASM-SPEC.md#WASM-TESTING
//!
//! Exercises `crates/basilisk-wasm/src/engine.rs`, `src/options.rs` and
//! `src/report.rs` on the host target, so the browser engine is covered by the
//! normal `make test` gate without a wasm runtime in the loop.
//!
//! Every expectation below was first captured from the real `basilisk check
//! --output json` binary on the same source, so these tests pin the engine to
//! the CLI's actual answer rather than to an assumption about it.
#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "a test asserts by failing loudly; the repo's other integration suites allow the same two"
)]

use basilisk_wasm::{check_json, check_source, CheckOptions, Report, WasmDiagnostic};

/// The rule that reports an import Basilisk cannot resolve.
///
/// `third_party_import_is_unresolved` asserts this code IS emitted and
/// `stdlib_import_resolves_from_the_embedded_bundle` asserts it is NOT. The
/// pair is what makes the second test meaningful: a filter on a code the
/// checker never emits would pass no matter what the engine did.
const UNRESOLVED_IMPORT: &str = "imports_unresolved";

/// Options targeting a concrete Python version, so version-guarded stubs
/// resolve rather than being skipped for want of evidence.
fn options() -> CheckOptions {
    CheckOptions {
        path: Some("main.py".to_owned()),
        python_version: Some("3.13".to_owned()),
    }
}

/// The `(code, line, col, end_line, end_col)` of each diagnostic, which is the
/// part a rendering client positions against.
fn positions(report: &Report) -> Vec<(Option<&str>, usize, usize, usize, usize)> {
    report
        .diagnostics
        .iter()
        .map(|d| (d.code.as_deref(), d.line, d.col, d.end_line, d.end_col))
        .collect()
}

/// Verifies [WASM-NOFS]: the standard library resolves from the snapshot
/// embedded in the binary, with every import search root empty.
///
/// This is the property that lets the playground be a static file. If it
/// regressed, the browser would need a server to serve typeshed — which is
/// exactly the outcome the design exists to avoid.
#[test]
fn stdlib_import_resolves_from_the_embedded_bundle() {
    let report = check_source(
        "import typing\nfrom collections.abc import Sequence\n\ndef first(items: Sequence[int]) -> typing.Optional[int]:\n    return items[0] if items else None\n",
        &options(),
    );

    let unresolved: Vec<&str> = report
        .diagnostics
        .iter()
        .filter(|d| d.code.as_deref() == Some(UNRESOLVED_IMPORT))
        .map(|d| d.message.as_str())
        .collect();

    assert!(
        unresolved.is_empty(),
        "`typing` and `collections.abc` must resolve from the embedded typeshed \
         with no filesystem, got: {unresolved:?}"
    );
}

/// Verifies [WASM-LIMITS]: with no `site_packages`, a third-party import
/// reports unresolved.
///
/// Silently resolving it would make the playground answer a question about an
/// environment that does not exist. This also pins [`UNRESOLVED_IMPORT`] to a
/// code the checker really emits.
#[test]
fn third_party_import_is_unresolved() {
    let report = check_source("import numpy\n", &options());

    assert_eq!(
        positions(&report),
        vec![(Some(UNRESOLVED_IMPORT), 1, 1, 1, 13)],
        "an uninstalled third-party import must be reported exactly once, \
         spanning the import statement"
    );
    assert!(
        report.diagnostics[0].message.contains("numpy"),
        "the message must name the module that could not be resolved, got: {}",
        report.diagnostics[0].message
    );
}

/// Verifies [WASM-API]: the default configuration runs the PEP typing-spec
/// rules and no house-style rule, so ordinary annotated code is silent.
#[test]
fn clean_source_reports_no_diagnostics() {
    let report = check_source(
        "def add(a: int, b: int) -> int:\n    return a + b\n",
        &options(),
    );

    assert_eq!(
        report.diagnostics,
        vec![],
        "well-typed source must produce no diagnostics under the default config"
    );
}

/// Verifies [WASM-DIAGNOSTIC]: the engine reproduces the CLI's answer exactly.
///
/// The expected values below are the verbatim output of `basilisk check
/// --output json` on this source — both diagnostics, both spans. Positions come
/// from the same `basilisk_common::text::LineIndex` the CLI renders with, so a
/// span cannot land one column apart between the browser and the terminal.
#[test]
fn type_error_matches_the_cli_byte_for_byte() {
    let report = check_source("def f() -> int:\n    return \"not an int\"\n", &options());

    assert_eq!(
        positions(&report),
        vec![
            // Anchored on the declared return type in the signature.
            (Some("returns_compatibility"), 1, 5, 1, 6),
            // Anchored on the offending `return` statement.
            (Some("returns_compatibility_2"), 2, 5, 2, 24),
        ],
        "the browser must report the same rules at the same spans as the CLI"
    );

    for diagnostic in &report.diagnostics {
        assert_eq!(
            diagnostic.severity, "error",
            "a return mismatch is an error"
        );
        assert_eq!(
            diagnostic.path, "main.py",
            "diagnostics carry the requested path"
        );
        assert!(
            diagnostic.message.contains("not assignable to int"),
            "the message must explain the mismatch, got: {}",
            diagnostic.message
        );
    }
}

/// Verifies [WASM-API]: `python_version` is load-bearing, not decorative.
///
/// A PEP 695 `type` alias is valid on 3.12+ and an error before it, so the same
/// source must produce different answers for different targets. Without this,
/// the option could silently do nothing and every other test would still pass.
#[test]
fn python_version_gates_version_specific_syntax() {
    let source = "type Alias = int\n";

    let old = check_source(
        source,
        &CheckOptions {
            path: Some("main.py".to_owned()),
            python_version: Some("3.10".to_owned()),
        },
    );
    assert_eq!(
        positions(&old),
        vec![(Some("version_target_syntax"), 1, 6, 1, 11)],
        "a PEP 695 alias must be rejected when targeting 3.10"
    );

    let new = check_source(source, &options());
    assert_eq!(
        new.diagnostics,
        vec![],
        "the same alias must be accepted when targeting 3.13"
    );
}

/// Verifies [WASM-API]: an absent `python_version` means no version evidence at
/// all, deliberately rather than a default release
/// ([CHKARCH-VERSION-TARGET]). Version-gated rules therefore do not fire.
#[test]
fn absent_python_version_means_no_version_evidence() {
    let report = check_source(
        "type Alias = int\n",
        &CheckOptions {
            path: None,
            python_version: None,
        },
    );

    assert_eq!(
        report.diagnostics,
        vec![],
        "with no target evidence a version-gated rule must not guess a release"
    );
}

/// Verifies [WASM-PIPELINE]: a syntax error arrives as a diagnostic on the
/// normal channel, not as a panic or a thrown exception. The caller is an
/// editor that wants to render the problem.
#[test]
fn parse_failure_becomes_a_diagnostic() {
    let report = check_source("def broken(\n", &options());

    let first = report
        .diagnostics
        .first()
        .expect("unparseable source must still produce a report");

    assert_eq!(first.code, None, "no rule ran, so none can be named");
    assert_eq!(first.severity, "error", "a parse failure is an error");
    assert_eq!(
        (first.line, first.col),
        (1, 1),
        "a whole-file failure is anchored at the start of the file"
    );
}

/// Verifies [WASM-API]: malformed options are reported, never fatal. The JS
/// boundary has no exception contract worth relying on.
#[test]
fn malformed_options_json_is_an_error_not_a_panic() {
    let json = check_json("x = 1\n", "{ not json");
    let report: Report = serde_json::from_str(&json).expect("the result is always valid JSON");

    let first = report
        .diagnostics
        .first()
        .expect("bad options must be reported, not silently defaulted");

    assert_eq!(first.code, None, "no rule ran");
    assert!(
        first.message.contains("options"),
        "the message must say what it disliked, got: {}",
        first.message
    );
}

/// Verifies [WASM-API]: an unknown option is refused rather than ignored. A
/// playground that discarded `python_verison` would answer a question the
/// reader did not ask.
#[test]
fn unknown_option_is_refused_rather_than_ignored() {
    let json = check_json("x = 1\n", r#"{"python_verison": "3.13"}"#);
    let report: Report = serde_json::from_str(&json).expect("the result is always valid JSON");

    assert_eq!(
        report.diagnostics.len(),
        1,
        "a misspelled option must be reported, got: {:?}",
        report.diagnostics
    );
    assert_eq!(report.diagnostics[0].code, None, "no rule ran");
}

/// Verifies [WASM-API]: `{}` is a valid request, and an absent path falls back
/// to the documented label.
#[test]
fn empty_options_are_valid_and_use_the_default_path() {
    let json = check_json("def f() -> int:\n    return \"s\"\n", "{}");
    let report: Report = serde_json::from_str(&json).expect("the result is always valid JSON");

    let first = report
        .diagnostics
        .first()
        .expect("the type error must still be found with default options");

    assert_eq!(
        first.path, "<playground>.py",
        "an absent path uses the documented default label"
    );
}

/// Verifies [WASM-DIAGNOSTIC]: the entry shape is field-for-field the CLI's
/// `--output json` contract.
///
/// Redeclaring the DTO was forced by the CLI's being native-only; this test is
/// what stops the two drifting apart silently.
#[test]
fn diagnostic_fields_match_the_cli_json_contract() {
    // `basilisk-cli/src/output/json.rs::JsonDiagnostic`, in declaration order.
    const CLI_JSON_FIELDS: [&str; 8] = [
        "code", "severity", "message", "path", "line", "col", "end_line", "end_col",
    ];

    let json = check_json("def f() -> int:\n    return \"s\"\n", "{}");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    let first = parsed["diagnostics"]
        .get(0)
        .and_then(serde_json::Value::as_object)
        .expect("the type error must produce an entry to inspect");

    let mut actual: Vec<&str> = first.keys().map(String::as_str).collect();
    actual.sort_unstable();
    let mut expected = CLI_JSON_FIELDS.to_vec();
    expected.sort_unstable();

    assert_eq!(
        actual, expected,
        "the browser JSON must carry exactly the CLI's fields — \
         update both DTOs together or consumers break"
    );
}

/// Verifies [WASM-DIAGNOSTIC]: the report round-trips through JSON unchanged,
/// so what a client parses is what the engine produced.
#[test]
fn report_round_trips_through_json() {
    let source = "def f() -> int:\n    return \"s\"\n";
    let direct = check_source(source, &options());

    let parsed: Report = serde_json::from_str(&check_json(
        source,
        r#"{"path":"main.py","python_version":"3.13"}"#,
    ))
    .expect("the result is always valid JSON");

    assert_eq!(
        parsed, direct,
        "the JSON entry point must carry the engine's report without loss"
    );
    assert!(
        !direct.diagnostics.is_empty(),
        "the fixture must produce diagnostics, or this proves nothing"
    );
}

/// Verifies [WASM-DIAGNOSTIC]: a failure report is well-formed and positioned
/// at the start of the file.
#[test]
fn failure_report_is_anchored_at_the_start_of_the_file() {
    let report = Report::from_failure("main.py", "boom");

    assert_eq!(
        report.diagnostics,
        vec![WasmDiagnostic {
            code: None,
            severity: "error".to_owned(),
            message: "boom".to_owned(),
            path: "main.py".to_owned(),
            line: 1,
            col: 1,
            end_line: 1,
            end_col: 1,
        }],
        "a whole-file failure names no rule and points at the file's start"
    );
}
