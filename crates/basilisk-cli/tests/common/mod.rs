//! Tests for [CHKARCH-CLI]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions,
    // Shared helpers (`run` / `run_with_config` / `annotation_rules_config` /
    // `fixture`) are each used by SOME but not every CLI test binary; `mod
    // common` compiles into each binary independently, so a helper unused by one
    // is not dead across the suite.
    dead_code
)]
//! Shared helpers for Basilisk CLI end-to-end tests.
//!
//! Every test uses a real `.py` fixture file and asserts the exact set of
//! diagnostics produced: error code, symbol name, byte span, line, column,
//! and message. No hand-wavy count assertions — if a diagnostic appears at
//! the wrong location or with the wrong message, the test fails.
//!
//! Pipeline under test: `parse_file` → resolve → check

use std::path::Path;

use basilisk_checker::{check, check_with_config, Diagnostic};
use basilisk_config::BasiliskConfig;
use basilisk_parser::parse_file;
use basilisk_resolver::resolve;

// Re-export shared helpers from the test-utils crate — used by sibling test modules.
#[expect(
    unused_imports,
    reason = "re-exported for sibling test files via `use common::assert_diagnostics`"
)]
pub use basilisk_test_utils::assert_diagnostics;
#[expect(
    unused_imports,
    reason = "re-exported for sibling test files via `use common::Expected`"
)]
pub use basilisk_test_utils::Expected;

pub fn fixture(rel: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(rel)
        .to_string_lossy()
        .into_owned()
}

pub fn run(rel: &str) -> Result<Vec<Diagnostic>, Box<dyn std::error::Error>> {
    let path = fixture(rel);
    let parsed = parse_file(&path)?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

/// Run the checker over a fixture honoring an explicit project configuration.
///
/// Basilisk has **no modes** — the checker does exactly what configuration
/// says. [`run`] uses the default config (every PEP rule, no `BSK-`prefixed
/// house rules), so out of the box a fixture is graded as pure PEP conformance.
/// House-style rules (require-annotation, require-`@override`, redundant
/// annotation, …) are off by default; a fixture that exercises them passes a
/// config that opts in. See [CHKARCH-CONFIGURATION-ONLY].
pub fn run_with_config(
    rel: &str,
    config: &BasiliskConfig,
) -> Result<Vec<Diagnostic>, Box<dyn std::error::Error>> {
    let path = fixture(rel);
    let parsed = parse_file(&path)?;
    let resolved = resolve(&parsed)?;
    Ok(check_with_config(&resolved, config))
}

/// Project configuration with explicit native severities for Basilisk's
/// annotation house rules.
///
/// This is configuration **data** — exactly what a project writes in
/// config file to enable these off-by-default rules — not a checker "mode".
/// See [CHKARCH-CONFIGURATION-ONLY].
#[must_use]
pub fn annotation_rules_config() -> BasiliskConfig {
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
