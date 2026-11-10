//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
pub use basilisk_checker::{check, Diagnostic};
pub use basilisk_parser::parse_source;
pub use basilisk_resolver::resolve;

pub fn run(source: &str) -> Result<Vec<Diagnostic>, Box<dyn std::error::Error>> {
    let resolved = resolve_test_module(source)?;
    Ok(check(&resolved))
}

/// Resolve a test module with the release-bundled immutable Typeshed generation.
fn resolve_test_module(
    source: &str,
) -> Result<basilisk_resolver::ResolvedModule, Box<dyn std::error::Error>> {
    use std::sync::Arc;

    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let mut resolved = resolve(&parsed)?;
    let snapshot = basilisk_stubs::typeshed::bundle::bundled_snapshot()?;
    let paths = basilisk_checker::imports::ImportSearchPaths {
        roots: Vec::new(),
        extra_paths: Vec::new(),
        stub_paths: Vec::new(),
        workspace_members: Vec::new(),
        site_packages: None,
        registry: None,
        typeshed_snapshot: Some(basilisk_checker::imports::ActiveTypeshed::new(
            Arc::new(snapshot),
            None,
        )),
    };
    basilisk_checker::imports::resolve_module_imports(&mut resolved, &paths);
    Ok(resolved)
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
#[allow(
    dead_code,
    reason = "shared harness: each test binary uses the subset of helpers it needs"
)]
pub fn run_with_config(
    source: &str,
    config: &basilisk_config::BasiliskConfig,
) -> Result<Vec<Diagnostic>, Box<dyn std::error::Error>> {
    let resolved = resolve_test_module(source)?;
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
#[allow(
    dead_code,
    reason = "shared harness: each test binary uses the subset of helpers it needs"
)]
pub fn annotation_rules_config() -> basilisk_config::BasiliskConfig {
    use basilisk_config::RuleSeverity::{Error, Warning};

    basilisk_config::BasiliskConfig::with_rule_entries(
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

#[allow(
    dead_code,
    reason = "shared harness: each test binary uses the subset of helpers it needs"
)]
pub fn codes(diags: &[Diagnostic]) -> Vec<&str> {
    diags.iter().map(|d| d.code.code).collect()
}

#[allow(
    dead_code,
    reason = "shared harness: each test binary uses the subset of helpers it needs"
)]
pub fn codes_owned(diags: &[Diagnostic]) -> Vec<String> {
    diags.iter().map(|d| d.code.code.to_string()).collect()
}

#[allow(
    dead_code,
    reason = "shared harness: each test binary uses the subset of helpers it needs"
)]
pub fn has_code(diags: &[Diagnostic], code: &str) -> bool {
    diags.iter().any(|d| d.code.code == code)
}

#[allow(
    dead_code,
    reason = "shared harness: each test binary uses the subset of helpers it needs"
)]
pub fn messages_for<'a>(diags: &'a [Diagnostic], code: &str) -> Vec<&'a str> {
    diags
        .iter()
        .filter(|d| d.code.code == code)
        .map(|d| d.message.as_str())
        .collect()
}

/// Assert one isolated typing-spec obligation against one diagnostic family.
///
/// Unlike `!diagnostics.is_empty()`, this cannot pass because an unrelated rule
/// happened to reject the fixture.
pub fn assert_rule_count(diags: &[Diagnostic], code: &str, expected: usize, obligation: &str) {
    let matching: Vec<&Diagnostic> = diags
        .iter()
        .filter(|diagnostic| diagnostic.code.code == code)
        .collect();
    assert_eq!(
        matching.len(),
        expected,
        "{obligation}: expected {expected} `{code}` diagnostic(s), got {}: {matching:#?}",
        matching.len(),
    );
}
