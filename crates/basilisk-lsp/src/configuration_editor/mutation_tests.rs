//! Tests for [CONFIGEDITOR-OPERATIONS] mutation validation and projection.
//! See `mutation.rs` for the implementation.

#![expect(
    clippy::panic,
    reason = "test-only destructuring failures should abort with a focused message"
)]

use std::collections::HashMap;
use std::path::PathBuf;

use basilisk_config::{
    build_rule_patch, BasiliskConfig, ConfigDocument, ConfigFormat, PathOverride, RuleConfigScope,
    RuleConfigUpdate, RuleSeverity,
};

use super::{
    build_impact, expand_mutations, require_revision, resolved_changes, selection_error,
    validate_document_rules, validate_path_pattern, validate_selector,
};
use crate::configuration_editor::catalog::{descriptors, SelectionError};
use crate::configuration_editor::model::{
    ConfigurationMutation, MutationScope, PreviewConfigurationRequest, RuleSelector, RuleSetting,
};
use crate::configuration_editor::snapshot::Inventory;

#[test]
fn empty_code_and_tag_selectors_are_rejected() {
    assert!(validate_selector(&RuleSelector::Codes { codes: Vec::new() }).is_err());
    assert!(validate_selector(&RuleSelector::Tags {
        tags: Vec::new(),
        match_all: true,
    })
    .is_err());
}

#[test]
fn path_patterns_are_project_relative_portable_globs() {
    for accepted in [
        "legacy/**",
        "src/*.py",
        "tests/test_?.py",
        "src/app.py",
        "**/generated/**",
    ] {
        assert!(
            validate_path_pattern(accepted).is_ok(),
            "rejected {accepted}"
        );
    }
    for rejected in [
        "",
        " ",
        ".",
        " . ",
        "./",
        " legacy/**",
        "./legacy/**",
        "legacy/./**",
        "../legacy/**",
        "legacy/../**",
        "/absolute/**",
        "//server/share/**",
        "C:/absolute/**",
        "C:\\absolute\\**",
        "legacy\\**",
        "legacy//**",
        "legacy/",
        "legacy/\0file.py",
    ] {
        assert!(
            validate_path_pattern(rejected).is_err(),
            "accepted {rejected:?}"
        );
    }
}

#[test]
fn strict_recipe_expands_every_catalog_rule_to_an_explicit_native_value() {
    let catalog = descriptors();
    let request = PreviewConfigurationRequest {
        root_uri: "file:///workspace".to_owned(),
        base_revision: "revision".to_owned(),
        mutations: vec![ConfigurationMutation {
            selector: RuleSelector::All,
            setting: RuleSetting::Native,
            scope: MutationScope::Project,
        }],
    };
    let result = expand_mutations(&request, &catalog, &HashMap::new());
    let Ok((updates, codes)) = result else {
        panic!("strict selector must expand");
    };
    assert_eq!(codes.len(), catalog.len());
    let Some(update) = updates.first() else {
        panic!("strict selector must produce a project update");
    };
    assert_eq!(update.scope, RuleConfigScope::Project);
    assert_eq!(update.rules.len(), catalog.len());
    assert!(update.rules.values().all(Option::is_some));
}

// Implements [CONFIGEDITOR-OPERATIONS]: mutations sharing a scope merge into
// one grouped update instead of producing conflicting patch entries.
#[test]
fn mutations_group_by_scope_and_reject_invalid_path_patterns() {
    let catalog = descriptors();
    let path_mutation = |pattern: &str, code: &str| ConfigurationMutation {
        selector: RuleSelector::Codes {
            codes: vec![code.to_owned()],
        },
        setting: RuleSetting::Warning,
        scope: MutationScope::Path {
            pattern: pattern.to_owned(),
        },
    };
    let request = PreviewConfigurationRequest {
        root_uri: "file:///workspace".to_owned(),
        base_revision: "revision".to_owned(),
        mutations: vec![
            path_mutation("legacy/**", "BSK-E0001"),
            path_mutation("legacy/**", "BSK-E0002"),
        ],
    };
    let result = expand_mutations(&request, &catalog, &HashMap::new());
    let Ok((updates, mut codes)) = result else {
        panic!("path mutations must expand");
    };
    codes.sort();
    assert_eq!(codes, vec!["BSK-E0001".to_owned(), "BSK-E0002".to_owned()]);
    let [update] = updates.as_slice() else {
        panic!("same-scope mutations must group into one update");
    };
    assert_eq!(
        update.scope,
        RuleConfigScope::Path {
            pattern: "legacy/**".to_owned(),
            adoption: false,
        }
    );
    assert_eq!(update.rules.len(), 2);

    let invalid = PreviewConfigurationRequest {
        root_uri: "file:///workspace".to_owned(),
        base_revision: "revision".to_owned(),
        mutations: vec![path_mutation("../escape/**", "BSK-E0001")],
    };
    let Err(error) = expand_mutations(&invalid, &catalog, &HashMap::new()) else {
        panic!("absolute-escaping path pattern must fail");
    };
    assert_eq!(
        error.data.as_ref().and_then(|data| data.get("kind")),
        Some(&serde_json::json!("invalidMutation"))
    );
}

#[test]
fn resolved_changes_are_concrete_scoped_and_omit_noops() {
    let mut document = document(BasiliskConfig::default());
    let _ = document
        .config
        .rules
        .insert("BSK-E0001".to_owned(), RuleSeverity::Warning);
    let _ = document.config.per_path_overrides.insert(
        "legacy/**".to_owned(),
        PathOverride {
            disabled_rules: vec!["BSK-E0002".to_owned()],
            rule_overrides: HashMap::new(),
        },
    );
    let updates = vec![
        RuleConfigUpdate {
            scope: RuleConfigScope::Project,
            rules: std::collections::BTreeMap::from([
                ("BSK-E0001".to_owned(), Some(RuleSeverity::Warning)),
                ("BSK-E0002".to_owned(), Some(RuleSeverity::Error)),
            ]),
        },
        RuleConfigUpdate {
            scope: RuleConfigScope::Path {
                pattern: "legacy/**".to_owned(),
                adoption: false,
            },
            rules: std::collections::BTreeMap::from([("BSK-E0002".to_owned(), None)]),
        },
    ];

    let changes = resolved_changes(&document, &updates);
    let [project, path] = changes.as_slice() else {
        panic!("expected one project and one path change");
    };
    assert_eq!(project.rule_code, "BSK-E0002");
    assert_eq!(project.scope, MutationScope::Project);
    assert_eq!(project.previous_setting, RuleSetting::Inherit);
    assert_eq!(project.resulting_setting, RuleSetting::Error);
    assert_eq!(
        path.scope,
        MutationScope::Path {
            pattern: "legacy/**".to_owned()
        }
    );
    assert_eq!(path.previous_setting, RuleSetting::Disabled);
    assert_eq!(path.resulting_setting, RuleSetting::Inherit);
}

// A path-scope override that already configures the rule reports its current
// severity as the previous setting, not Inherit.
#[test]
fn resolved_changes_read_existing_path_overrides() {
    let mut document = document(BasiliskConfig::default());
    let _ = document.config.per_path_overrides.insert(
        "legacy/**".to_owned(),
        PathOverride {
            disabled_rules: Vec::new(),
            rule_overrides: HashMap::from([("BSK-E0001".to_owned(), RuleSeverity::Warning)]),
        },
    );
    let updates = vec![RuleConfigUpdate {
        scope: RuleConfigScope::Path {
            pattern: "legacy/**".to_owned(),
            adoption: false,
        },
        rules: std::collections::BTreeMap::from([(
            "BSK-E0001".to_owned(),
            Some(RuleSeverity::Error),
        )]),
    }];
    let changes = resolved_changes(&document, &updates);
    let [change] = changes.as_slice() else {
        panic!("expected exactly one path change");
    };
    assert_eq!(change.previous_setting, RuleSetting::Warning);
    assert_eq!(change.resulting_setting, RuleSetting::Error);
}

#[test]
fn unknown_rule_in_active_config_is_rejected_at_protocol_boundary() {
    let mut config = BasiliskConfig::default();
    let _ = config.rules.insert(
        "NOT-A-RULE".to_owned(),
        basilisk_config::RuleSeverity::Warning,
    );
    let document = document(config);
    let Err(error) = validate_document_rules(&document) else {
        panic!("unknown configured rule must fail");
    };
    assert_eq!(
        error.data.as_ref().and_then(|data| data.get("kind")),
        Some(&serde_json::json!("unknownRule"))
    );
    assert_eq!(
        error
            .data
            .as_ref()
            .and_then(|data| data.pointer("/context/sourceUri")),
        Some(&serde_json::json!("file:///workspace/basilisk.json"))
    );
}

// Implements [CONFIGEDITOR-OPERATIONS]: impact reports the projected rule and
// diagnostic movement a preview would cause, straight from both inventories.
#[test]
fn impact_projects_rule_and_diagnostic_movement() {
    let document = document(BasiliskConfig::default());
    let update = RuleConfigUpdate {
        scope: RuleConfigScope::Project,
        rules: std::collections::BTreeMap::from([(
            "BSK-E0001".to_owned(),
            Some(RuleSeverity::Disabled),
        )]),
    };
    let patch = build_rule_patch(&document, std::slice::from_ref(&update));
    let Ok(patch) = patch else {
        panic!("patch over a default document must render");
    };
    let changes = resolved_changes(&document, std::slice::from_ref(&update));
    let catalog = descriptors();
    let before = inventory_fixture(7, 5, 2);
    let after = inventory_fixture(4, 3, 1);

    let impact = build_impact(&patch, &catalog, &changes, &before, &after);

    assert_eq!(impact.changed_rules, 1);
    assert_eq!(impact.diagnostics_before, 7);
    assert_eq!(impact.diagnostics_after, 4);
    assert_eq!(impact.errors_before, 5);
    assert_eq!(impact.errors_after, 3);
    assert_eq!(impact.warnings_before, 2);
    assert_eq!(impact.warnings_after, 1);
    let enabled = usize::try_from(impact.enabled_rules).unwrap_or(0);
    let disabled = usize::try_from(impact.disabled_rules).unwrap_or(0);
    assert_eq!(enabled + disabled, catalog.len());
    // The patch disabled BSK-E0001, so it must land on the disabled side.
    assert!(disabled >= 1);
}

#[test]
fn revision_gate_reports_both_revisions_on_conflict() {
    let document = document(BasiliskConfig::default());
    assert!(require_revision(&document, "revision").is_ok());
    let Err(error) = require_revision(&document, "stale") else {
        panic!("stale revision must conflict");
    };
    assert_eq!(
        error.data.as_ref().and_then(|data| data.get("kind")),
        Some(&serde_json::json!("revisionConflict"))
    );
    assert_eq!(
        error
            .data
            .as_ref()
            .and_then(|data| data.pointer("/context/expected")),
        Some(&serde_json::json!("stale"))
    );
    assert_eq!(
        error
            .data
            .as_ref()
            .and_then(|data| data.pointer("/context/actual")),
        Some(&serde_json::json!("revision"))
    );
}

#[test]
fn selection_errors_carry_their_offending_name() {
    let rule = selection_error(SelectionError::UnknownRule("NOT-A-RULE".to_owned()));
    assert_eq!(
        rule.data.as_ref().and_then(|data| data.get("kind")),
        Some(&serde_json::json!("unknownRule"))
    );
    assert_eq!(
        rule.data
            .as_ref()
            .and_then(|data| data.pointer("/context/rule")),
        Some(&serde_json::json!("NOT-A-RULE"))
    );
    let tag = selection_error(SelectionError::UnknownTag("not-a-tag".to_owned()));
    assert_eq!(
        tag.data.as_ref().and_then(|data| data.get("kind")),
        Some(&serde_json::json!("unknownTag"))
    );
    assert_eq!(
        tag.data
            .as_ref()
            .and_then(|data| data.pointer("/context/tag")),
        Some(&serde_json::json!("not-a-tag"))
    );
}

fn inventory_fixture(total: usize, errors: usize, warnings: usize) -> Inventory {
    Inventory {
        counts: HashMap::new(),
        files: HashMap::new(),
        total,
        errors,
        warnings,
    }
}

fn document(config: BasiliskConfig) -> ConfigDocument {
    ConfigDocument {
        root: PathBuf::from("/workspace"),
        path: PathBuf::from("/workspace/basilisk.json"),
        format: ConfigFormat::BasiliskJson,
        exists: true,
        read_only: false,
        shadowed_sources: Vec::new(),
        content: "{}".to_owned(),
        revision: "revision".to_owned(),
        config,
    }
}
