//! Tests for [CHKARCH-CONFIG-MODEL] / [CHKARCH-COMMANDS]. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions,
    unused_results,
    dead_code
)]
//! E2E tests for the configuration model through the full pipeline.
//!
//! Pipeline: `parse_file` → `resolve` → `check_with_config`
//!
//! The model is two flat maps ([CHKARCH-CONFIG-MODEL]): `[tool.basilisk.rules]`
//! (code → severity) and `[tool.basilisk.rule-tags]` (tag → severity),
//! resolved nearest-deciding-table-first; a rule entry beats tag entries and
//! the strictest matching tag wins. There are no per-path globs, per-module
//! overrides, or presets.

mod common;

use std::collections::HashMap;

use basilisk_checker::{check_with_config, Diagnostic, Severity};
use basilisk_config::{BasiliskConfig, RuleSeverity, RuleTables};
use basilisk_parser::parse_file;
use basilisk_resolver::resolve;
use common::fixture;

/// Parse + resolve + check with a given config.
fn run_with_config(
    rel: &str,
    config: &BasiliskConfig,
) -> Result<Vec<Diagnostic>, Box<dyn std::error::Error>> {
    let path = fixture(rel);
    let parsed = parse_file(&path)?;
    let resolved = resolve(&parsed)?;
    Ok(check_with_config(&resolved, config))
}

/// A config whose single nearest table holds these per-rule entries.
fn rules_config(entries: &[(&str, RuleSeverity)]) -> BasiliskConfig {
    BasiliskConfig::with_rule_entries(
        entries
            .iter()
            .map(|(code, severity)| ((*code).to_owned(), *severity))
            .collect(),
    )
}

/// A config whose single nearest table holds these tag entries.
fn tags_config(entries: &[(&str, RuleSeverity)]) -> BasiliskConfig {
    BasiliskConfig {
        rule_chain: vec![RuleTables {
            rules: HashMap::new(),
            rule_tags: entries
                .iter()
                .map(|(tag, severity)| ((*tag).to_owned(), *severity))
                .collect(),
        }],
        ..BasiliskConfig::default()
    }
}

/// Config with explicit severities for the opt-in rules used here.
fn annotations_on() -> BasiliskConfig {
    rules_config(&[
        ("BSK-0001", RuleSeverity::Error),
        ("BSK-0002", RuleSeverity::Error),
    ])
}

// ---------------------------------------------------------------------------
// Rule-entry selection and grading ([CHKARCH-CONFIG-MODEL])
// ---------------------------------------------------------------------------

/// A `disabled` entry deselects an analyze-scope rule entirely.
#[test]
fn rule_entry_disabled_suppresses_bsk_0001() -> Result<(), Box<dyn std::error::Error>> {
    let config = rules_config(&[
        ("BSK-0001", RuleSeverity::Disabled),
        ("BSK-0002", RuleSeverity::Error),
    ]);

    let diags = run_with_config("missing_param_annotation.py", &config)?;
    let has_bsk_0001 = diags.iter().any(|d| d.code.code == "BSK-0001");
    assert!(
        !has_bsk_0001,
        "BSK-0001 must be deselected by a disabled rule entry, got: {diags:#?}"
    );
    Ok(())
}

/// A `warning` entry selects and grades the rule.
#[test]
fn rule_entry_warning_demotes_bsk_0001() -> Result<(), Box<dyn std::error::Error>> {
    let config = rules_config(&[("BSK-0001", RuleSeverity::Warning)]);

    let diags = run_with_config("missing_param_annotation.py", &config)?;
    let bsk_0001: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0001").collect();
    assert!(!bsk_0001.is_empty(), "should still emit BSK-0001, graded");
    for diag in &bsk_0001 {
        assert_eq!(
            diag.severity,
            Severity::Warning,
            "BSK-0001 should be graded to warning, got: {diag:?}"
        );
    }
    Ok(())
}

/// An `info` entry selects and grades the rule.
#[test]
fn rule_entry_info_demotes_bsk_0001() -> Result<(), Box<dyn std::error::Error>> {
    let config = rules_config(&[("BSK-0001", RuleSeverity::Info)]);

    let diags = run_with_config("missing_param_annotation.py", &config)?;
    let bsk_0001: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0001").collect();
    assert!(
        !bsk_0001.is_empty(),
        "should still emit BSK-0001, graded to info"
    );
    for diag in &bsk_0001 {
        assert_eq!(
            diag.severity,
            Severity::Info,
            "BSK-0001 should be graded to info, got: {diag:?}"
        );
    }
    Ok(())
}

/// The default config selects nothing beyond the pep scope: `check()` and
/// `check_with_config(default)` are identical. [CHKARCH-CONFIGURATION-ONLY]
#[test]
fn default_config_does_not_change_diagnostics() -> Result<(), Box<dyn std::error::Error>> {
    let config = BasiliskConfig::default();
    let diags_config = run_with_config("missing_param_annotation.py", &config)?;

    // Compare with plain check() (no config)
    let path = fixture("missing_param_annotation.py");
    let parsed = parse_file(&path)?;
    let resolved = resolve(&parsed)?;
    let diags_plain = basilisk_checker::check(&resolved);

    assert_eq!(
        diags_config.len(),
        diags_plain.len(),
        "default config should produce identical diagnostics"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Tag entries ([CHKARCH-CONFIG-MODEL])
// ---------------------------------------------------------------------------

/// One `"basilisk" = "error"` tag entry turns every house rule on: the
/// annotation rules fire without any per-rule entry.
#[test]
fn basilisk_tag_entry_selects_house_rules() -> Result<(), Box<dyn std::error::Error>> {
    let config = tags_config(&[("basilisk", RuleSeverity::Error)]);

    let diags = run_with_config("missing_both.py", &config)?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"BSK-0001") && codes.contains(&"BSK-0002"),
        "the `basilisk` tag entry must select the annotation rules, got: {codes:?}"
    );
    assert!(
        diags
            .iter()
            .filter(|d| d.code.code.starts_with("BSK-000"))
            .all(|d| d.severity == Severity::Error),
        "the tag entry's severity grades the selected rules"
    );
    Ok(())
}

/// Within one table a per-rule entry beats tag entries: the tag turns the
/// house rules on at error, the rule entry re-grades one of them to info.
#[test]
fn rule_entry_beats_tag_entry() -> Result<(), Box<dyn std::error::Error>> {
    let config = BasiliskConfig {
        rule_chain: vec![RuleTables {
            rules: [("BSK-0001".to_owned(), RuleSeverity::Info)]
                .into_iter()
                .collect(),
            rule_tags: [("basilisk".to_owned(), RuleSeverity::Error)]
                .into_iter()
                .collect(),
        }],
        ..BasiliskConfig::default()
    };

    let diags = run_with_config("missing_both.py", &config)?;
    let bsk_0001: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-0001").collect();
    assert!(!bsk_0001.is_empty(), "BSK-0001 must still be selected");
    for diag in &bsk_0001 {
        assert_eq!(
            diag.severity,
            Severity::Info,
            "the per-rule entry must beat the tag entry within one table"
        );
    }
    assert!(
        diags
            .iter()
            .filter(|d| d.code.code == "BSK-0002")
            .all(|d| d.severity == Severity::Error),
        "rules without a per-rule entry keep the tag entry's grade"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Nearest-table resolution ([CHKARCH-CONFIG-MODEL])
// ---------------------------------------------------------------------------

/// The nearest table that decides a rule wins outright: a nearer `warning`
/// entry beats an ancestor `error` entry — per rule, not per table.
#[test]
fn nearest_deciding_table_wins() -> Result<(), Box<dyn std::error::Error>> {
    // rule_chain is nearest-first ([CHKARCH-CONFIG-DISCOVERY]).
    let nearer = RuleTables {
        rules: [("BSK-0001".to_owned(), RuleSeverity::Warning)]
            .into_iter()
            .collect(),
        rule_tags: HashMap::new(),
    };
    let ancestor = RuleTables {
        rules: [
            ("BSK-0001".to_owned(), RuleSeverity::Error),
            ("BSK-0002".to_owned(), RuleSeverity::Error),
        ]
        .into_iter()
        .collect(),
        rule_tags: HashMap::new(),
    };
    let config = BasiliskConfig {
        rule_chain: vec![nearer, ancestor],
        ..BasiliskConfig::default()
    };

    let diags = run_with_config("missing_both.py", &config)?;
    assert!(
        diags
            .iter()
            .filter(|d| d.code.code == "BSK-0001")
            .all(|d| d.severity == Severity::Warning),
        "the nearest table's BSK-0001 grade must win"
    );
    assert!(
        diags
            .iter()
            .any(|d| d.code.code == "BSK-0002" && d.severity == Severity::Error),
        "rules the nearest table does not decide fall through to the ancestor"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// PEP rules can be graded, never disabled ([CHKARCH-CONFIG-MODEL])
// ---------------------------------------------------------------------------

/// Grading a pep rule works like any entry.
#[test]
fn pep_rule_grades_to_warning() -> Result<(), Box<dyn std::error::Error>> {
    let config = rules_config(&[("returns_compatibility_2", RuleSeverity::Warning)]);
    let diags = run_with_config("errors/e0013_return_mismatch.py", &config)?;
    let graded: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "returns_compatibility_2")
        .collect();
    assert!(!graded.is_empty(), "the pep rule must still fire");
    assert!(
        graded.iter().all(|d| d.severity == Severity::Warning),
        "a pep rule can be graded to warning, got: {graded:#?}"
    );
    Ok(())
}

/// A config resolving a pep rule to `disabled` is invalid —
/// `pep_disable_violations` reports it, and the checker defensively keeps the
/// rule running so `check` never loses a PEP diagnostic.
#[test]
fn pep_rule_disable_is_invalid_and_defensively_ignored() -> Result<(), Box<dyn std::error::Error>> {
    let config = rules_config(&[("returns_compatibility_2", RuleSeverity::Disabled)]);

    let violations = basilisk_checker::pep_disable_violations(&config);
    assert_eq!(
        violations,
        vec!["returns_compatibility_2"],
        "the invalid pep-disable must be reported"
    );

    let diags = run_with_config("errors/e0013_return_mismatch.py", &config)?;
    assert!(
        diags
            .iter()
            .any(|d| d.code.code == "returns_compatibility_2"),
        "the checker must defensively keep the pep rule running, got: {diags:#?}"
    );
    Ok(())
}
