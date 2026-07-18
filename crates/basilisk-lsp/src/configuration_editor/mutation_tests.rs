//! Tests for [CONFIGEDITOR-OPERATIONS] mutation validation and projection.
//! See `mutation.rs` for the implementation.

#![expect(
    clippy::panic,
    reason = "test-only destructuring failures should abort with a focused message"
)]

use std::collections::HashMap;
use std::path::PathBuf;

use basilisk_config::{
    build_configuration_patch, BasiliskConfig, ConfigDocument, RuleSeverity, TypeshedConfigKey,
    TypeshedConfigValue,
};

use super::{
    build_impact, build_update, require_mutations, require_no_pep_disable, require_revision,
    require_valid_typeshed_configuration, resolved_changes, resolved_typeshed_changes,
    selection_error, validate_document_rules,
};
use crate::configuration_editor::catalog::{descriptors, SelectionError};
use crate::configuration_editor::model::{
    EditorMutation, RuleSeverity as WireSeverity, TypeshedSettingKey, TypeshedSettingValue,
};
use crate::configuration_editor::snapshot::Inventory;

fn error_kind(error: &tower_lsp::jsonrpc::Error) -> Option<serde_json::Value> {
    error
        .data
        .as_ref()
        .and_then(|data| data.get("kind"))
        .cloned()
}

/// A `pep`-tagged and an analyze-scope rule code from the live registry.
fn partitioned_codes() -> (String, String) {
    let catalog = descriptors();
    let pep = catalog
        .iter()
        .find(|rule| basilisk_checker::is_pep_rule(&rule.code))
        .map(|rule| rule.code.clone());
    let analyze = catalog
        .iter()
        .find(|rule| !basilisk_checker::is_pep_rule(&rule.code) && rule.code != "BSK-0001")
        .map(|rule| rule.code.clone());
    let (Some(pep), Some(analyze)) = (pep, analyze) else {
        panic!("registry must hold both pep and analyze rules");
    };
    (pep, analyze)
}

/// [CONFIGEDITOR-OPERATIONS]: an empty mutation list is rejected before any
/// state is touched.
#[test]
fn empty_mutation_lists_are_rejected() {
    let Err(error) = require_mutations(&[]) else {
        panic!("empty mutation list must fail");
    };
    assert_eq!(
        error_kind(&error),
        Some(serde_json::json!("invalidMutation"))
    );
    assert!(require_mutations(&[EditorMutation::RemoveTag {
        tag: "basilisk".to_owned(),
    }])
    .is_ok());
}

/// [CONFIGEDITOR-OPERATIONS]: all six mutation kinds fold into one update.
#[test]
fn build_update_folds_all_six_mutations_into_entry_updates() {
    let catalog = descriptors();
    let (_, analyze) = partitioned_codes();
    let update = build_update(
        &[
            EditorMutation::SetRule {
                code: analyze.clone(),
                severity: WireSeverity::Warning,
            },
            EditorMutation::RemoveRule {
                code: "BSK-0001".to_owned(),
            },
            EditorMutation::SetTag {
                tag: "basilisk".to_owned(),
                severity: WireSeverity::Error,
            },
            EditorMutation::RemoveTag {
                tag: "suppressions".to_owned(),
            },
            EditorMutation::SetTypeshedSetting {
                key: TypeshedSettingKey::TypeshedCache,
                value: TypeshedSettingValue::Boolean { value: false },
            },
            EditorMutation::RemoveTypeshedSetting {
                key: TypeshedSettingKey::TypeshedUrl,
            },
        ],
        &catalog,
    );
    let Ok(update) = update else {
        panic!("valid mutations must build an update");
    };
    assert_eq!(
        update.rules.rules.get(&analyze),
        Some(&Some(RuleSeverity::Warning))
    );
    assert_eq!(update.rules.rules.get("BSK-0001"), Some(&None));
    assert_eq!(
        update.rules.rule_tags.get("basilisk"),
        Some(&Some(RuleSeverity::Error))
    );
    assert_eq!(update.rules.rule_tags.get("suppressions"), Some(&None));
    assert_eq!(
        update
            .typeshed
            .entries
            .get(&TypeshedConfigKey::TypeshedCache),
        Some(&Some(TypeshedConfigValue::Boolean(false)))
    );
    assert_eq!(
        update.typeshed.entries.get(&TypeshedConfigKey::TypeshedUrl),
        Some(&None)
    );
}

/// [CONFIGEDITOR-OPERATIONS]: unknown rule codes and tags are request errors.
#[test]
fn unknown_codes_and_tags_are_rejected() {
    let catalog = descriptors();
    let Err(rule_error) = build_update(
        &[EditorMutation::SetRule {
            code: "NOT-A-RULE".to_owned(),
            severity: WireSeverity::Warning,
        }],
        &catalog,
    ) else {
        panic!("unknown rule must fail");
    };
    assert_eq!(
        error_kind(&rule_error),
        Some(serde_json::json!("unknownRule"))
    );
    let Err(tag_error) = build_update(
        &[EditorMutation::RemoveTag {
            tag: "not-a-tag".to_owned(),
        }],
        &catalog,
    ) else {
        panic!("unknown tag must fail");
    };
    assert_eq!(
        error_kind(&tag_error),
        Some(serde_json::json!("unknownTag"))
    );
}

/// [CHKARCH-CONFIG-MODEL]: requesting `disabled` for a `pep`-tagged rule —
/// directly or via a tag entry that would resolve one to `disabled` — is a
/// request error. PEP rules are graded, never disabled.
#[test]
fn pep_disable_mutations_are_rejected() {
    let catalog = descriptors();
    let (pep, analyze) = partitioned_codes();

    // Direct SetRule(disabled) on a pep rule fails at mutation validation.
    let Err(direct) = build_update(
        &[EditorMutation::SetRule {
            code: pep.clone(),
            severity: WireSeverity::Disabled,
        }],
        &catalog,
    ) else {
        panic!("pep disable must fail");
    };
    assert_eq!(
        error_kind(&direct),
        Some(serde_json::json!("pepRuleDisable"))
    );

    // Disabling an analyze rule is legitimate configuration.
    assert!(build_update(
        &[EditorMutation::SetRule {
            code: analyze,
            severity: WireSeverity::Disabled,
        }],
        &catalog,
    )
    .is_ok());

    // A tag entry that resolves a pep rule to disabled fails the hypothetical
    // configuration check ([`pep_disable_violations`]).
    let pep_disabled_by_tag = {
        let document = document(BasiliskConfig::default());
        let update = build_update(
            &[EditorMutation::SetTag {
                tag: "pep".to_owned(),
                severity: WireSeverity::Disabled,
            }],
            &catalog,
        );
        let Ok(update) = update else {
            panic!("tag mutation itself is well-formed");
        };
        let Ok(patch) = build_configuration_patch(&document, &update) else {
            panic!("patch must render");
        };
        require_no_pep_disable(&patch.config)
    };
    let Err(error) = pep_disabled_by_tag else {
        panic!("pep tag entry at disabled must fail");
    };
    assert_eq!(
        error_kind(&error),
        Some(serde_json::json!("pepRuleDisable"))
    );
}

/// [LSPCFGED-TYPESHED]: Typeshed settings reject wrong scalar types,
/// malformed pins, and non-HTTPS/multi-placeholder mirrors.
#[test]
fn typeshed_setting_values_are_strictly_validated() {
    let catalog = descriptors();
    for mutation in [
        EditorMutation::SetTypeshedSetting {
            key: TypeshedSettingKey::TypeshedCommit,
            value: TypeshedSettingValue::Text {
                value: "short".to_owned(),
            },
        },
        EditorMutation::SetTypeshedSetting {
            key: TypeshedSettingKey::TypeshedUrl,
            value: TypeshedSettingValue::Text {
                value: "http://mirror.invalid/{sha}/{sha}.zip".to_owned(),
            },
        },
        EditorMutation::SetTypeshedSetting {
            key: TypeshedSettingKey::TypeshedVerify,
            value: TypeshedSettingValue::Text {
                value: "false".to_owned(),
            },
        },
    ] {
        let result = build_update(&[mutation], &catalog);
        assert!(result.is_err(), "invalid value must fail");
        let Some(error) = result.err() else { continue };
        assert_eq!(
            error_kind(&error),
            Some(serde_json::json!("invalidTypeshedSetting"))
        );
    }
}

/// [LSPCFGED-TYPESHED]: custom and exact sources cannot coexist, and custom
/// trees must contain the canonical top-level `stdlib/` directory.
#[test]
fn typeshed_source_combination_and_shape_are_validated() {
    let root = std::env::temp_dir().join(format!("bsk_typeshed_ui_{}", std::process::id()));
    let setup = std::fs::create_dir_all(&root);
    assert!(setup.is_ok(), "fixture root: {setup:?}");
    if setup.is_err() {
        return;
    }
    let mut config = BasiliskConfig {
        project_root: Some(root.clone()),
        typeshed_path: Some(PathBuf::from("custom")),
        typeshed_commit: Some("83c2518a9e6abbda0c44592c3483de459198f887".to_owned()),
        ..BasiliskConfig::default()
    };
    assert!(require_valid_typeshed_configuration(&config).is_err());
    config.typeshed_commit = None;
    assert!(require_valid_typeshed_configuration(&config).is_err());
    let stdlib = std::fs::create_dir_all(root.join("custom/stdlib"));
    assert!(stdlib.is_ok(), "stdlib fixture: {stdlib:?}");
    if stdlib.is_err() {
        let _ = std::fs::remove_dir_all(root);
        return;
    }
    assert!(require_valid_typeshed_configuration(&config).is_ok());
    let _ = std::fs::remove_dir_all(root);
}

/// [CONFIGEDITOR-MODEL]: a preview reports fully resolved effective-severity
/// changes and omits rules whose resolution did not move.
#[test]
fn resolved_changes_are_effective_and_omit_noops() {
    let catalog = descriptors();
    let (pep, analyze) = partitioned_codes();
    let before = BasiliskConfig::default();
    let after = BasiliskConfig::with_rule_entries(HashMap::from([
        (pep.clone(), RuleSeverity::Warning),
        (analyze.clone(), RuleSeverity::Warning),
    ]));

    let changes = resolved_changes(&catalog, &before, &after);
    let pep_change = changes.iter().find(|change| change.code == pep);
    let Some(pep_change) = pep_change else {
        panic!("graded pep rule must appear in the changes");
    };
    assert_eq!(pep_change.before, WireSeverity::Error);
    assert_eq!(pep_change.after, WireSeverity::Warning);
    let analyze_change = changes.iter().find(|change| change.code == analyze);
    let Some(analyze_change) = analyze_change else {
        panic!("enabled analyze rule must appear in the changes");
    };
    assert_eq!(analyze_change.before, WireSeverity::Disabled);
    assert_eq!(analyze_change.after, WireSeverity::Warning);
    // Everything else resolved identically on both sides and is omitted.
    assert_eq!(changes.len(), 2);
}

/// [LSPCFGED-TYPESHED]: previews describe the exact persisted before/after
/// value for every allowlisted setting and omit unchanged settings.
#[test]
fn resolved_typeshed_changes_cover_the_closed_setting_allowlist() {
    let before = BasiliskConfig::default();
    let after = BasiliskConfig {
        typeshed_path: Some(PathBuf::from("custom-typeshed")),
        typeshed_commit: Some("83c2518a9e6abbda0c44592c3483de459198f887".to_owned()),
        typeshed_url: Some("https://mirror.example/{sha}.zip".to_owned()),
        typeshed_cache_path: Some(PathBuf::from(".typeshed-cache")),
        typeshed_cache: Some(false),
        typeshed_verify: Some(false),
        ..BasiliskConfig::default()
    };

    let changes = resolved_typeshed_changes(&before, &after);
    let keys = changes.iter().map(|change| change.key).collect::<Vec<_>>();
    assert_eq!(
        keys,
        vec![
            TypeshedSettingKey::TypeshedPath,
            TypeshedSettingKey::TypeshedCommit,
            TypeshedSettingKey::TypeshedUrl,
            TypeshedSettingKey::TypeshedCachePath,
            TypeshedSettingKey::TypeshedCache,
            TypeshedSettingKey::TypeshedVerify,
        ]
    );
    assert!(changes.iter().all(|change| change.before.is_none()));
    assert!(resolved_typeshed_changes(&after, &after).is_empty());
}

#[test]
fn unknown_rule_in_active_config_is_rejected_at_protocol_boundary() {
    let config = BasiliskConfig::with_rule_entries(HashMap::from([(
        "NOT-A-RULE".to_owned(),
        RuleSeverity::Warning,
    )]));
    let document = document(config);
    let Err(error) = validate_document_rules(&document) else {
        panic!("unknown configured rule must fail");
    };
    assert_eq!(error_kind(&error), Some(serde_json::json!("unknownRule")));
    assert_eq!(
        error
            .data
            .as_ref()
            .and_then(|data| data.pointer("/context/sourceUri")),
        Some(&serde_json::json!("file:///workspace/pyproject.toml"))
    );
}

// Implements [CONFIGEDITOR-MODEL]: impact is a complete errors/warnings/infos
// before/after partition, straight from both inventories.
#[test]
fn impact_partitions_diagnostics_by_emitting_severity() {
    let before = inventory_fixture(5, 2, 1);
    let after = inventory_fixture(3, 1, 0);

    let impact = build_impact(&before, &after);

    assert_eq!(impact.errors_before, 5);
    assert_eq!(impact.errors_after, 3);
    assert_eq!(impact.warnings_before, 2);
    assert_eq!(impact.warnings_after, 1);
    assert_eq!(impact.infos_before, 1);
    assert_eq!(impact.infos_after, 0);
}

#[test]
fn revision_gate_reports_both_revisions_on_conflict() {
    let document = document(BasiliskConfig::default());
    assert!(require_revision(&document, "revision").is_ok());
    let Err(error) = require_revision(&document, "stale") else {
        panic!("stale revision must conflict");
    };
    assert_eq!(
        error_kind(&error),
        Some(serde_json::json!("revisionConflict"))
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
    assert_eq!(error_kind(&rule), Some(serde_json::json!("unknownRule")));
    assert_eq!(
        rule.data
            .as_ref()
            .and_then(|data| data.pointer("/context/rule")),
        Some(&serde_json::json!("NOT-A-RULE"))
    );
    let tag = selection_error(SelectionError::UnknownTag("not-a-tag".to_owned()));
    assert_eq!(error_kind(&tag), Some(serde_json::json!("unknownTag")));
    assert_eq!(
        tag.data
            .as_ref()
            .and_then(|data| data.pointer("/context/tag")),
        Some(&serde_json::json!("not-a-tag"))
    );
}

fn inventory_fixture(errors: usize, warnings: usize, infos: usize) -> Inventory {
    Inventory {
        counts: HashMap::new(),
        errors,
        warnings,
        infos,
    }
}

fn document(config: BasiliskConfig) -> ConfigDocument {
    ConfigDocument {
        root: PathBuf::from("/workspace"),
        path: PathBuf::from("/workspace/pyproject.toml"),
        exists: true,
        read_only: false,
        content: "[tool.basilisk]\n".to_owned(),
        revision: "revision".to_owned(),
        config,
    }
}
