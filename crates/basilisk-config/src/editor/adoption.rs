//! Adoption metadata projected from the active configuration document.

use std::collections::BTreeMap;

use super::{ConfigDocument, ConfigFormat};
use crate::RuleSeverity;

/// Return editor-generated exact-path adoption rule overrides.
///
/// Only entries explicitly marked `adoption = true` are included. This keeps
/// ordinary path-scoped configuration distinct from temporary migration debt.
#[must_use]
pub fn adoption_rule_overrides(
    document: &ConfigDocument,
) -> BTreeMap<String, BTreeMap<String, RuleSeverity>> {
    let ConfigFormat::PyprojectToml = document.format;
    toml_adoptions(&document.content)
}

fn toml_adoptions(content: &str) -> BTreeMap<String, BTreeMap<String, RuleSeverity>> {
    let Some(paths) = content
        .parse::<toml::Table>()
        .ok()
        .and_then(|root| root.get("tool").cloned())
        .and_then(|tool| tool.get("basilisk").cloned())
        .and_then(|basilisk| basilisk.get("per-path-overrides").cloned())
        .and_then(|paths| paths.as_table().cloned())
    else {
        return BTreeMap::new();
    };
    paths
        .into_iter()
        .filter_map(|(pattern, value)| {
            let entry = value.as_table()?;
            entry.get("adoption")?.as_bool()?.then(|| {
                (
                    pattern,
                    toml_rules(entry.get("rules").and_then(toml::Value::as_table)),
                )
            })
        })
        .filter(|(_, rules)| !rules.is_empty())
        .collect()
}

fn toml_rules(rules: Option<&toml::Table>) -> BTreeMap<String, RuleSeverity> {
    rules
        .into_iter()
        .flatten()
        .filter_map(|(code, value)| {
            value
                .as_str()
                .and_then(RuleSeverity::parse)
                .map(|severity| (code.clone(), severity))
        })
        .collect()
}
