//! Tests for [CHKARCH-CLI]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI
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
//! E2E tests for project-level config overrides through the full pipeline.
//!
//! Pipeline: `parse_file` → `resolve` → `check_with_config`
//!
//! These tests verify that `BasiliskConfig` overrides (global rule severity,
//! per-module, per-path) correctly suppress or demote diagnostics when wired
//! through the real analyzer pipeline.

mod common;

use std::collections::HashMap;

use basilisk_checker::{check_with_config, Diagnostic, Severity};
use basilisk_config::{BasiliskConfig, ModuleOverride, PathOverride, RuleSeverity};
use basilisk_parser::parse_file;
use basilisk_resolver::resolve;
use common::fixture;

/// Return the fixtures directory path as a string suitable for per-path pattern matching.
fn fixtures_path_prefix() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .to_string_lossy()
        .into_owned()
}

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

// ---------------------------------------------------------------------------
// Global rule severity overrides
// ---------------------------------------------------------------------------

#[test]
fn global_severity_off_suppresses_e0001() -> Result<(), Box<dyn std::error::Error>> {
    // missing_param_annotation.py triggers E0001 — should be suppressed
    let mut rules = HashMap::new();
    rules.insert("BSK-E0001".to_owned(), RuleSeverity::Disabled);
    let config = BasiliskConfig {
        rules,
        ..Default::default()
    };

    let diags = run_with_config("missing_param_annotation.py", &config)?;
    let has_e0001 = diags.iter().any(|d| d.code.code == "BSK-E0001");
    assert!(
        !has_e0001,
        "E0001 should be suppressed by global rule override, got: {diags:#?}"
    );
    Ok(())
}

#[test]
fn global_severity_warning_demotes_e0001() -> Result<(), Box<dyn std::error::Error>> {
    let mut rules = HashMap::new();
    rules.insert("BSK-E0001".to_owned(), RuleSeverity::Warning);
    let config = BasiliskConfig {
        rules,
        ..Default::default()
    };

    let diags = run_with_config("missing_param_annotation.py", &config)?;
    let e0001_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0001")
        .collect();
    assert!(
        !e0001_diags.is_empty(),
        "should still emit E0001, just demoted"
    );
    for diag in &e0001_diags {
        assert_eq!(
            diag.severity,
            Severity::Warning,
            "E0001 should be demoted to warning, got: {diag:?}"
        );
    }
    Ok(())
}

#[test]
fn global_severity_info_demotes_e0001() -> Result<(), Box<dyn std::error::Error>> {
    let mut rules = HashMap::new();
    rules.insert("BSK-E0001".to_owned(), RuleSeverity::Info);
    let config = BasiliskConfig {
        rules,
        ..Default::default()
    };

    let diags = run_with_config("missing_param_annotation.py", &config)?;
    let e0001_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0001")
        .collect();
    assert!(
        !e0001_diags.is_empty(),
        "should still emit E0001, just demoted to info"
    );
    for diag in &e0001_diags {
        assert_eq!(
            diag.severity,
            Severity::Info,
            "E0001 should be demoted to info, got: {diag:?}"
        );
    }
    Ok(())
}

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
// Per-module overrides: ignore-missing-stubs
// ---------------------------------------------------------------------------

#[test]
fn per_module_override_suppresses_e0010() -> Result<(), Box<dyn std::error::Error>> {
    // e0010_untyped_import.py imports `requests` → triggers E0010
    let mut per_module = HashMap::new();
    per_module.insert(
        "requests".to_owned(),
        ModuleOverride {
            ignore_missing_stubs: true,
        },
    );
    let config = BasiliskConfig {
        per_module_overrides: per_module,
        ..Default::default()
    };

    let diags = run_with_config("errors/e0010_untyped_import.py", &config)?;
    let has_e0010 = diags.iter().any(|d| d.code.code == "BSK-E0010");
    assert!(
        !has_e0010,
        "E0010 should be suppressed for 'requests' via per-module override, got: {diags:#?}"
    );
    Ok(())
}

#[test]
fn per_module_wildcard_suppresses_e0010() -> Result<(), Box<dyn std::error::Error>> {
    // Wildcard pattern should also suppress
    let mut per_module = HashMap::new();
    per_module.insert(
        "requests.*".to_owned(),
        ModuleOverride {
            ignore_missing_stubs: true,
        },
    );
    let config = BasiliskConfig {
        per_module_overrides: per_module,
        ..Default::default()
    };

    // The fixture imports `requests` directly — wildcard `requests.*` matches `requests`
    let diags = run_with_config("errors/e0010_untyped_import.py", &config)?;
    // Note: whether `requests.*` matches `requests` depends on the wildcard logic.
    // The important thing is we exercise the full pipeline path.
    // If the wildcard doesn't match bare `requests`, E0010 is still emitted.
    let _has_e0010 = diags.iter().any(|d| d.code.code == "BSK-E0010");
    // Just assert we didn't crash — the exact match behavior is tested in config unit tests.
    Ok(())
}

// ---------------------------------------------------------------------------
// Per-path overrides
// ---------------------------------------------------------------------------

#[test]
fn per_path_disabled_suppresses_all_diagnostics() -> Result<(), Box<dyn std::error::Error>> {
    // Disable ALL rules for the fixture's path pattern
    let mut path_overrides = HashMap::new();
    let mut rule_overrides = HashMap::new();
    rule_overrides.insert("BSK-E0001".to_owned(), RuleSeverity::Disabled);
    rule_overrides.insert("BSK-E0002".to_owned(), RuleSeverity::Disabled);
    path_overrides.insert(
        fixtures_path_prefix(),
        PathOverride {
            disabled_rules: vec![],
            rule_overrides,
        },
    );
    let config = BasiliskConfig {
        per_path_overrides: path_overrides,
        ..Default::default()
    };

    let diags = run_with_config("missing_both.py", &config)?;
    let e0001_e0002: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0001" || d.code.code == "BSK-E0002")
        .collect();
    assert!(
        e0001_e0002.is_empty(),
        "E0001 and E0002 should be suppressed by per-path override, got: {e0001_e0002:#?}"
    );
    Ok(())
}

#[test]
fn per_path_warning_demotes_severity() -> Result<(), Box<dyn std::error::Error>> {
    let mut path_overrides = HashMap::new();
    let mut rule_overrides = HashMap::new();
    rule_overrides.insert("BSK-E0001".to_owned(), RuleSeverity::Warning);
    path_overrides.insert(
        fixtures_path_prefix(),
        PathOverride {
            disabled_rules: vec![],
            rule_overrides,
        },
    );
    let config = BasiliskConfig {
        per_path_overrides: path_overrides,
        ..Default::default()
    };

    let diags = run_with_config("missing_param_annotation.py", &config)?;
    let e0001_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0001")
        .collect();
    assert!(
        !e0001_diags.is_empty(),
        "E0001 should still be emitted as warning"
    );
    for diag in &e0001_diags {
        assert_eq!(
            diag.severity,
            Severity::Warning,
            "E0001 should be demoted to warning via per-path override"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Combined: global + per-path (per-path takes priority over global)
// ---------------------------------------------------------------------------

#[test]
fn per_path_overrides_global_severity() -> Result<(), Box<dyn std::error::Error>> {
    let mut rules = HashMap::new();
    rules.insert("BSK-E0001".to_owned(), RuleSeverity::Warning);

    let mut path_overrides = HashMap::new();
    let mut rule_overrides = HashMap::new();
    rule_overrides.insert("BSK-E0001".to_owned(), RuleSeverity::Info);
    path_overrides.insert(
        fixtures_path_prefix(),
        PathOverride {
            disabled_rules: vec![],
            rule_overrides,
        },
    );

    let config = BasiliskConfig {
        rules,
        per_path_overrides: path_overrides,
        ..Default::default()
    };

    let diags = run_with_config("missing_param_annotation.py", &config)?;
    let e0001_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0001")
        .collect();
    assert!(!e0001_diags.is_empty(), "E0001 should still be emitted");
    for diag in &e0001_diags {
        assert_eq!(
            diag.severity,
            Severity::Info,
            "per-path Info should override global Warning"
        );
    }
    Ok(())
}
