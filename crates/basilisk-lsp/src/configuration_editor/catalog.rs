//! Live checker-catalog adapter, effective-severity resolution, and selector
//! expansion.
//!
//! Implements [CONFIGEDITOR-TAGS]: the checker registry is the one rule
//! catalog; severity comes only from configuration entries resolved over the
//! model in [CHKARCH-CONFIG-MODEL].

use std::collections::{BTreeSet, HashSet};

use basilisk_config::{BasiliskConfig, RuleSeverity as ConfigSeverity};

use super::model::{RuleDescriptor, RuleSelector, RuleSeverity, TagKind};

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

/// Map a wire severity back onto the persisted config severity.
pub(super) const fn wire_to_config(value: RuleSeverity) -> ConfigSeverity {
    match value {
        RuleSeverity::Error => ConfigSeverity::Error,
        RuleSeverity::Warning => ConfigSeverity::Warning,
        RuleSeverity::Info => ConfigSeverity::Info,
        RuleSeverity::Disabled => ConfigSeverity::Disabled,
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
        })
        .collect()
}

/// Resolve one rule's effective severity at the root scope.
///
/// Implements [CHKARCH-CONFIG-MODEL] / [CHKARCH-COMMANDS]: the configured
/// resolution wins when a table decides the rule; otherwise `pep` rules run
/// at `error` and every other rule is disabled. A `disabled` resolution can
/// never apply to a `pep` rule — such configuration is invalid and the rule
/// keeps running at `error`.
pub(super) fn effective_severity(
    descriptor: &RuleDescriptor,
    config: &BasiliskConfig,
) -> RuleSeverity {
    let tags: Vec<&str> = descriptor.tags.iter().map(String::as_str).collect();
    let resolved = config
        .resolve_severity(&descriptor.code, &tags)
        .map(config_to_wire);
    let is_pep = basilisk_checker::is_pep_rule(&descriptor.code);
    match resolved {
        // An (invalid) disabled resolution never applies to a pep rule, and
        // with no deciding table a pep rule runs at error.
        None | Some(RuleSeverity::Disabled) if is_pep => RuleSeverity::Error,
        Some(severity) => severity,
        None => RuleSeverity::Disabled,
    }
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

/// Expand an occurrence selector against the live catalog.
///
/// Implements [CONFIGEDITOR-OPERATIONS]: selectors exist only on the read
/// side (`basilisk/ruleOccurrences`); mutations never take selectors.
pub(super) fn expand_selector(
    selector: &RuleSelector,
    catalog: &[RuleDescriptor],
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
    };
    Ok(catalog
        .iter()
        .filter(|rule| selected.contains(rule.code.as_str()))
        .map(|rule| rule.code.clone())
        .collect())
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
#[expect(
    clippy::expect_used,
    reason = "test-only: a rule missing from the live registry must abort naming it"
)]
mod tests {
    use std::collections::HashMap;

    use basilisk_config::RuleSeverity as ConfigSeverity;

    use super::{
        config_to_wire, descriptors, effective_severity, expand_selector, tag_kind, wire_severity,
        wire_to_config, SelectionError,
    };
    use crate::configuration_editor::model::{RuleSelector, RuleSeverity, TagKind};

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
            assert_eq!(wire_to_config(wire), config);
        }
    }

    // Implements [CONFIGEDITOR-TAGS]: descriptors mirror the live checker
    // registry, complete with docs links and provenance tags.
    #[test]
    fn descriptors_expose_the_live_checker_registry() {
        let catalog = descriptors();
        assert_eq!(catalog.len(), basilisk_checker::rule_catalog().len());
        let annotation = catalog.iter().find(|rule| rule.code == "BSK-0001");
        let annotation = annotation.expect("BSK-0001 must exist in the live registry");
        assert!(annotation.docs_url.contains("BSK-0001"));
        assert!(!annotation.title.is_empty());
        assert!(!annotation.tags.is_empty());
    }

    /// [CHKARCH-CONFIG-MODEL] / [CHKARCH-COMMANDS]: with no deciding table a
    /// `pep` rule runs at `error` and an analyze rule is disabled; explicit
    /// entries win; `disabled` never lands on a `pep` rule.
    #[test]
    fn effective_severity_applies_the_scope_default_partition() {
        let catalog = descriptors();
        let pep = catalog
            .iter()
            .find(|rule| basilisk_checker::is_pep_rule(&rule.code));
        let analyze = catalog
            .iter()
            .find(|rule| !basilisk_checker::is_pep_rule(&rule.code));
        let pep = pep.expect("the registry holds at least one pep rule");
        let analyze = analyze.expect("the registry holds at least one analyze rule");
        let bare = basilisk_config::BasiliskConfig::default();
        assert_eq!(effective_severity(pep, &bare), RuleSeverity::Error);
        assert_eq!(effective_severity(analyze, &bare), RuleSeverity::Disabled);

        let graded = basilisk_config::BasiliskConfig::with_rule_entries(HashMap::from([
            (pep.code.clone(), ConfigSeverity::Info),
            (analyze.code.clone(), ConfigSeverity::Warning),
        ]));
        assert_eq!(effective_severity(pep, &graded), RuleSeverity::Info);
        assert_eq!(effective_severity(analyze, &graded), RuleSeverity::Warning);

        // An (invalid) pep-disable resolution never surfaces as Disabled.
        let invalid = basilisk_config::BasiliskConfig::with_rule_entries(HashMap::from([(
            pep.code.clone(),
            ConfigSeverity::Disabled,
        )]));
        assert_eq!(effective_severity(pep, &invalid), RuleSeverity::Error);
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

    // Implements [CONFIGEDITOR-OPERATIONS]: the occurrence selectors are
    // all/codes/tags — nothing else — and unknown names fail loudly.
    #[test]
    fn selector_expansion_covers_every_selector_kind() {
        let catalog = descriptors();
        let all = expand_selector(&RuleSelector::All, &catalog);
        assert_eq!(all.map(|codes| codes.len()), Ok(catalog.len()));

        let codes = expand_selector(
            &RuleSelector::Codes {
                codes: vec!["BSK-0001".to_owned()],
            },
            &catalog,
        );
        assert_eq!(codes, Ok(vec!["BSK-0001".to_owned()]));
        assert_eq!(
            expand_selector(
                &RuleSelector::Codes {
                    codes: vec!["NOT-A-RULE".to_owned()],
                },
                &catalog,
            ),
            Err(SelectionError::UnknownRule("NOT-A-RULE".to_owned()))
        );

        let tagged = expand_selector(
            &RuleSelector::Tags {
                tags: vec!["suppressions".to_owned()],
                match_all: false,
            },
            &catalog,
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
        );
        assert_eq!(impossible_conjunction, Ok(Vec::new()));
        assert_eq!(
            expand_selector(
                &RuleSelector::Tags {
                    tags: vec!["not-a-tag".to_owned()],
                    match_all: false,
                },
                &catalog,
            ),
            Err(SelectionError::UnknownTag("not-a-tag".to_owned()))
        );
    }
}
