//! Snapshot, impact, and occurrence projections over one workspace root.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use basilisk_config::{adoption_rule_overrides, BasiliskConfig, ConfigDocument, ConfigFormat};
use tower_lsp::lsp_types::Url;

use super::catalog::{
    config_to_wire, descriptors, is_fixable, is_safe_fixable, severities, tag_kind, wire_severity,
};
use super::model::{
    ConfigurationFormat, ConfigurationMutation, ConfigurationPreset, ConfigurationSnapshot,
    ConfigurationSource, DebtSummary, FixSafety, MutationScope, PathOverrideState, PathRuleSetting,
    RuleDescriptor, RuleOccurrence, RuleOccurrencesResponse, RuleSelector, RuleSetting,
    RuleSeverity, RuleState, SourcePosition, SourceRange, TagState,
};
use crate::workspace::WorkspaceIndex;

/// Per-root diagnostic inventory used by snapshots and selectors.
pub(super) struct Inventory {
    /// Occurrence count by code.
    pub counts: HashMap<String, usize>,
    /// Distinct affected file count by code.
    pub files: HashMap<String, usize>,
    /// Total emitted diagnostics.
    pub total: usize,
    /// Error diagnostics.
    pub errors: usize,
    /// Warning diagnostics.
    pub warnings: usize,
}

/// Build current counts from the indexed diagnostics owned by `root`.
pub(super) fn inventory(index: &WorkspaceIndex, root: &Path) -> Inventory {
    let mut counts = HashMap::new();
    let mut files_by_code: HashMap<String, HashSet<std::path::PathBuf>> = HashMap::new();
    let mut errors = 0;
    let mut warnings = 0;
    for entry in index.files.iter().filter(|entry| {
        entry.key().starts_with(root) && index.configuration_path_is_in_scope(entry.key())
    }) {
        for diagnostic in &entry.diagnostics {
            let code = diagnostic.code.code.to_owned();
            *counts.entry(code.clone()).or_insert(0) += 1;
            let _ = files_by_code
                .entry(code)
                .or_default()
                .insert(entry.key().clone());
            match diagnostic.severity {
                basilisk_checker::Severity::Error | basilisk_checker::Severity::SafetyViolation => {
                    errors += 1;
                }
                basilisk_checker::Severity::Warning => warnings += 1,
                basilisk_checker::Severity::Info => {}
            }
        }
    }
    let total = counts.values().sum();
    let files = files_by_code
        .into_iter()
        .map(|(code, paths)| (code, paths.len()))
        .collect();
    Inventory {
        counts,
        files,
        total,
        errors,
        warnings,
    }
}

/// Analyse current indexed modules using a hypothetical root config.
pub(super) fn hypothetical_inventory(
    index: &WorkspaceIndex,
    root: &Path,
    config: &BasiliskConfig,
) -> Inventory {
    let mut counts = HashMap::new();
    let mut files_by_code: HashMap<String, HashSet<std::path::PathBuf>> = HashMap::new();
    let mut errors = 0;
    let mut warnings = 0;
    for entry in index.files.iter().filter(|entry| {
        entry.key().starts_with(root) && index.configuration_path_is_in_scope(entry.key())
    }) {
        let Some(resolved) = &entry.resolved else {
            continue;
        };
        let diagnostics = basilisk_checker::check_with_config(resolved, config);
        for diagnostic in diagnostics {
            let code = diagnostic.code.code.to_owned();
            *counts.entry(code.clone()).or_insert(0) += 1;
            let _ = files_by_code
                .entry(code)
                .or_default()
                .insert(entry.key().clone());
            match diagnostic.severity {
                basilisk_checker::Severity::Error | basilisk_checker::Severity::SafetyViolation => {
                    errors += 1;
                }
                basilisk_checker::Severity::Warning => warnings += 1,
                basilisk_checker::Severity::Info => {}
            }
        }
    }
    let total = counts.values().sum();
    let files = files_by_code
        .into_iter()
        .map(|(code, paths)| (code, paths.len()))
        .collect();
    Inventory {
        counts,
        files,
        total,
        errors,
        warnings,
    }
}

/// Build a complete wire snapshot from a validated active document.
pub(super) fn build_snapshot(
    index: &WorkspaceIndex,
    root: &Path,
    document: &ConfigDocument,
) -> ConfigurationSnapshot {
    let catalog = descriptors();
    let inventory = inventory(index, root);
    let adoption_entries = adoption_rule_overrides(document);
    let adoption: Vec<(String, String)> = adoption_entries
        .clone()
        .into_iter()
        .flat_map(|(pattern, rules)| rules.into_keys().map(move |code| (pattern.clone(), code)))
        .collect();
    let rules: Vec<RuleState> = catalog
        .iter()
        .map(|descriptor| {
            let (configured_severity, effective_severity) =
                severities(descriptor, &document.config);
            let diagnostic_count = count_i64(*inventory.counts.get(&descriptor.code).unwrap_or(&0));
            RuleState {
                descriptor: descriptor.clone(),
                configured_severity,
                effective_severity,
                inherited: configured_severity.is_none(),
                diagnostic_count,
                affected_file_count: count_i64(
                    *inventory.files.get(&descriptor.code).unwrap_or(&0),
                ),
                safe_fix_count: if is_safe_fixable(&descriptor.code) {
                    diagnostic_count
                } else {
                    0
                },
                unsafe_fix_count: if is_fixable(&descriptor.code)
                    && !is_safe_fixable(&descriptor.code)
                {
                    diagnostic_count
                } else {
                    0
                },
                adoption_exception_count: count_i64(
                    adoption
                        .iter()
                        .filter(|(_, code)| code == &descriptor.code)
                        .count(),
                ),
            }
        })
        .collect();
    let tags = tag_states(&rules);
    let source_uri = path_uri(&document.path);
    let shadowed_sources = document
        .shadowed_sources
        .iter()
        .map(|path| path_uri(path))
        .collect();
    ConfigurationSnapshot {
        root_uri: path_uri(root),
        revision: document.revision.clone(),
        source: ConfigurationSource {
            uri: source_uri,
            format: match document.format {
                ConfigFormat::PyprojectToml => ConfigurationFormat::PyprojectToml,
            },
            exists: document.exists,
            read_only: document.read_only,
            shadowed_sources,
        },
        debt: DebtSummary {
            remaining_diagnostics: count_i64(inventory.total),
            adopted_files: count_i64(
                adoption
                    .iter()
                    .map(|(path, _)| path)
                    .collect::<HashSet<_>>()
                    .len(),
            ),
            adoption_exceptions: count_i64(adoption.len()),
            suppression_diagnostics: suppression_count(&inventory.counts, &catalog),
            disabled_rules: count_i64(
                rules
                    .iter()
                    .filter(|state| state.effective_severity == RuleSeverity::Disabled)
                    .count(),
            ),
        },
        rules,
        tags,
        presets: presets(),
        path_overrides: path_override_states(document, &adoption_entries),
        problems: Vec::new(),
    }
}

fn presets() -> Vec<ConfigurationPreset> {
    vec![
        ConfigurationPreset {
            id: "strict".to_owned(),
            name: "Strict".to_owned(),
            summary: "Enable every live rule at its native severity.".to_owned(),
            mutations: vec![ConfigurationMutation {
                selector: RuleSelector::All,
                setting: RuleSetting::Native,
                scope: MutationScope::Project,
            }],
        },
        ConfigurationPreset {
            id: "maximum".to_owned(),
            name: "Maximum".to_owned(),
            summary: "Enable every live rule and promote every diagnostic to an error.".to_owned(),
            mutations: vec![ConfigurationMutation {
                selector: RuleSelector::All,
                setting: RuleSetting::Error,
                scope: MutationScope::Project,
            }],
        },
        ConfigurationPreset {
            id: "suppression-audit".to_owned(),
            name: "Suppression audit".to_owned(),
            summary: "Surface unused, broad, conflicting, and malformed suppressions.".to_owned(),
            mutations: vec![ConfigurationMutation {
                selector: RuleSelector::Tags {
                    tags: vec!["suppressions".to_owned()],
                    match_all: false,
                },
                setting: RuleSetting::Native,
                scope: MutationScope::Project,
            }],
        },
    ]
}

fn path_override_states(
    document: &ConfigDocument,
    adoption: &BTreeMap<String, BTreeMap<String, basilisk_config::RuleSeverity>>,
) -> Vec<PathOverrideState> {
    let mut entries: Vec<_> = document.config.per_path_overrides.iter().collect();
    entries.sort_by_key(|(pattern, _)| (*pattern).clone());
    entries
        .into_iter()
        .map(|(pattern, entry)| {
            let mut rules: BTreeMap<String, RuleSeverity> = entry
                .rule_overrides
                .iter()
                .map(|(code, severity)| (code.clone(), config_to_wire(*severity)))
                .collect();
            for code in &entry.disabled_rules {
                let _ = rules.insert(code.clone(), RuleSeverity::Disabled);
            }
            PathOverrideState {
                pattern: pattern.clone(),
                adoption: adoption.contains_key(pattern),
                rules: rules
                    .into_iter()
                    .map(|(rule_code, severity)| PathRuleSetting {
                        rule_code,
                        severity,
                    })
                    .collect(),
            }
        })
        .collect()
}

fn tag_states(rules: &[RuleState]) -> Vec<TagState> {
    let mut values: BTreeMap<String, (usize, i64)> = BTreeMap::new();
    for rule in rules {
        for tag in &rule.descriptor.tags {
            let entry = values.entry(tag.clone()).or_default();
            entry.0 += 1;
            entry.1 += rule.diagnostic_count;
        }
    }
    values
        .into_iter()
        .map(|(name, (rule_count, diagnostic_count))| TagState {
            kind: tag_kind(&name),
            name,
            rule_count: count_i64(rule_count),
            diagnostic_count,
        })
        .collect()
}

/// Page through selected occurrences in stable URI/range/code order.
pub(super) fn occurrences(
    index: &WorkspaceIndex,
    root: &Path,
    source_uri: &str,
    selected: &HashSet<String>,
    cursor: Option<&str>,
    limit: usize,
) -> RuleOccurrencesResponse {
    let mut items = Vec::new();
    for entry in index
        .files
        .iter()
        .filter(|entry| entry.key().starts_with(root))
    {
        let Some(uri) = Url::from_file_path(entry.key()).ok() else {
            continue;
        };
        for diagnostic in entry
            .diagnostics
            .iter()
            .filter(|diag| selected.contains(diag.code.code))
        {
            let start =
                crate::util::byte_offset_to_position(&entry.text, diagnostic.span.start_usize());
            let end =
                crate::util::byte_offset_to_position(&entry.text, diagnostic.span.end_usize());
            items.push(RuleOccurrence {
                rule_code: diagnostic.code.code.to_owned(),
                uri: uri.to_string(),
                range: SourceRange {
                    start: SourcePosition {
                        line: i64::from(start.line),
                        character: i64::from(start.character),
                    },
                    end: SourcePosition {
                        line: i64::from(end.line),
                        character: i64::from(end.character),
                    },
                },
                effective_severity: wire_severity(diagnostic.severity),
                fix_safety: if is_safe_fixable(diagnostic.code.code) {
                    Some(FixSafety::Safe)
                } else if is_fixable(diagnostic.code.code) {
                    Some(FixSafety::Unsafe)
                } else {
                    None
                },
                configuration_source: source_uri.to_owned(),
            });
        }
    }
    items.sort_by(|left, right| {
        (
            &left.uri,
            left.range.start.line,
            left.range.start.character,
            &left.rule_code,
        )
            .cmp(&(
                &right.uri,
                right.range.start.line,
                right.range.start.character,
                &right.rule_code,
            ))
    });
    page_occurrences(&items, cursor, limit)
}

fn page_occurrences(
    items: &[RuleOccurrence],
    cursor: Option<&str>,
    limit: usize,
) -> RuleOccurrencesResponse {
    let offset = cursor
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0)
        .min(items.len());
    let end = offset.saturating_add(limit).min(items.len());
    let next_cursor = (end < items.len()).then(|| end.to_string());
    RuleOccurrencesResponse {
        items: items.get(offset..end).unwrap_or_default().to_vec(),
        next_cursor,
    }
}

fn suppression_count(counts: &HashMap<String, usize>, catalog: &[RuleDescriptor]) -> i64 {
    catalog
        .iter()
        .filter(|rule| rule.tags.iter().any(|tag| tag == "suppressions"))
        .map(|rule| counts.get(&rule.code).copied().unwrap_or(0))
        .sum::<usize>()
        .try_into()
        .unwrap_or(i64::MAX)
}

fn path_uri(path: &Path) -> String {
    Url::from_file_path(path).map_or_else(
        |()| path.to_string_lossy().into_owned(),
        |uri| uri.to_string(),
    )
}

fn count_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
