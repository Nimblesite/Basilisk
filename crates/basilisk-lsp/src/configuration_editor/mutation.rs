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
#[path = "mutation_tests.rs"]
mod tests;
