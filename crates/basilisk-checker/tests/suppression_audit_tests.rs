//! Integration tests for [CONFIGEDITOR-SUPPRESSIONS] and
//! [CHKARCH-STRICTNESS-SUPPRESSION-DIAGNOSTICS].
#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test-only fixture setup and compact diagnostic assertions"
)]

use std::collections::HashMap;

use basilisk_checker::{Diagnostic, Severity};
use basilisk_config::{BasiliskConfig, RuleSeverity};

fn check(
    source: &str,
    rules: impl IntoIterator<Item = (&'static str, RuleSeverity)>,
) -> Vec<Diagnostic> {
    let parsed = basilisk_parser::parse_source(source.to_owned(), "test.py".to_owned())
        .expect("source should parse");
    let resolved = basilisk_resolver::resolve(&parsed).expect("source should resolve");
    let config = BasiliskConfig {
        rules: rules
            .into_iter()
            .map(|(code, severity)| (code.to_owned(), severity))
            .collect::<HashMap<_, _>>(),
        ..Default::default()
    };
    basilisk_checker::check_with_config(&resolved, &config)
}

fn diagnostics_for<'a>(diagnostics: &'a [Diagnostic], code: &str) -> Vec<&'a Diagnostic> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code.code == code)
        .collect()
}

#[test]
fn suppression_auditing_is_off_by_default() {
    let source = "x: int = \"bad\"  # type: ignore[assignment_compatibility]\n";
    let diagnostics = check(source, []);
    assert!(!diagnostics.iter().any(|diagnostic| matches!(
        diagnostic.code.code,
        "BSK-I0060" | "BSK-W0061" | "BSK-W0062" | "BSK-E0063"
    )));
}

#[test]
fn each_audit_rule_honours_every_configured_severity() {
    let cases = [
        (
            "BSK-I0060",
            "x: int = \"bad\"  # type: ignore[assignment_compatibility]\n",
        ),
        ("BSK-W0061", "x: int = \"bad\"  # type: ignore\n"),
        (
            "BSK-W0062",
            "x: int = 1  # type: ignore[assignment_compatibility]\n",
        ),
        ("BSK-E0063", "x: int = \"bad\"  # type: ignore[BSK-E9999]\n"),
    ];
    let severities = [
        (RuleSeverity::Error, Some(Severity::Error)),
        (RuleSeverity::Warning, Some(Severity::Warning)),
        (RuleSeverity::Info, Some(Severity::Info)),
        (RuleSeverity::Disabled, None),
    ];

    for (code, source) in cases {
        for (configured, expected) in severities {
            let diagnostics = check(source, [(code, configured)]);
            let audit = diagnostics_for(&diagnostics, code);
            match expected {
                Some(expected) => {
                    assert!(
                        !audit.is_empty(),
                        "{code} should be selected by {configured:?}"
                    );
                    assert!(audit
                        .iter()
                        .all(|diagnostic| diagnostic.severity == expected));
                }
                None => assert!(audit.is_empty(), "disabled {code} must remain off"),
            }
        }
    }
}

#[test]
fn a_blanket_directive_cannot_suppress_its_own_audit_diagnostic() {
    let source = "x: int = \"bad\"  # type: ignore\n";
    let diagnostics = check(source, [("BSK-W0061", RuleSeverity::Error)]);
    let audit = diagnostics_for(&diagnostics, "BSK-W0061");
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].severity, Severity::Error);
    assert!(diagnostics_for(&diagnostics, "assignment_compatibility").is_empty());
}

#[test]
fn line_block_and_file_directives_retain_auditable_spans_and_usage() {
    let line = "x: int = \"bad\"  # type: ignore[assignment_compatibility]\n";
    let line_diagnostics = check(line, [("BSK-I0060", RuleSeverity::Info)]);
    let line_audit = diagnostics_for(&line_diagnostics, "BSK-I0060");
    assert_eq!(line_audit.len(), 1);
    assert!(line_audit[0]
        .span
        .slice_source(line)
        .is_some_and(|text| text.starts_with("# type:")));

    let block = "# type: disabled[assignment_compatibility]\nx: int = \"bad\"\n# type: end-disabled[assignment_compatibility]\n";
    let block_diagnostics = check(block, [("BSK-I0060", RuleSeverity::Info)]);
    let block_audit = diagnostics_for(&block_diagnostics, "BSK-I0060");
    assert_eq!(block_audit.len(), 2, "both paired boundaries retain usage");
    assert!(block_audit.iter().all(|diagnostic| diagnostic
        .span
        .slice_source(block)
        .is_some_and(|text| text.starts_with("# type:"))));

    let file = "# basilisk: file-disabled[assignment_compatibility]\nx: int = \"bad\"\n";
    let file_diagnostics = check(file, [("BSK-I0060", RuleSeverity::Info)]);
    let file_audit = diagnostics_for(&file_diagnostics, "BSK-I0060");
    assert_eq!(file_audit.len(), 1);
    assert_eq!(
        file_audit[0].span.slice_source(file),
        Some("# basilisk: file-disabled[assignment_compatibility]")
    );
}

#[test]
fn unmatched_and_unclosed_block_boundaries_are_malformed() {
    let unmatched = "# type: end-disabled[assignment_compatibility]\n";
    assert_eq!(
        diagnostics_for(
            &check(unmatched, [("BSK-E0063", RuleSeverity::Error)]),
            "BSK-E0063"
        )
        .len(),
        1
    );

    let unclosed = "# type: disabled[assignment_compatibility]\nx: int = \"bad\"\n";
    let diagnostics = check(unclosed, [("BSK-E0063", RuleSeverity::Error)]);
    assert_eq!(diagnostics_for(&diagnostics, "BSK-E0063").len(), 1);
    let assignment = diagnostics_for(&diagnostics, "assignment_compatibility");
    assert_eq!(assignment.len(), 1, "an unclosed block must be inert");
    assert_eq!(assignment[0].severity, Severity::Error);
}

#[test]
fn conflicting_line_directives_are_reported_and_all_inert() {
    for (source, participants) in [
        (
            "x: int = \"bad\"  # type: warning[assignment_compatibility]  # type: ignore[assignment_compatibility]\n",
            2,
        ),
        (
            "x: int = \"bad\"  # type: ignore[assignment_compatibility]  # type: warning[assignment_compatibility]\n",
            2,
        ),
        (
            "x: int = \"bad\"  # type: warning[assignment_compatibility]  # type: info[assignment_compatibility]  # type: ignore[assignment_compatibility]\n",
            3,
        ),
    ] {
        let diagnostics = check(source, [("BSK-E0063", RuleSeverity::Error)]);
        assert_eq!(
            diagnostics_for(&diagnostics, "BSK-E0063").len(),
            participants,
            "every conflict participant must be reported"
        );
        let assignment = diagnostics_for(&diagnostics, "assignment_compatibility");
        assert_eq!(assignment.len(), 1, "conflicting directives must be inert");
        assert_eq!(assignment[0].severity, Severity::Error);
    }
}

#[test]
fn valid_line_and_closed_block_directives_still_apply() {
    let line = "x: int = \"bad\"  # type: warning[assignment_compatibility]\n";
    let line_diagnostics = check(line, [("BSK-E0063", RuleSeverity::Error)]);
    assert!(diagnostics_for(&line_diagnostics, "BSK-E0063").is_empty());
    let assignment = diagnostics_for(&line_diagnostics, "assignment_compatibility");
    assert_eq!(assignment.len(), 1);
    assert_eq!(assignment[0].severity, Severity::Warning);

    let block = "# type: disabled[assignment_compatibility]\nx: int = \"bad\"\n# type: end-disabled[assignment_compatibility]\n";
    let block_diagnostics = check(block, [("BSK-E0063", RuleSeverity::Error)]);
    assert!(diagnostics_for(&block_diagnostics, "BSK-E0063").is_empty());
    assert!(diagnostics_for(&block_diagnostics, "assignment_compatibility").is_empty());
}

#[test]
fn malformed_line_directives_are_reported_but_never_applied() {
    for source in [
        "x: int = \"bad\"  # type: ignoree\n",
        "x: int = \"bad\"  # type: warningg\n",
        "x: int = \"bad\"  # type: warnin[assignment_compatibility]\n",
        "x: int = \"bad\"  # type: infoo\n",
        "x: int = \"bad\"  # type: disable[assignment_compatibility]\n",
        // Only a structurally broken *bracket* selector stays malformed: an
        // unclosed `[` or an empty `[]`. Trailing text after a closed `]` is a
        // comment (see the ignore-with-trailing-text test), not junk.
        "x: int = \"bad\"  # type: ignore[assignment_compatibility\n",
        "x: int = \"bad\"  # type: ignore[]\n",
    ] {
        let diagnostics = check(source, [("BSK-E0063", RuleSeverity::Error)]);
        assert_eq!(
            diagnostics_for(&diagnostics, "BSK-E0063").len(),
            1,
            "the malformed suppression must remain auditable"
        );
        let assignment = diagnostics_for(&diagnostics, "assignment_compatibility");
        assert_eq!(
            assignment.len(),
            1,
            "malformed syntax must not suppress an otherwise live diagnostic"
        );
        assert_eq!(
            assignment[0].severity,
            Severity::Error,
            "malformed syntax must not demote an otherwise live diagnostic"
        );
    }
}

/// A `# type: ignore` followed by free-form trailing text (`- reason`,
/// `# comment`, a bare word) is a spec-valid blanket suppression, so it silences
/// the otherwise-live diagnostic on its line while remaining auditable. See
/// `directives_type_ignore.py` in the conformance suite.
#[test]
fn ignore_with_trailing_text_blanket_suppresses_and_is_auditable() {
    for source in [
        "x: int = \"bad\"  # type: ignore - additional stuff\n",
        "x: int = \"bad\"  # type: ignore # other comment\n",
        "x: int = \"bad\"  # type: ignore assignment_compatibility\n",
    ] {
        let diagnostics = check(source, [("BSK-E0063", RuleSeverity::Error)]);
        assert!(
            diagnostics_for(&diagnostics, "assignment_compatibility").is_empty(),
            "blanket ignore with trailing text must suppress the diagnostic: {source}"
        );
    }
}

#[test]
fn pep_484_type_comments_are_not_suppression_directives() {
    let source = r"
values = []  # type: list[int]

def render(value):
    # type: (int) -> str
    return str(value)
";
    let diagnostics = check(
        source,
        [
            ("BSK-I0060", RuleSeverity::Info),
            ("BSK-W0061", RuleSeverity::Warning),
            ("BSK-W0062", RuleSeverity::Warning),
            ("BSK-E0063", RuleSeverity::Error),
        ],
    );
    assert!(
        diagnostics.iter().all(|diagnostic| !matches!(
            diagnostic.code.code,
            "BSK-I0060" | "BSK-W0061" | "BSK-W0062" | "BSK-E0063"
        )),
        "standard variable and function type comments must not enter the suppression ledger"
    );
}

#[test]
fn file_directives_after_code_are_malformed_and_never_applied() {
    for source in [
        "marker = 1\n# basilisk: relaxed\nx: int = \"bad\"\n",
        "marker = 1\n# basilisk: file-disabled[assignment_compatibility]\nx: int = \"bad\"\n",
        "marker = 1\n# type: ignore\nx: int = \"bad\"\n",
    ] {
        let diagnostics = check(source, [("BSK-E0063", RuleSeverity::Error)]);
        assert_eq!(
            diagnostics_for(&diagnostics, "BSK-E0063").len(),
            1,
            "a file directive outside the header must be reported"
        );
        let assignment = diagnostics_for(&diagnostics, "assignment_compatibility");
        assert_eq!(assignment.len(), 1, "a misplaced directive must not apply");
        assert_eq!(assignment[0].severity, Severity::Error);
    }
}
