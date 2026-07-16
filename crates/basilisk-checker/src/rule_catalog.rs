//! Implements [CONFIGEDITOR-TAGS] and [CHKTAG-CONSUMERS].
//! Public, live metadata for every rule in the checker registry.

/// Stable metadata needed by configuration clients.
///
/// Mirrors `RuleDescriptor` in `models/configuration_editor.td`: code, prose,
/// documentation URL, and canonical tags. A descriptor carries no severity —
/// severity comes only from configuration entries ([CHKARCH-CONFIG-MODEL]),
/// and scope comes from the `pep` provenance tag ([CHKARCH-COMMANDS]).
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
    /// Canonical provenance, PEP-category, and descriptive tags.
    pub tags: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy)]
struct GeneratedRuleDescriptor {
    code: &'static str,
    title: &'static str,
    summary: &'static str,
    docs_url: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/rule_catalog_generated.rs"));

/// Return the complete rule catalog in live registry order.
///
/// Static prose and code metadata are generated from each registered rule's
/// source header. Tags are resolved at runtime through the same
/// `opt_in_spec` declarations used by checking, so consumers cannot drift
/// from rule selection.
#[must_use]
pub fn rule_catalog() -> Vec<RuleDescriptor> {
    GENERATED_RULES
        .iter()
        .map(|rule| RuleDescriptor {
            code: rule.code,
            title: rule.title,
            summary: rule.summary,
            docs_url: rule.docs_url,
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

    /// [CHKARCH-DIAG-CODES]: codes carry no severity class — `BSK-nnnn` or a
    /// conformance `snake_case` name, nothing else.
    #[test]
    fn codes_carry_no_severity_class() {
        for rule in rule_catalog() {
            let valid_bsk = rule
                .code
                .strip_prefix("BSK-")
                .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()));
            let valid_named = rule
                .code
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
            assert!(
                valid_bsk || valid_named,
                "rule code `{}` must be BSK-nnnn or snake_case",
                rule.code
            );
        }
    }
}
