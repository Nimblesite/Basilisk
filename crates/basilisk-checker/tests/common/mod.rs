//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
pub use basilisk_checker::{check, Diagnostic};
pub use basilisk_parser::parse_source;
pub use basilisk_resolver::resolve;

pub fn run(source: &str) -> Result<Vec<Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

/// Run with every Basilisk-only opt-in rule enabled.
///
/// The `BSK-`prefixed rules (strict annotations, uv dependency hygiene, stub
/// suggestions) are off by default so the out-of-the-box experience is pure PEP
/// conformance. Tests that assert those rules fire must opt in via this helper.
pub fn run_strict(source: &str) -> Result<Vec<Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let config = basilisk_config::BasiliskConfig {
        strict_annotations: true,
        uv_stub_suggestions: true,
        uv_dependency_diagnostics: true,
        ..basilisk_config::BasiliskConfig::default()
    };
    Ok(basilisk_checker::check_with_config(&resolved, &config))
}

pub fn codes(diags: &[Diagnostic]) -> Vec<&str> {
    diags.iter().map(|d| d.code.code).collect()
}

pub fn codes_owned(diags: &[Diagnostic]) -> Vec<String> {
    diags.iter().map(|d| d.code.code.to_string()).collect()
}

pub fn has_code(diags: &[Diagnostic], code: &str) -> bool {
    diags.iter().any(|d| d.code.code == code)
}

pub fn messages_for<'a>(diags: &'a [Diagnostic], code: &str) -> Vec<&'a str> {
    diags
        .iter()
        .filter(|d| d.code.code == code)
        .map(|d| d.message.as_str())
        .collect()
}
