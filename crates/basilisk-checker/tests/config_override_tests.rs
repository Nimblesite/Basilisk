//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Tests for the configuration model via `check_with_config`.
//!
//! Exercises [CHKARCH-CONFIG-MODEL] (per-rule entries, tag entries, rule
//! entry over tag entry, strictest matching tag), the command partition
//! [CHKARCH-COMMANDS] (`pep` rules always run and can never be disabled;
//! everything else runs only when configuration decides it), and the
//! severity values of [CHKARCH-STRICTNESS-SEVERITY], and the unrun-rule count
//! behind [CHKARCH-CLI-SCOPE-NOTICE]. Code under test:
//! `basilisk-checker/src/lib.rs` (`EffectiveRuleConfig`,
//! `pep_disable_violations`, `analyze_selected_rules`, `is_pep_rule`).

use std::collections::HashMap;

use basilisk_config::{BasiliskConfig, RuleSeverity, RuleTables};

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

/// A config whose single table holds the given rule and tag entries.
fn config_with(
    rules: &[(&str, RuleSeverity)],
    rule_tags: &[(&str, RuleSeverity)],
) -> BasiliskConfig {
    BasiliskConfig {
        rule_chain: vec![RuleTables {
            rules: rules
                .iter()
                .map(|(code, severity)| ((*code).to_owned(), *severity))
                .collect(),
            rule_tags: rule_tags
                .iter()
                .map(|(tag, severity)| ((*tag).to_owned(), *severity))
                .collect(),
        }],
        ..Default::default()
    }
}

/// [CHKARCH-CONFIG-MODEL]: an explicit per-rule entry runs an analyze rule at
/// exactly that severity.
#[test]
fn rule_entry_selects_an_analyze_rule_at_each_severity() {
    let source = "def foo(x):\n    return x\n";
    for (configured, expected) in [
        (RuleSeverity::Error, basilisk_checker::Severity::Error),
        (RuleSeverity::Warning, basilisk_checker::Severity::Warning),
        (RuleSeverity::Info, basilisk_checker::Severity::Info),
    ] {
        let config = config_with(&[("BSK-0001", configured)], &[]);
        let diagnostics = check_with(source, "test.py", &config);
        let selected = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code.code == "BSK-0001")
            .collect::<Vec<_>>();
        assert!(
            !selected.is_empty(),
            "an explicit {configured:?} entry must run the analyze rule"
        );
        assert!(selected
            .iter()
            .all(|diagnostic| diagnostic.severity == expected));
    }
}

/// [CHKARCH-CONFIG-MODEL]: no entry means no check; an explicit `disabled`
/// entry means the same. Analyze rules are tabula rasa.
#[test]
fn analyze_rule_without_entry_or_disabled_stays_off() {
    let source = "def foo(x):\n    return x\n";
    let no_entry = check_default(source, "test.py");
    assert!(!no_entry
        .iter()
        .any(|diagnostic| diagnostic.code.code == "BSK-0001"));

    let disabled = config_with(&[("BSK-0001", RuleSeverity::Disabled)], &[]);
    assert!(!check_with(source, "test.py", &disabled)
        .iter()
        .any(|diagnostic| diagnostic.code.code == "BSK-0001"));
}

/// [CHKARCH-CONFIG-MODEL]: one tag entry grades every rule carrying the tag —
/// the two-line seed (`rule-tags."basilisk" = "error"`) turns every house
/// rule on at error ([LSPARCH-CONFIG-SEEDING]).
#[test]
fn tag_entry_enables_every_rule_carrying_the_tag() {
    let source = "def foo(x):\n    return x\n";
    let config = config_with(&[], &[("basilisk", RuleSeverity::Error)]);
    let diagnostics = check_with(source, "test.py", &config);
    let e0001: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-0001")
        .collect();
    assert!(
        !e0001.is_empty(),
        "the basilisk tag entry must run the require-annotation rule"
    );
    assert!(e0001
        .iter()
        .all(|d| d.severity == basilisk_checker::Severity::Error));
}

/// [CHKARCH-CONFIG-MODEL]: within a table a per-rule entry beats tag entries.
#[test]
fn rule_entry_beats_tag_entry() {
    let source = "def foo(x):\n    return x\n";
    let config = config_with(
        &[("BSK-0001", RuleSeverity::Info)],
        &[("basilisk", RuleSeverity::Error)],
    );
    let diagnostics = check_with(source, "test.py", &config);
    let selected: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-0001")
        .collect();
    assert!(!selected.is_empty());
    assert!(
        selected
            .iter()
            .all(|d| d.severity == basilisk_checker::Severity::Info),
        "the per-rule entry must override the tag entry"
    );
}

/// [CHKARCH-CONFIG-MODEL]: among matching tag entries the strictest severity
/// wins. BSK-0050 carries `basilisk` and `redundancy`.
#[test]
fn strictest_matching_tag_entry_wins() {
    let source = "x: int = 42\n";
    let config = config_with(
        &[],
        &[
            ("basilisk", RuleSeverity::Info),
            ("redundancy", RuleSeverity::Error),
        ],
    );
    let diagnostics = check_with(source, "test.py", &config);
    let w0050: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-0050")
        .collect();
    assert!(!w0050.is_empty(), "the tag entries must run BSK-0050");
    assert!(
        w0050
            .iter()
            .all(|d| d.severity == basilisk_checker::Severity::Error),
        "error must beat info among overlapping tag entries"
    );
}

/// [CHKARCH-CONFIG-MODEL]: entries dial severity up as well as down — a
/// house rule promoted to a hard error.
#[test]
fn rule_entry_promotes_to_error() {
    let source = "x: int = 42\n";
    let config = config_with(&[("BSK-0050", RuleSeverity::Error)], &[]);
    let diagnostics = check_with(source, "test.py", &config);
    let promoted: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-0050")
        .collect();
    assert!(!promoted.is_empty());
    assert!(promoted
        .iter()
        .all(|d| d.severity == basilisk_checker::Severity::Error));
}

/// [CHKARCH-COMMANDS]: `pep` rules run with no config at all — the bare-tree
/// conformance surface — and analyze rules do not.
#[test]
fn bare_config_runs_pep_rules_only() {
    let source = "import definitely_missing_basilisk_module\n\ndef foo(x):\n    return x\n";
    let diagnostics = check_default(source, "test.py");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code.code == "imports_unresolved"),
        "pep rules must fire with no config"
    );
    assert!(
        diagnostics
            .iter()
            .all(|d| basilisk_checker::is_pep_rule(d.code.code)),
        "no analyze rule may fire with no config"
    );
}

/// [CHKARCH-CONFIG-MODEL]: PEP rules can be graded — never disabled.
#[test]
fn pep_rule_can_be_graded_down() {
    let source = "import definitely_missing_basilisk_module\n";
    let config = config_with(&[("imports_unresolved", RuleSeverity::Warning)], &[]);
    let diagnostics = check_with(source, "test.py", &config);
    let graded: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "imports_unresolved")
        .collect();
    assert!(!graded.is_empty(), "the graded pep rule must still fire");
    assert!(graded
        .iter()
        .all(|d| d.severity == basilisk_checker::Severity::Warning));
}

/// [CHKARCH-CONFIG-MODEL]: a `disabled` resolution never applies to a `pep`
/// rule — the rule keeps firing, and the config is reported invalid through
/// [`basilisk_checker::pep_disable_violations`].
#[test]
fn pep_rule_disable_is_invalid_and_never_applied() {
    let source = "import definitely_missing_basilisk_module\n";
    let config = config_with(&[("imports_unresolved", RuleSeverity::Disabled)], &[]);

    let diagnostics = check_with(source, "test.py", &config);
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code.code == "imports_unresolved"),
        "a pep rule must keep firing even when config tries to disable it"
    );

    let violations = basilisk_checker::pep_disable_violations(&config);
    assert!(
        violations.contains(&"imports_unresolved"),
        "the invalid pep-disable must be reported: {violations:?}"
    );

    let clean = config_with(&[("imports_unresolved", RuleSeverity::Warning)], &[]);
    assert!(
        basilisk_checker::pep_disable_violations(&clean).is_empty(),
        "grading a pep rule is valid"
    );
}

/// [CHKARCH-CLI-SCOPE-NOTICE] (Refs #334): `analyze_selected_rules` reports
/// exactly the rules configuration selects that `check` will never evaluate —
/// the count a clean `check` run tells the user about.
#[test]
fn analyze_selected_rules_reports_what_check_will_not_run() {
    assert!(
        basilisk_checker::analyze_selected_rules(&BasiliskConfig::default()).is_empty(),
        "a bare tree selects no analyze rule, so `check` hides nothing"
    );

    let tagged = config_with(&[], &[("basilisk", RuleSeverity::Error)]);
    let selected = basilisk_checker::analyze_selected_rules(&tagged);
    assert!(
        selected.contains(&"BSK-0001"),
        "one `basilisk` tag entry selects the house rules: {selected:?}"
    );
    assert!(
        selected
            .iter()
            .all(|code| !basilisk_checker::is_pep_rule(code)),
        "a `pep` rule always runs under check, so it is never reported unrun: {selected:?}"
    );

    let graded_pep = config_with(&[("imports_unresolved", RuleSeverity::Warning)], &[]);
    assert!(
        basilisk_checker::analyze_selected_rules(&graded_pep).is_empty(),
        "grading a pep rule configures a rule `check` does run"
    );

    let disabled = config_with(&[("BSK-0001", RuleSeverity::Disabled)], &[]);
    assert!(
        !basilisk_checker::analyze_selected_rules(&disabled).contains(&"BSK-0001"),
        "a disabled rule is not selected, so it is not something `check` hid"
    );
}

/// [CHKARCH-COMMANDS]: the partition is the `pep` provenance tag.
#[test]
fn partition_is_the_pep_tag() {
    assert!(basilisk_checker::is_pep_rule("imports_unresolved"));
    assert!(basilisk_checker::is_pep_rule("returns_compatibility"));
    assert!(!basilisk_checker::is_pep_rule("BSK-0001"));
    assert!(!basilisk_checker::is_pep_rule("BSK-0050"));
}

/// The default config is behaviourally identical to passing no config.
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

/// [CHKARCH-CONFIG-MODEL]: `with_rule_entries` and a hand-built chain agree.
#[test]
fn with_rule_entries_matches_hand_built_chain() {
    let by_helper = BasiliskConfig::with_rule_entries(HashMap::from([(
        "BSK-0001".to_owned(),
        RuleSeverity::Warning,
    )]));
    let by_hand = config_with(&[("BSK-0001", RuleSeverity::Warning)], &[]);
    assert_eq!(
        by_helper.resolve_severity("BSK-0001", &["basilisk"]),
        by_hand.resolve_severity("BSK-0001", &["basilisk"]),
    );
}
