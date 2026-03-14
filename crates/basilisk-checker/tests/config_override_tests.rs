//! Tests for project-level configuration overrides via `check_with_config`.

use std::collections::HashMap;

use basilisk_config::{BasiliskConfig, ModuleOverride, PathOverride, RuleSeverity};

/// Parse source and check with the given config, returning diagnostics.
fn check_with(source: &str, path: &str, config: &BasiliskConfig) -> Vec<basilisk_checker::Diagnostic> {
    let parsed = basilisk_parser::parse_source(source.to_owned(), path.to_owned())
        .expect("source should parse");
    let resolved = basilisk_resolver::resolve(&parsed).expect("source should resolve");
    basilisk_checker::check_with_config(&resolved, config)
}

/// Parse source and check with default config, returning diagnostics.
fn check_default(source: &str, path: &str) -> Vec<basilisk_checker::Diagnostic> {
    check_with(source, path, &BasiliskConfig::default())
}

#[test]
fn global_rule_severity_override_disables_rule() {
    let source = "def foo(x):\n    return x\n";
    let default_diags = check_default(source, "test.py");
    let e0001_count = default_diags.iter().filter(|d| d.code.code == "BSK-E0001").count();
    assert!(e0001_count > 0, "E0001 should fire without config override");

    let config = BasiliskConfig {
        rules: HashMap::from([("BSK-E0001".to_owned(), RuleSeverity::Disabled)]),
        ..Default::default()
    };
    let diags = check_with(source, "test.py", &config);
    let e0001_after = diags.iter().filter(|d| d.code.code == "BSK-E0001").count();
    assert_eq!(e0001_after, 0, "E0001 should be suppressed when disabled");
}

#[test]
fn global_rule_severity_override_demotes_to_warning() {
    let source = "def foo(x):\n    return x\n";
    let config = BasiliskConfig {
        rules: HashMap::from([("BSK-E0001".to_owned(), RuleSeverity::Warning)]),
        ..Default::default()
    };
    let diags = check_with(source, "test.py", &config);
    let e0001_diags: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0001").collect();
    assert!(!e0001_diags.is_empty(), "E0001 should still fire");
    for diag in &e0001_diags {
        assert_eq!(
            diag.severity,
            basilisk_checker::Severity::Warning,
            "E0001 should be demoted to warning"
        );
    }
}

#[test]
fn global_rule_severity_override_demotes_to_info() {
    let source = "def foo(x):\n    return x\n";
    let config = BasiliskConfig {
        rules: HashMap::from([("BSK-E0001".to_owned(), RuleSeverity::Info)]),
        ..Default::default()
    };
    let diags = check_with(source, "test.py", &config);
    let e0001_diags: Vec<_> = diags.iter().filter(|d| d.code.code == "BSK-E0001").collect();
    assert!(!e0001_diags.is_empty(), "E0001 should still fire");
    for diag in &e0001_diags {
        assert_eq!(
            diag.severity,
            basilisk_checker::Severity::Info,
            "E0001 should be demoted to info"
        );
    }
}

#[test]
fn per_path_override_disables_rule() {
    let source = "def foo(x):\n    return x\n";

    // Without override, E0001 fires.
    let default_diags = check_default(source, "vendor/lib/foo.py");
    let has_e0001 = default_diags.iter().any(|d| d.code.code == "BSK-E0001");
    assert!(has_e0001, "E0001 should fire without path override");

    // With per-path override disabling E0001 for vendor/**.
    let config = BasiliskConfig {
        per_path_overrides: HashMap::from([(
            "vendor/**".to_owned(),
            PathOverride {
                disabled_rules: vec!["BSK-E0001".to_owned()],
                rule_overrides: HashMap::new(),
            },
        )]),
        ..Default::default()
    };
    let diags = check_with(source, "vendor/lib/foo.py", &config);
    let has_e0001_after = diags.iter().any(|d| d.code.code == "BSK-E0001");
    assert!(!has_e0001_after, "E0001 should be disabled for vendor/**");
}

#[test]
fn default_config_does_not_change_behaviour() {
    let source = "def foo(x):\n    return x\n";
    let default_diags = check_default(source, "test.py");
    let config_diags = check_with(source, "test.py", &BasiliskConfig::default());
    assert_eq!(
        default_diags.len(),
        config_diags.len(),
        "default config should produce identical diagnostics"
    );
}

#[test]
fn per_module_override_suppresses_e0010() {
    // E0010 fires for unresolved third-party imports.
    let source = "import fastmcp\n";
    let default_diags = check_default(source, "test.py");
    let has_e0010 = default_diags.iter().any(|d| d.code.code == "BSK-E0010");
    assert!(has_e0010, "E0010 should fire for unresolved import");

    let config = BasiliskConfig {
        per_module_overrides: HashMap::from([(
            "fastmcp".to_owned(),
            ModuleOverride {
                ignore_missing_stubs: true,
            },
        )]),
        ..Default::default()
    };
    let diags = check_with(source, "test.py", &config);
    let has_e0010_after = diags.iter().any(|d| d.code.code == "BSK-E0010");
    assert!(
        !has_e0010_after,
        "E0010 should be suppressed for fastmcp with ignore-missing-stubs"
    );
}

#[test]
fn per_module_wildcard_override() {
    let source = "import django.db.models\n";
    let config = BasiliskConfig {
        per_module_overrides: HashMap::from([(
            "django.*".to_owned(),
            ModuleOverride {
                ignore_missing_stubs: true,
            },
        )]),
        ..Default::default()
    };
    let diags = check_with(source, "test.py", &config);
    let has_e0010 = diags.iter().any(|d| d.code.code == "BSK-E0010");
    assert!(
        !has_e0010,
        "E0010 should be suppressed for django.* wildcard"
    );
}
