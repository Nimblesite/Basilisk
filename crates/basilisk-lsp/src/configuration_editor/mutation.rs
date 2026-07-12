//! Mutation validation, selector expansion, and impact projection.

use std::collections::{BTreeMap, HashMap, HashSet};

use basilisk_config::{
    ConfigDocument, ConfigPatch, RuleConfigScope, RuleConfigUpdate, RuleSeverity as ConfigSeverity,
};
use tower_lsp::jsonrpc::{Error, Result as LspResult};

use super::catalog::{descriptors, expand_selector, setting_severity, severities, SelectionError};
use super::model::{
    ConfigurationImpact, MutationScope, PreviewConfigurationRequest, ResolvedConfigurationChange,
    RuleSelector, RuleSetting,
};
use super::protocol::{path_uri, rpc_error, rpc_error_data};

pub(super) fn validate_selector(selector: &RuleSelector) -> LspResult<()> {
    match selector {
        RuleSelector::Codes { codes } if codes.is_empty() => Err(rpc_error(
            "invalidMutation",
            "a code selector requires at least one rule code",
        )),
        RuleSelector::Tags { tags, .. } if tags.is_empty() => Err(rpc_error(
            "invalidMutation",
            "a tag selector requires at least one tag",
        )),
        _ => Ok(()),
    }
}

pub(super) fn validate_document_rules(document: &ConfigDocument) -> LspResult<()> {
    let catalog = descriptors();
    let known: HashSet<&str> = catalog.iter().map(|rule| rule.code.as_str()).collect();
    let unknown = document
        .config
        .rules
        .keys()
        .chain(
            document
                .config
                .per_path_overrides
                .values()
                .flat_map(|entry| entry.rule_overrides.keys()),
        )
        .chain(
            document
                .config
                .per_path_overrides
                .values()
                .flat_map(|entry| entry.disabled_rules.iter()),
        )
        .find(|code| !known.contains(code.as_str()));
    if let Some(code) = unknown {
        Err(rpc_error_data(
            "unknownRule",
            "active configuration contains an unknown rule",
            serde_json::json!({ "rule": code, "sourceUri": path_uri(&document.path) }),
        ))
    } else {
        Ok(())
    }
}

pub(super) fn expand_mutations(
    request: &PreviewConfigurationRequest,
    catalog: &[super::model::RuleDescriptor],
    counts: &HashMap<String, usize>,
) -> LspResult<(Vec<RuleConfigUpdate>, Vec<String>)> {
    let by_code: HashMap<&str, &super::model::RuleDescriptor> = catalog
        .iter()
        .map(|descriptor| (descriptor.code.as_str(), descriptor))
        .collect();
    let mut updates: Vec<RuleConfigUpdate> = Vec::new();
    let mut expanded = HashSet::new();
    for mutation in &request.mutations {
        let codes =
            expand_selector(&mutation.selector, catalog, counts).map_err(selection_error)?;
        let scope = mutation_scope(&mutation.scope)?;
        if !updates.iter().any(|update| update.scope == scope) {
            updates.push(RuleConfigUpdate {
                scope: scope.clone(),
                rules: BTreeMap::new(),
            });
        }
        let target = updates
            .iter_mut()
            .find(|update| update.scope == scope)
            .ok_or_else(|| rpc_error("invalidMutation", "failed to group mutation scope"))?;
        for code in codes {
            let descriptor = by_code.get(code.as_str()).ok_or_else(|| {
                rpc_error_data(
                    "unknownRule",
                    "rule disappeared during selector expansion",
                    serde_json::json!({ "rule": code }),
                )
            })?;
            let _ = target
                .rules
                .insert(code.clone(), setting_severity(mutation.setting, descriptor));
            let _ = expanded.insert(code);
        }
    }
    let expanded_rule_codes = catalog
        .iter()
        .filter(|rule| expanded.contains(&rule.code))
        .map(|rule| rule.code.clone())
        .collect();
    Ok((updates, expanded_rule_codes))
}

fn mutation_scope(scope: &MutationScope) -> LspResult<RuleConfigScope> {
    match scope {
        MutationScope::Project => Ok(RuleConfigScope::Project),
        MutationScope::Path { pattern } => {
            validate_path_pattern(pattern)?;
            Ok(RuleConfigScope::Path {
                pattern: pattern.clone(),
                adoption: false,
            })
        }
    }
}

fn validate_path_pattern(pattern: &str) -> LspResult<()> {
    let trimmed = pattern.trim();
    if trimmed.is_empty() || matches!(trimmed, "." | "./") {
        return Err(invalid_path_pattern(
            "path mutation pattern must select a project-relative path",
        ));
    }
    if trimmed != pattern {
        return Err(invalid_path_pattern(
            "path mutation pattern must not have leading or trailing whitespace",
        ));
    }
    let bytes = pattern.as_bytes();
    let windows_absolute =
        bytes.get(1) == Some(&b':') && bytes.first().is_some_and(u8::is_ascii_alphabetic);
    if pattern.starts_with('/') || pattern.starts_with('\\') || windows_absolute {
        return Err(invalid_path_pattern(
            "path mutation pattern must not be absolute",
        ));
    }
    if pattern.contains('\\') {
        return Err(invalid_path_pattern(
            "path mutation pattern must use forward-slash separators",
        ));
    }
    if pattern.chars().any(char::is_control) {
        return Err(invalid_path_pattern(
            "path mutation pattern must not contain control characters",
        ));
    }
    if pattern
        .split('/')
        .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(invalid_path_pattern(
            "path mutation pattern contains an empty, dot, or parent component",
        ));
    }
    Ok(())
}

fn invalid_path_pattern(message: &str) -> Error {
    rpc_error("invalidMutation", message)
}

pub(super) fn build_impact(
    patch: &ConfigPatch,
    catalog: &[super::model::RuleDescriptor],
    changes: &[ResolvedConfigurationChange],
    before: &super::snapshot::Inventory,
    after: &super::snapshot::Inventory,
) -> ConfigurationImpact {
    let changed_rules = changes
        .iter()
        .map(|change| change.rule_code.as_str())
        .collect::<HashSet<_>>()
        .len();
    let enabled_rules = catalog
        .iter()
        .filter(|rule| severities(rule, &patch.config).1 != super::model::RuleSeverity::Disabled)
        .count();
    let disabled_rules = catalog.len().saturating_sub(enabled_rules);
    ConfigurationImpact {
        changed_rules: count_i64(changed_rules),
        enabled_rules: count_i64(enabled_rules),
        disabled_rules: count_i64(disabled_rules),
        diagnostics_before: count_i64(before.total),
        diagnostics_after: count_i64(after.total),
        errors_before: count_i64(before.errors),
        errors_after: count_i64(after.errors),
        warnings_before: count_i64(before.warnings),
        warnings_after: count_i64(after.warnings),
    }
}

/// Project the concrete config entries that a preview would actually change.
pub(super) fn resolved_changes(
    document: &ConfigDocument,
    updates: &[RuleConfigUpdate],
) -> Vec<ResolvedConfigurationChange> {
    updates
        .iter()
        .flat_map(|update| {
            update.rules.iter().filter_map(|(code, severity)| {
                let before = configured_in_scope(document, &update.scope, code);
                (before != *severity).then(|| ResolvedConfigurationChange {
                    rule_code: code.clone(),
                    scope: wire_scope(&update.scope),
                    previous_setting: wire_setting(before),
                    resulting_setting: wire_setting(*severity),
                })
            })
        })
        .collect()
}

fn configured_in_scope(
    document: &ConfigDocument,
    scope: &RuleConfigScope,
    code: &str,
) -> Option<ConfigSeverity> {
    match scope {
        RuleConfigScope::Project => document.config.rules.get(code).copied(),
        RuleConfigScope::Path { pattern, .. } => document
            .config
            .per_path_overrides
            .get(pattern)
            .and_then(|entry| {
                entry.rule_overrides.get(code).copied().or_else(|| {
                    entry
                        .disabled_rules
                        .iter()
                        .any(|disabled| disabled == code)
                        .then_some(ConfigSeverity::Disabled)
                })
            }),
    }
}

fn wire_scope(scope: &RuleConfigScope) -> MutationScope {
    match scope {
        RuleConfigScope::Project => MutationScope::Project,
        RuleConfigScope::Path { pattern, .. } => MutationScope::Path {
            pattern: pattern.clone(),
        },
    }
}

const fn wire_setting(severity: Option<ConfigSeverity>) -> RuleSetting {
    match severity {
        None => RuleSetting::Inherit,
        Some(ConfigSeverity::Error) => RuleSetting::Error,
        Some(ConfigSeverity::Warning) => RuleSetting::Warning,
        Some(ConfigSeverity::Info) => RuleSetting::Info,
        Some(ConfigSeverity::Disabled) => RuleSetting::Disabled,
    }
}

pub(super) fn require_revision(document: &ConfigDocument, expected: &str) -> LspResult<()> {
    if document.revision == expected {
        Ok(())
    } else {
        Err(rpc_error_data(
            "revisionConflict",
            "configuration changed; refresh and preview again",
            serde_json::json!({ "expected": expected, "actual": document.revision }),
        ))
    }
}

pub(super) fn selection_error(error: SelectionError) -> Error {
    match error {
        SelectionError::UnknownRule(rule) => rpc_error_data(
            "unknownRule",
            "selector contains an unknown rule",
            serde_json::json!({ "rule": rule }),
        ),
        SelectionError::UnknownTag(tag) => rpc_error_data(
            "unknownTag",
            "selector contains an unknown tag",
            serde_json::json!({ "tag": tag }),
        ),
    }
}

fn count_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
#[expect(
    clippy::panic,
    reason = "test-only destructuring failures should abort with a focused message"
)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use basilisk_config::{
        BasiliskConfig, ConfigDocument, ConfigFormat, PathOverride, RuleConfigScope,
        RuleConfigUpdate, RuleSeverity,
    };

    use super::{
        expand_mutations, resolved_changes, validate_document_rules, validate_path_pattern,
        validate_selector,
    };
    use crate::configuration_editor::catalog::descriptors;
    use crate::configuration_editor::model::{
        ConfigurationMutation, MutationScope, PreviewConfigurationRequest, RuleSelector,
        RuleSetting,
    };

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
}
