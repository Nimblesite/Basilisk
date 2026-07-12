//! Implements [CONFIGEDITOR-TAGS] and [CHKTAG-CONSUMERS].
//! Public, live metadata for every rule in the checker registry.

use crate::diagnostic::Severity;

/// Stable metadata needed by configuration clients.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleDescriptor {
    /// Stable diagnostic code written in configuration.
    pub code: &'static str,
    /// Short user-facing rule name.
    pub title: &'static str,
    /// Concise description suitable for a catalog row.
    pub summary: &'static str,
    /// Canonical documentation page.
    pub docs_url: &'static str,
    /// Native severity when the rule is enabled and not overridden.
    pub default_severity: Severity,
    /// Whether an unconfigured project selects this rule.
    pub default_enabled: bool,
    /// Canonical provenance, PEP-category, and descriptive tags.
    pub tags: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy)]
struct GeneratedRuleDescriptor {
    code: &'static str,
    title: &'static str,
    summary: &'static str,
    docs_url: &'static str,
    default_severity: Severity,
}

include!(concat!(env!("OUT_DIR"), "/rule_catalog_generated.rs"));

/// Return the complete rule catalog in live registry order.
///
/// Static prose and code metadata are generated from each registered rule's
/// source header. Tags and default selection are resolved at runtime through
/// the same `opt_in_spec` declarations used by checking, so consumers cannot
/// drift from rule selection.
#[must_use]
pub fn rule_catalog() -> Vec<RuleDescriptor> {
    GENERATED_RULES
        .iter()
        .map(|rule| RuleDescriptor {
            code: rule.code,
            title: rule.title,
            summary: rule.summary,
            docs_url: rule.docs_url,
            default_severity: rule.default_severity,
            default_enabled: crate::rule_tags::opt_in_spec_for_code(rule.code).is_none(),
            tags: crate::rule_tags::tags_for_code(rule.code),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::rule_catalog;

    #[test]
    fn generated_catalog_has_one_entry_per_registered_rule() {
        let catalog = rule_catalog();
        assert_eq!(catalog.len(), crate::rules::registered_rule_count());
        let codes = catalog
            .iter()
            .map(|rule| rule.code)
            .collect::<BTreeSet<_>>();
        assert_eq!(codes.len(), catalog.len(), "catalog codes must be unique");
    }
}
