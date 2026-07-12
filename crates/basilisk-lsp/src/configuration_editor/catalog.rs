//! Live checker-catalog adapter and selector expansion.

use std::collections::{BTreeSet, HashMap, HashSet};

use basilisk_config::{BasiliskConfig, RuleSeverity as ConfigSeverity};

use super::model::{RuleDescriptor, RuleSelector, RuleSetting, RuleSeverity, TagKind};

/// Map the checker's diagnostic severity to the four configuration severities.
pub(super) const fn wire_severity(value: basilisk_checker::Severity) -> RuleSeverity {
    match value {
        basilisk_checker::Severity::Info => RuleSeverity::Info,
        basilisk_checker::Severity::Warning => RuleSeverity::Warning,
        basilisk_checker::Severity::Error | basilisk_checker::Severity::SafetyViolation => {
            RuleSeverity::Error
        }
    }
}

/// Map a persisted config severity to its wire representation.
pub(super) const fn config_to_wire(value: ConfigSeverity) -> RuleSeverity {
    match value {
        ConfigSeverity::Error => RuleSeverity::Error,
        ConfigSeverity::Warning => RuleSeverity::Warning,
        ConfigSeverity::Info => RuleSeverity::Info,
        ConfigSeverity::Disabled => RuleSeverity::Disabled,
    }
}

/// Expand the live checker registry into the generated wire descriptor shape.
pub(super) fn descriptors() -> Vec<RuleDescriptor> {
    basilisk_checker::rule_catalog()
        .into_iter()
        .map(|rule| RuleDescriptor {
            code: rule.code.to_owned(),
            title: rule.title.to_owned(),
            summary: rule.summary.to_owned(),
            docs_url: rule.docs_url.to_owned(),
            tags: rule.tags.into_iter().map(str::to_owned).collect(),
            default_severity: wire_severity(rule.default_severity),
            default_enabled: rule.default_enabled,
        })
        .collect()
}

/// Current configured and effective severity for one catalog rule.
pub(super) fn severities(
    descriptor: &RuleDescriptor,
    config: &BasiliskConfig,
) -> (Option<RuleSeverity>, RuleSeverity) {
    let configured = config
        .rules
        .get(&descriptor.code)
        .copied()
        .map(config_to_wire);
    let effective = match (configured, descriptor.default_enabled) {
        (Some(severity), _) => severity,
        (None, true) => descriptor.default_severity,
        (None, false) => RuleSeverity::Disabled,
    };
    (configured, effective)
}

/// Classify a canonical catalog tag for the tag dashboard.
pub(super) fn tag_kind(tag: &str) -> TagKind {
    if basilisk_checker::rule_tags::is_provenance(tag) {
        TagKind::Provenance
    } else if basilisk_checker::rule_tags::is_pep_category(tag) {
        TagKind::PepCategory
    } else {
        TagKind::Descriptive
    }
}

/// Expand a selector against the live catalog and current occurrence counts.
pub(super) fn expand_selector(
    selector: &RuleSelector,
    catalog: &[RuleDescriptor],
    counts: &HashMap<String, usize>,
) -> Result<Vec<String>, SelectionError> {
    let catalog_codes: HashSet<&str> = catalog.iter().map(|rule| rule.code.as_str()).collect();
    let catalog_tags: HashSet<&str> = catalog
        .iter()
        .flat_map(|rule| rule.tags.iter().map(String::as_str))
        .collect();
    let selected: BTreeSet<&str> = match selector {
        RuleSelector::All => catalog_codes.into_iter().collect(),
        RuleSelector::Codes { codes } => {
            if let Some(unknown) = codes
                .iter()
                .find(|code| !catalog_codes.contains(code.as_str()))
            {
                return Err(SelectionError::UnknownRule(unknown.clone()));
            }
            codes.iter().map(String::as_str).collect()
        }
        RuleSelector::Tags { tags, match_all } => {
            if let Some(unknown) = tags.iter().find(|tag| !catalog_tags.contains(tag.as_str())) {
                return Err(SelectionError::UnknownTag(unknown.clone()));
            }
            catalog
                .iter()
                .filter(|rule| {
                    let matches = tags.iter().filter(|tag| rule.tags.contains(tag)).count();
                    if *match_all {
                        matches == tags.len()
                    } else {
                        matches > 0
                    }
                })
                .map(|rule| rule.code.as_str())
                .collect()
        }
        RuleSelector::CurrentViolations => counts
            .iter()
            .filter(|(_, count)| **count > 0)
            .map(|(code, _)| code.as_str())
            .collect(),
        RuleSelector::SafeFixable => counts
            .iter()
            .filter(|(code, count)| **count > 0 && is_safe_fixable(code))
            .map(|(code, _)| code.as_str())
            .collect(),
        RuleSelector::WithoutSafeFix => counts
            .iter()
            .filter(|(code, count)| **count > 0 && !is_safe_fixable(code))
            .map(|(code, _)| code.as_str())
            .collect(),
    };
    Ok(catalog
        .iter()
        .filter(|rule| selected.contains(rule.code.as_str()))
        .map(|rule| rule.code.clone())
        .collect())
}

/// Resolve an intent to a concrete persisted severity for one rule.
pub(super) fn setting_severity(
    setting: RuleSetting,
    descriptor: &RuleDescriptor,
) -> Option<ConfigSeverity> {
    match setting {
        RuleSetting::Inherit => None,
        RuleSetting::Native => Some(wire_to_config(descriptor.default_severity)),
        RuleSetting::Error => Some(ConfigSeverity::Error),
        RuleSetting::Warning => Some(ConfigSeverity::Warning),
        RuleSetting::Info => Some(ConfigSeverity::Info),
        RuleSetting::Disabled => Some(ConfigSeverity::Disabled),
    }
}

const fn wire_to_config(value: RuleSeverity) -> ConfigSeverity {
    match value {
        RuleSeverity::Error => ConfigSeverity::Error,
        RuleSeverity::Warning => ConfigSeverity::Warning,
        RuleSeverity::Info => ConfigSeverity::Info,
        RuleSeverity::Disabled => ConfigSeverity::Disabled,
    }
}

/// Whether a rule has a safe deterministic fixer.
pub(super) fn is_safe_fixable(code: &str) -> bool {
    crate::code_actions::mass_fix::SAFE_FIXABLE_RULES.contains(&code)
}

/// Whether a rule has any deterministic fixer.
pub(super) fn is_fixable(code: &str) -> bool {
    crate::code_actions::mass_fix::ALL_FIXABLE_RULES.contains(&code)
}

/// Selector validation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SelectionError {
    /// Code absent from the live registry.
    UnknownRule(String),
    /// Tag absent from the live registry.
    UnknownTag(String),
}
