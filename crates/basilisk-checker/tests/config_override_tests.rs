//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Tests for project-level configuration overrides via `check_with_config`.
//!
//! Exercises the config-realized severity model [CHKARCH-STRICTNESS-SEVERITY]
//! (disable / demote-to-warning / demote-to-info / promote-to-error), the
//! per-path and per-module rungs of [CHKARCH-STRICTNESS-PRECEDENCE], and the
//! opt-in house-rule discipline of [CHKARCH-CONFIGURATION-ONLY].

use std::collections::HashMap;

use basilisk_config::{BasiliskConfig, ModuleOverride, PathOverride, RuleSeverity};

/// Parse source and check with the given config, returning diagnostics.
fn check_with(
    source: &str,
    path: &str,
    config: &BasiliskConfig,
) -> Vec<basilisk_checker::Diagnostic> {
    let parsed = basilisk_parser::parse_source(source.to_owned(), path.to_owned())
        .expect("source should parse");
    let resolved = basilisk_resolver::resolve(&parsed).expect("source should resolve");
    basilisk_checker::check_with_config(&resolved, config)
}

/// Parse source and check with default config, returning diagnostics.
fn check_default(source: &str, path: &str) -> Vec<basilisk_checker::Diagnostic> {
    check_with(source, path, &BasiliskConfig::default())
}

/// Config with explicit native severities for the opt-in rules used here.
fn annotations_on() -> BasiliskConfig {
    BasiliskConfig {
        rules: HashMap::from([
            ("BSK-E0001".to_owned(), RuleSeverity::Error),
            ("BSK-W0050".to_owned(), RuleSeverity::Warning),
        ]),
        ..Default::default()
    }
}

#[test]
fn explicit_global_severity_selects_an_opt_in_rule_without_a_tag_switch() {
    let source = "def foo(x):\n    return x\n";
    for (configured, expected) in [
        (RuleSeverity::Error, basilisk_checker::Severity::Error),
        (RuleSeverity::Warning, basilisk_checker::Severity::Warning),
        (RuleSeverity::Info, basilisk_checker::Severity::Info),
    ] {
        let config = BasiliskConfig {
            rules: HashMap::from([("BSK-E0001".to_owned(), configured)]),
            ..Default::default()
        };
        let diagnostics = check_with(source, "test.py", &config);
        let selected = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code.code == "BSK-E0001")
            .collect::<Vec<_>>();
        assert!(
            !selected.is_empty(),
            "an explicit {configured:?} severity must select the opt-in rule"
        );
        assert!(selected
            .iter()
            .all(|diagnostic| diagnostic.severity == expected));
    }
}

#[test]
fn inherited_and_explicitly_disabled_opt_in_rules_remain_off() {
    let source = "def foo(x):\n    return x\n";
    let inherited = check_with(source, "test.py", &BasiliskConfig::default());
    assert!(!inherited
        .iter()
        .any(|diagnostic| diagnostic.code.code == "BSK-E0001"));

    let disabled = BasiliskConfig {
        rules: HashMap::from([("BSK-E0001".to_owned(), RuleSeverity::Disabled)]),
        ..Default::default()
    };
    assert!(!check_with(source, "test.py", &disabled)
        .iter()
        .any(|diagnostic| diagnostic.code.code == "BSK-E0001"));
}

#[test]
fn explicit_per_path_severity_selects_an_opt_in_rule() {
    let source = "def foo(x):\n    return x\n";
    let config = BasiliskConfig {
        per_path_overrides: HashMap::from([(
            "src/**".to_owned(),
            PathOverride {
                disabled_rules: Vec::new(),
                rule_overrides: HashMap::from([("BSK-E0001".to_owned(), RuleSeverity::Warning)]),
            },
        )]),
        ..Default::default()
    };
    let diagnostics = check_with(source, "src/test.py", &config);
    let selected = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code.code == "BSK-E0001")
        .collect::<Vec<_>>();
    assert!(!selected.is_empty());
    assert!(selected
        .iter()
        .all(|diagnostic| diagnostic.severity == basilisk_checker::Severity::Warning));
}

#[test]
fn per_path_severity_can_reenable_a_globally_disabled_opt_in_rule() {
    let source = "def foo(x):\n    return x\n";
    let config = BasiliskConfig {
        rules: HashMap::from([("BSK-E0001".to_owned(), RuleSeverity::Disabled)]),
        per_path_overrides: HashMap::from([(
            "src/**".to_owned(),
            PathOverride {
                disabled_rules: Vec::new(),
                rule_overrides: HashMap::from([("BSK-E0001".to_owned(), RuleSeverity::Warning)]),
            },
        )]),
        ..Default::default()
    };
    let diagnostics = check_with(source, "src/test.py", &config);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code.code == "BSK-E0001"
            && diagnostic.severity == basilisk_checker::Severity::Warning
    }));
}

#[test]
fn global_rule_severity_override_disables_rule() {
    let source = "def foo(x):\n    return x\n";
    let enabled_diags = check_with(source, "test.py", &annotations_on());
    let e0001_count = enabled_diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0001")
        .count();
    assert!(
        e0001_count > 0,
        "BSK-E0001 should fire once the house rule is enabled in config"
    );

    let config = BasiliskConfig {
        rules: HashMap::from([("BSK-E0001".to_owned(), RuleSeverity::Disabled)]),
        ..annotations_on()
    };
    let diags = check_with(source, "test.py", &config);
    let e0001_after = diags.iter().filter(|d| d.code.code == "BSK-E0001").count();
    assert_eq!(
        e0001_after, 0,
        "BSK-E0001 should be suppressed when disabled"
    );
}

#[test]
fn global_rule_severity_override_demotes_to_warning() {
    let source = "def foo(x):\n    return x\n";
    let config = BasiliskConfig {
        rules: HashMap::from([("BSK-E0001".to_owned(), RuleSeverity::Warning)]),
        ..annotations_on()
    };
    let diags = check_with(source, "test.py", &config);
    let e0001_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0001")
        .collect();
    assert!(!e0001_diags.is_empty(), "BSK-E0001 should still fire");
    for diag in &e0001_diags {
        assert_eq!(
            diag.severity,
            basilisk_checker::Severity::Warning,
            "BSK-E0001 should be demoted to warning"
        );
    }
}

#[test]
fn global_rule_severity_override_demotes_to_info() {
    let source = "def foo(x):\n    return x\n";
    let config = BasiliskConfig {
        rules: HashMap::from([("BSK-E0001".to_owned(), RuleSeverity::Info)]),
        ..annotations_on()
    };
    let diags = check_with(source, "test.py", &config);
    let e0001_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0001")
        .collect();
    assert!(!e0001_diags.is_empty(), "BSK-E0001 should still fire");
    for diag in &e0001_diags {
        assert_eq!(
            diag.severity,
            basilisk_checker::Severity::Info,
            "BSK-E0001 should be demoted to info"
        );
    }
}

#[test]
fn global_rule_severity_override_promotes_warning_to_error() {
    // BSK-W0050 (redundant annotation) is a house rule a project opts into. Once
    // enabled it defaults to a warning, and `rules."BSK-W0050" = "error"` must be
    // able to promote it to a hard ERROR — letting a project dial severity UP, not
    // just down. See [CHKARCH-CONFIGURATION-ONLY].
    let source = "x: int = 42\n";
    let enabled_diags = check_with(source, "test.py", &annotations_on());
    let w0050_default: Vec<_> = enabled_diags
        .iter()
        .filter(|d| d.code.code == "BSK-W0050")
        .collect();
    assert!(
        !w0050_default.is_empty(),
        "BSK-W0050 should fire once the house rule is enabled in config"
    );
    assert_eq!(
        w0050_default[0].severity,
        basilisk_checker::Severity::Warning,
        "BSK-W0050 defaults to warning"
    );

    let config = BasiliskConfig {
        rules: HashMap::from([("BSK-W0050".to_owned(), RuleSeverity::Error)]),
        ..annotations_on()
    };
    let diags = check_with(source, "test.py", &config);
    let promoted: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-W0050")
        .collect();
    assert!(
        !promoted.is_empty(),
        "BSK-W0050 should still fire when promoted to error"
    );
    for diag in &promoted {
        assert_eq!(
            diag.severity,
            basilisk_checker::Severity::Error,
            "BSK-W0050 should be promoted to a hard error via config"
        );
    }
}

#[test]
fn per_path_override_disables_rule() {
    let source = "def foo(x):\n    return x\n";

    // With the house rule enabled but no path override, BSK-E0001 fires.
    let enabled_diags = check_with(source, "vendor/lib/foo.py", &annotations_on());
    let has_e0001 = enabled_diags.iter().any(|d| d.code.code == "BSK-E0001");
    assert!(has_e0001, "BSK-E0001 should fire without path override");

    // With per-path override disabling BSK-E0001 for vendor/**.
    let config = BasiliskConfig {
        per_path_overrides: HashMap::from([(
            "vendor/**".to_owned(),
            PathOverride {
                disabled_rules: vec!["BSK-E0001".to_owned()],
                rule_overrides: HashMap::new(),
            },
        )]),
        ..annotations_on()
    };
    let diags = check_with(source, "vendor/lib/foo.py", &config);
    let has_e0001_after = diags.iter().any(|d| d.code.code == "BSK-E0001");
    assert!(
        !has_e0001_after,
        "BSK-E0001 should be disabled for vendor/**"
    );
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
    let has_e0010 = default_diags
        .iter()
        .any(|d| d.code.code == "imports_unresolved");
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
    let has_e0010_after = diags.iter().any(|d| d.code.code == "imports_unresolved");
    assert!(
        !has_e0010_after,
        "E0010 should be suppressed for fastmcp with ignore-missing-stubs"
    );
}

#[test]
fn per_module_override_only_suppresses_the_matching_import() {
    let source = "import fastmcp\nimport definitely_missing_basilisk_module\n";
    let config = BasiliskConfig {
        per_module_overrides: HashMap::from([(
            "fastmcp".to_owned(),
            ModuleOverride {
                ignore_missing_stubs: true,
            },
        )]),
        ..Default::default()
    };

    let unresolved = check_with(source, "test.py", &config)
        .into_iter()
        .filter(|diagnostic| diagnostic.code.code == "imports_unresolved")
        .collect::<Vec<_>>();

    assert_eq!(
        unresolved.len(),
        1,
        "ignoring one missing module must not hide unrelated unresolved imports"
    );
    assert!(
        unresolved[0]
            .message
            .contains("definitely_missing_basilisk_module"),
        "the unrelated unresolved import must remain visible"
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
    let has_e0010 = diags.iter().any(|d| d.code.code == "imports_unresolved");
    assert!(
        !has_e0010,
        "E0010 should be suppressed for django.* wildcard"
    );
}
