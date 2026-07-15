//! Mutation validation and effective-severity change projection.
//!
//! Implements [CONFIGEDITOR-OPERATIONS]: a mutation is `SetRule`,
//! `RemoveRule`, `SetTag`, or `RemoveTag` — nothing else. Requesting
//! `disabled` for a `pep`-tagged rule (directly, or via a tag entry that
//! would resolve one to `disabled`) is a request error
//! ([CHKARCH-CONFIG-MODEL]).

use std::collections::HashSet;

use basilisk_config::{BasiliskConfig, ConfigDocument, RuleConfigUpdate};
use tower_lsp::jsonrpc::{Error, Result as LspResult};

use super::catalog::{descriptors, effective_severity, wire_to_config, SelectionError};
use super::model::{
    ConfigurationImpact, EditorMutation, ResolvedRuleChange, RuleDescriptor, RuleSeverity,
};
use super::protocol::{path_uri, rpc_error, rpc_error_data};
use super::snapshot::{count_i64, Inventory};

/// Reject active configuration whose rule entries name unknown rules.
pub(super) fn validate_document_rules(document: &ConfigDocument) -> LspResult<()> {
    let catalog = descriptors();
    let known: HashSet<&str> = catalog.iter().map(|rule| rule.code.as_str()).collect();
    let unknown = document
        .config
        .nearest_tables()
        .into_iter()
        .flat_map(|tables| tables.rules.keys())
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

/// Fold the requested mutations into one validated entry update.
///
/// Implements [CONFIGEDITOR-OPERATIONS] and `EditorMutation` in
/// `models/configuration_editor.td`: unknown codes and tags are request
/// errors, and an explicit `SetRule(disabled)` on a `pep` rule fails before
/// any patch is rendered.
pub(super) fn build_update(
    mutations: &[EditorMutation],
    catalog: &[RuleDescriptor],
) -> LspResult<RuleConfigUpdate> {
    let known_codes: HashSet<&str> = catalog.iter().map(|rule| rule.code.as_str()).collect();
    let known_tags: HashSet<&str> = catalog
        .iter()
        .flat_map(|rule| rule.tags.iter().map(String::as_str))
        .collect();
    let mut update = RuleConfigUpdate::default();
    for mutation in mutations {
        match mutation {
            EditorMutation::SetRule { code, severity } => {
                require_known_rule(&known_codes, code)?;
                if *severity == RuleSeverity::Disabled && basilisk_checker::is_pep_rule(code) {
                    return Err(pep_disable_error(std::slice::from_ref(code)));
                }
                let _ = update
                    .rules
                    .insert(code.clone(), Some(wire_to_config(*severity)));
            }
            EditorMutation::RemoveRule { code } => {
                require_known_rule(&known_codes, code)?;
                let _ = update.rules.insert(code.clone(), None);
            }
            EditorMutation::SetTag { tag, severity } => {
                require_known_tag(&known_tags, tag)?;
                let _ = update
                    .rule_tags
                    .insert(tag.clone(), Some(wire_to_config(*severity)));
            }
            EditorMutation::RemoveTag { tag } => {
                require_known_tag(&known_tags, tag)?;
                let _ = update.rule_tags.insert(tag.clone(), None);
            }
        }
    }
    Ok(update)
}

/// Reject a hypothetical configuration that resolves any `pep` rule to
/// `disabled` ([CHKARCH-CONFIG-MODEL]) — by rule entry or tag entry.
pub(super) fn require_no_pep_disable(config: &BasiliskConfig) -> LspResult<()> {
    let violations = basilisk_checker::pep_disable_violations(config);
    if violations.is_empty() {
        Ok(())
    } else {
        Err(pep_disable_error(&violations))
    }
}

fn pep_disable_error<S: AsRef<str>>(codes: &[S]) -> Error {
    rpc_error_data(
        "pepRuleDisable",
        "pep rules are graded, never disabled",
        serde_json::json!({
            "rules": codes.iter().map(AsRef::as_ref).collect::<Vec<_>>(),
        }),
    )
}

fn require_known_rule(known: &HashSet<&str>, code: &str) -> LspResult<()> {
    if known.contains(code) {
        Ok(())
    } else {
        Err(selection_error(SelectionError::UnknownRule(
            code.to_owned(),
        )))
    }
}

fn require_known_tag(known: &HashSet<&str>, tag: &str) -> LspResult<()> {
    if known.contains(tag) {
        Ok(())
    } else {
        Err(selection_error(SelectionError::UnknownTag(tag.to_owned())))
    }
}

/// Project the fully resolved per-rule effective-severity changes.
///
/// Implements [CONFIGEDITOR-MODEL]: a preview reports what actually changes
/// after resolution — rules whose effective severity is identical on both
/// sides are omitted.
pub(super) fn resolved_changes(
    catalog: &[RuleDescriptor],
    before: &BasiliskConfig,
    after: &BasiliskConfig,
) -> Vec<ResolvedRuleChange> {
    catalog
        .iter()
        .filter_map(|descriptor| {
            let previous = effective_severity(descriptor, before);
            let next = effective_severity(descriptor, after);
            (previous != next).then(|| ResolvedRuleChange {
                code: descriptor.code.clone(),
                before: previous,
                after: next,
            })
        })
        .collect()
}

/// Fold both inventories into the complete before/after impact partition.
pub(super) fn build_impact(before: &Inventory, after: &Inventory) -> ConfigurationImpact {
    ConfigurationImpact {
        errors_before: count_i64(before.errors),
        errors_after: count_i64(after.errors),
        warnings_before: count_i64(before.warnings),
        warnings_after: count_i64(after.warnings),
        infos_before: count_i64(before.infos),
        infos_after: count_i64(after.infos),
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
            "request names an unknown rule",
            serde_json::json!({ "rule": rule }),
        ),
        SelectionError::UnknownTag(tag) => rpc_error_data(
            "unknownTag",
            "request names an unknown tag",
            serde_json::json!({ "tag": tag }),
        ),
    }
}

/// Reject an empty mutation list before any state is touched.
pub(super) fn require_mutations(mutations: &[EditorMutation]) -> LspResult<()> {
    if mutations.is_empty() {
        Err(rpc_error(
            "invalidMutation",
            "configuration preview requires at least one mutation",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "mutation_tests.rs"]
mod tests;
