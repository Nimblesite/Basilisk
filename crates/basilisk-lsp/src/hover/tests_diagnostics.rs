//! Hover tests for the diagnostic sections ([LSPARCH-FEATURES-HOVER]),
//! including the Configure Severity link for non-PEP rules
//! ([CONFIGEDITOR-VSIX-EXPERIENCE]).

use super::tests::parse_and_resolve;
use super::*;
use basilisk_checker::{ErrorCode, Severity};
use basilisk_resolver::Span;

/// Build a checker diagnostic with `code` covering the first occurrence of
/// `needle` in `source`.
fn diagnostic_over(source: &str, needle: &str, code: ErrorCode) -> basilisk_checker::Diagnostic {
    let start = source.find(needle).expect("needle present in source");
    let end = start + needle.len();
    basilisk_checker::Diagnostic {
        code,
        severity: Severity::Error,
        message: format!("Missing parameter type annotation for `{needle}`"),
        span: Span::new(
            u32::try_from(start).expect("test offsets fit in u32"),
            u32::try_from(end).expect("test offsets fit in u32"),
        ),
        path: "test.py".to_owned(),
        help: Some(std::borrow::Cow::Borrowed(
            "Add a type annotation: `raw: <type>`",
        )),
        note: None,
        provenance: None,
    }
}

/// Hover markdown for `source` with `diag` at the diagnostic's start offset.
fn hover_markdown(source: &str, diag: basilisk_checker::Diagnostic) -> String {
    let resolved = parse_and_resolve(source);
    let offset = diag.span.start_usize();
    let hover = hover_at(&resolved, source, offset, &[diag])
        .expect("hover should be Some over a diagnostic");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected Markup hover contents");
    };
    markup.value
}

/// A non-PEP diagnostic (opt-in Basilisk house rule, e.g. `BSK-0001`) must
/// offer a "Configure Severity" command link that opens the configuration
/// editor focused on the rule, so users can grade or disable the rule from
/// the hover ([CONFIGEDITOR-VSIX-EXPERIENCE]).
#[test]
fn test_hover_on_non_pep_diagnostic_offers_configure_severity_link() {
    let source = "def normalize_reading(raw) -> None:\n    return None\n";
    let diag = diagnostic_over(
        source,
        "raw",
        ErrorCode {
            code: "BSK-0001",
            docs_url: "https://www.basilisk-python.dev/errors/BSK-0001",
        },
    );

    let markdown = hover_markdown(source, diag);
    assert!(
        markdown.contains(
            "[Configure Severity](command:basilisk.openConfigurationEditor?\
             %5B%7B%22rule%22%3A%22BSK-0001%22%7D%5D)"
        ),
        "non-PEP diagnostic hover must link to the configuration editor \
         focused on the rule: {markdown}"
    );
}

/// A PEP rule is graded by the typing spec and can never be disabled
/// ([CHKARCH-CONFIG-MODEL]) — its hover must NOT offer the link.
#[test]
fn test_hover_on_pep_diagnostic_has_no_configure_severity_link() {
    let source = "def normalize_reading(raw) -> None:\n    return None\n";
    let diag = diagnostic_over(
        source,
        "raw",
        ErrorCode {
            code: "returns_compatibility",
            docs_url: "https://www.basilisk-python.dev/errors/returns_compatibility",
        },
    );

    let markdown = hover_markdown(source, diag);
    assert!(
        !markdown.contains("Configure Severity"),
        "PEP diagnostic hover must not offer a Configure Severity link: {markdown}"
    );
}
