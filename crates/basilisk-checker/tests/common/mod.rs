//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
pub use basilisk_checker::{check, Diagnostic};
pub use basilisk_parser::parse_source;
pub use basilisk_resolver::resolve;

pub fn run(source: &str) -> Result<Vec<Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

/// Run the checker honoring an explicit project configuration.
///
/// Basilisk has **no modes** — there is no `basic`/`standard`/`strict`, no
/// "opt-in rules" bundle. The checker does exactly what the configuration says
/// and nothing more. [`run`] uses the default configuration (every PEP rule, no
/// `BSK-`prefixed house rules) so the out-of-the-box experience is pure PEP
/// conformance; pass a config here to exercise rules a project has opted into.
/// This mirrors production, whose only entry point is
/// [`basilisk_checker::check_with_config`] — config in, diagnostics out. See
/// [CHKARCH-CONFIGURATION-ONLY].
pub fn run_with_config(
    source: &str,
    config: &basilisk_config::BasiliskConfig,
) -> Result<Vec<Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(basilisk_checker::check_with_config(&resolved, config))
}

/// A project configuration with explicit native severities for Basilisk's
/// annotation house rules.
///
/// This is configuration **data** — the exact thing a project writes in
/// config file to enable these off-by-default rules — not a checker "mode".
/// Suites covering those opt-in rules pass it to [`run_with_config`] so the
/// rules under test fire exactly as they would for a user who set that key, and
/// nothing else turns on. See [CHKARCH-CONFIGURATION-ONLY].
#[must_use]
pub fn annotation_rules_config() -> basilisk_config::BasiliskConfig {
    use basilisk_config::RuleSeverity::{Error, Warning};

    basilisk_config::BasiliskConfig {
        rules: [
            ("BSK-E0001", Error),
            ("BSK-E0002", Error),
            ("BSK-E0003", Error),
            ("BSK-E0004", Error),
            ("BSK-E0005", Error),
            ("BSK-E0025", Error),
            ("BSK-W0014", Warning),
            ("BSK-W0040", Warning),
            ("BSK-W0050", Warning),
        ]
        .into_iter()
        .map(|(code, severity)| (code.to_owned(), severity))
        .collect(),
        ..basilisk_config::BasiliskConfig::default()
    }
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
