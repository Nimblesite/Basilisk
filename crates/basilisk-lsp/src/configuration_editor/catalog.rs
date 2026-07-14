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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use basilisk_config::RuleSeverity as ConfigSeverity;

    use super::{
        config_to_wire, descriptors, expand_selector, is_fixable, is_safe_fixable,
        setting_severity, severities, tag_kind, wire_severity, SelectionError,
    };
    use crate::configuration_editor::model::{RuleSelector, RuleSetting, RuleSeverity, TagKind};

    // Implements [CONFIGEDITOR-MODEL]: the checker's severity ladder folds onto
    // the four wire severities without losing the error class.
    #[test]
    fn severity_conversions_round_trip_the_wire_shape() {
        assert_eq!(
            wire_severity(basilisk_checker::Severity::Error),
            RuleSeverity::Error
        );
        assert_eq!(
            wire_severity(basilisk_checker::Severity::SafetyViolation),
            RuleSeverity::Error
        );
        assert_eq!(
            wire_severity(basilisk_checker::Severity::Warning),
            RuleSeverity::Warning
        );
        assert_eq!(
            wire_severity(basilisk_checker::Severity::Info),
            RuleSeverity::Info
        );
        for (config, wire) in [
            (ConfigSeverity::Error, RuleSeverity::Error),
            (ConfigSeverity::Warning, RuleSeverity::Warning),
            (ConfigSeverity::Info, RuleSeverity::Info),
            (ConfigSeverity::Disabled, RuleSeverity::Disabled),
        ] {
            assert_eq!(config_to_wire(config), wire);
        }
    }

    // Implements [CONFIGEDITOR-TAGS]: descriptors mirror the live checker
    // registry, complete with docs links and provenance tags.
    #[test]
    fn descriptors_expose_the_live_checker_registry() {
        let catalog = descriptors();
        assert_eq!(catalog.len(), basilisk_checker::rule_catalog().len());
        let annotation = catalog.iter().find(|rule| rule.code == "BSK-E0001");
        let Some(annotation) = annotation else {
            unreachable!("BSK-E0001 must exist in the live registry");
        };
        assert!(annotation.docs_url.contains("BSK-E0001"));
        assert!(!annotation.title.is_empty());
        assert!(!annotation.tags.is_empty());
    }

    #[test]
    fn severities_prefer_configured_then_default_then_disabled() {
        let catalog = descriptors();
        let configured_off = catalog.iter().find(|rule| rule.code == "BSK-E0001");
        let Some(descriptor) = configured_off else {
            unreachable!("BSK-E0001 must exist in the live registry");
        };
        let mut config = basilisk_config::BasiliskConfig::default();
        let _ = config
            .rules
            .insert("BSK-E0001".to_owned(), ConfigSeverity::Info);
        assert_eq!(
            severities(descriptor, &config),
            (Some(RuleSeverity::Info), RuleSeverity::Info)
        );
        let unconfigured = basilisk_config::BasiliskConfig::default();
        let expected = if descriptor.default_enabled {
            descriptor.default_severity
        } else {
            RuleSeverity::Disabled
        };
        assert_eq!(severities(descriptor, &unconfigured), (None, expected));
        let default_on = descriptors()
            .into_iter()
            .find(|rule| rule.default_enabled)
            .map(|rule| severities(&rule, &unconfigured));
        assert!(default_on.is_some_and(|(configured, effective)| {
            configured.is_none() && effective != RuleSeverity::Disabled
        }));
    }

    #[test]
    fn tag_kinds_classify_provenance_pep_and_descriptive_names() {
        assert_eq!(
            tag_kind(basilisk_checker::rule_tags::PEP),
            TagKind::Provenance
        );
        assert_eq!(
            tag_kind(basilisk_checker::rule_tags::BASILISK),
            TagKind::Provenance
        );
        let pep_category = basilisk_checker::rule_tags::PEP_CATEGORIES.first();
        assert_eq!(
            pep_category.map(|tag| tag_kind(tag)),
            Some(TagKind::PepCategory)
        );
        assert_eq!(tag_kind("suppressions"), TagKind::Descriptive);
    }

    #[test]
    fn selector_expansion_covers_every_selector_kind() {
        let catalog = descriptors();
        let safe_code = "BSK-E0001";
        assert!(is_safe_fixable(safe_code));
        let unsafe_only = "assignment_compatibility";
        assert!(!is_safe_fixable(unsafe_only));
        let counts: HashMap<String, usize> = HashMap::from([
            (safe_code.to_owned(), 3),
            (unsafe_only.to_owned(), 1),
            ("silent_rule".to_owned(), 0),
        ]);

        let all = expand_selector(&RuleSelector::All, &catalog, &counts);
        assert_eq!(all.map(|codes| codes.len()), Ok(catalog.len()));

        let codes = expand_selector(
            &RuleSelector::Codes {
                codes: vec![safe_code.to_owned()],
            },
            &catalog,
            &counts,
        );
        assert_eq!(codes, Ok(vec![safe_code.to_owned()]));
        assert_eq!(
            expand_selector(
                &RuleSelector::Codes {
                    codes: vec!["NOT-A-RULE".to_owned()],
                },
                &catalog,
                &counts,
            ),
            Err(SelectionError::UnknownRule("NOT-A-RULE".to_owned()))
        );

        let tagged = expand_selector(
            &RuleSelector::Tags {
                tags: vec!["suppressions".to_owned()],
                match_all: false,
            },
            &catalog,
            &counts,
        );
        assert!(tagged.is_ok_and(|codes| !codes.is_empty()));
        let impossible_conjunction = expand_selector(
            &RuleSelector::Tags {
                tags: vec![
                    basilisk_checker::rule_tags::PEP.to_owned(),
                    basilisk_checker::rule_tags::BASILISK.to_owned(),
                ],
                match_all: true,
            },
            &catalog,
            &counts,
        );
        assert_eq!(impossible_conjunction, Ok(Vec::new()));
        assert_eq!(
            expand_selector(
                &RuleSelector::Tags {
                    tags: vec!["not-a-tag".to_owned()],
                    match_all: false,
                },
                &catalog,
                &counts,
            ),
            Err(SelectionError::UnknownTag("not-a-tag".to_owned()))
        );

        let sorted = |result: Result<Vec<String>, SelectionError>| {
            result.map(|mut codes| {
                codes.sort();
                codes
            })
        };
        let current = sorted(expand_selector(
            &RuleSelector::CurrentViolations,
            &catalog,
            &counts,
        ));
        assert_eq!(
            current,
            Ok(vec![safe_code.to_owned(), unsafe_only.to_owned()])
        );
        let safe = expand_selector(&RuleSelector::SafeFixable, &catalog, &counts);
        assert_eq!(safe, Ok(vec![safe_code.to_owned()]));
        let without_safe = expand_selector(&RuleSelector::WithoutSafeFix, &catalog, &counts);
        assert_eq!(without_safe, Ok(vec![unsafe_only.to_owned()]));
    }

    #[test]
    fn setting_severity_resolves_every_intent() {
        let catalog = descriptors();
        let Some(descriptor) = catalog.first() else {
            unreachable!("the live registry is never empty");
        };
        assert_eq!(setting_severity(RuleSetting::Inherit, descriptor), None);
        assert_eq!(
            setting_severity(RuleSetting::Native, descriptor),
            Some(match descriptor.default_severity {
                RuleSeverity::Error => ConfigSeverity::Error,
                RuleSeverity::Warning => ConfigSeverity::Warning,
                RuleSeverity::Info => ConfigSeverity::Info,
                RuleSeverity::Disabled => ConfigSeverity::Disabled,
            })
        );
        assert_eq!(
            setting_severity(RuleSetting::Error, descriptor),
            Some(ConfigSeverity::Error)
        );
        assert_eq!(
            setting_severity(RuleSetting::Warning, descriptor),
            Some(ConfigSeverity::Warning)
        );
        assert_eq!(
            setting_severity(RuleSetting::Info, descriptor),
            Some(ConfigSeverity::Info)
        );
        assert_eq!(
            setting_severity(RuleSetting::Disabled, descriptor),
            Some(ConfigSeverity::Disabled)
        );
    }

    // Implements [AUTOFIX-CLASSIFY]: the safe tier is a strict subset of the
    // fixable tier, so occurrence fix badges can never contradict mass-fix.
    #[test]
    fn safe_fixable_rules_are_a_subset_of_fixable_rules() {
        for code in crate::code_actions::mass_fix::SAFE_FIXABLE_RULES {
            assert!(is_fixable(code), "{code} is safe-fixable but not fixable");
        }
        assert!(!is_safe_fixable("unused_suppression"));
    }
}
