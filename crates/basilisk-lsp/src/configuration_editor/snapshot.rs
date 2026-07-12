//! Snapshot, impact, and occurrence projections over one workspace root.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use basilisk_config::{BasiliskConfig, ConfigDocument, ConfigFormat, RuleSeverity as ConfigSeverity};
use tower_lsp::lsp_types::Url;

use super::catalog::{
    config_to_wire, descriptors, is_fixable, is_safe_fixable, severities, tag_kind, wire_severity,
};
use super::model::{
    ConfigurationFormat, ConfigurationSnapshot, ConfigurationSource, DebtSummary, FixSafety,
    RuleOccurrence, RuleOccurrencesResponse, RuleSeverity, RuleState, SourcePosition, SourceRange,
    TagState,
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
    for entry in index.files.iter().filter(|entry| entry.key().starts_with(root)) {
        for diagnostic in &entry.diagnostics {
            let code = diagnostic.code.code.to_owned();
            *counts.entry(code.clone()).or_insert(0) += 1;
            let _ = files_by_code.entry(code).or_default().insert(entry.key().clone());
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
    Inventory { counts, files, total, errors, warnings }
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
    for entry in index.files.iter().filter(|entry| entry.key().starts_with(root)) {
        let Some(resolved) = &entry.resolved else { continue };
        let diagnostics = basilisk_checker::check_with_config(resolved, config);
        for diagnostic in diagnostics {
            let code = diagnostic.code.code.to_owned();
            *counts.entry(code.clone()).or_insert(0) += 1;
            let _ = files_by_code.entry(code).or_default().insert(entry.key().clone());
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
    Inventory { counts, files, total, errors, warnings }
}

/// Build a complete wire snapshot from a validated active document.
pub(super) fn build_snapshot(
    index: &WorkspaceIndex,
    root: &Path,
    document: &ConfigDocument,
) -> ConfigurationSnapshot {
    let catalog = descriptors();
    let inventory = inventory(index, root);
    let adoption = adoption_rules(document);
    let rules: Vec<RuleState> = catalog
        .iter()
        .map(|descriptor| {
            let (configured_severity, effective_severity) = severities(descriptor, &document.config);
            let diagnostic_count = count_i64(*inventory.counts.get(&descriptor.code).unwrap_or(&0));
            RuleState {
                descriptor: descriptor.clone(),
                configured_severity,
                effective_severity,
                inherited: configured_severity.is_none(),
                diagnostic_count,
                affected_file_count: count_i64(*inventory.files.get(&descriptor.code).unwrap_or(&0)),
                safe_fix_count: if is_safe_fixable(&descriptor.code) { diagnostic_count } else { 0 },
                unsafe_fix_count: if is_fixable(&descriptor.code) && !is_safe_fixable(&descriptor.code) {
                    diagnostic_count
                } else {
                    0
                },
                adoption_exception_count: count_i64(
                    adoption.iter().filter(|(_, code)| code == &&descriptor.code).count(),
                ),
            }
        })
        .collect();
    let tags = tag_states(&rules);
    let source_uri = path_uri(&document.path);
    let shadowed_sources = document.shadowed_sources.iter().map(|path| path_uri(path)).collect();
    ConfigurationSnapshot {
        root_uri: path_uri(root),
        revision: document.revision.clone(),
        source: ConfigurationSource {
            uri: source_uri,
            format: match document.format {
                ConfigFormat::PyprojectToml => ConfigurationFormat::PyprojectToml,
                ConfigFormat::BasiliskJson => ConfigurationFormat::BasiliskJson,
            },
            exists: document.exists,
            read_only: document.read_only,
            shadowed_sources,
        },
        debt: DebtSummary {
            remaining_diagnostics: count_i64(inventory.total),
            adopted_files: count_i64(adoption.iter().map(|(path, _)| path).collect::<HashSet<_>>().len()),
            adoption_exceptions: count_i64(adoption.len()),
            suppression_diagnostics: suppression_count(&inventory.counts),
            disabled_rules: count_i64(
                rules.iter().filter(|state| state.effective_severity == RuleSeverity::Disabled).count(),
            ),
        },
        rules,
        tags,
        problems: Vec::new(),
    }
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
    for entry in index.files.iter().filter(|entry| entry.key().starts_with(root)) {
        let Some(uri) = Url::from_file_path(entry.key()).ok() else { continue };
        for diagnostic in entry.diagnostics.iter().filter(|diag| selected.contains(diag.code.code)) {
            let start = crate::util::byte_offset_to_position(&entry.text, diagnostic.span.start_usize());
            let end = crate::util::byte_offset_to_position(&entry.text, diagnostic.span.end_usize());
            items.push(RuleOccurrence {
                rule_code: diagnostic.code.code.to_owned(),
                uri: uri.to_string(),
                range: SourceRange {
                    start: SourcePosition { line: i64::from(start.line), character: i64::from(start.character) },
                    end: SourcePosition { line: i64::from(end.line), character: i64::from(end.character) },
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
        (&left.uri, left.range.start.line, left.range.start.character, &left.rule_code)
            .cmp(&(&right.uri, right.range.start.line, right.range.start.character, &right.rule_code))
    });
    let offset = cursor.and_then(|value| value.parse::<usize>().ok()).unwrap_or(0).min(items.len());
    let end = offset.saturating_add(limit).min(items.len());
    let next_cursor = (end < items.len()).then(|| end.to_string());
    RuleOccurrencesResponse { items: items[offset..end].to_vec(), next_cursor }
}

fn adoption_rules(document: &ConfigDocument) -> Vec<(String, String)> {
    match document.format {
        ConfigFormat::PyprojectToml => adoption_rules_toml(&document.content),
        ConfigFormat::BasiliskJson => adoption_rules_json(&document.content),
    }
}

fn adoption_rules_toml(content: &str) -> Vec<(String, String)> {
    let Ok(table) = content.parse::<toml::Table>() else { return Vec::new() };
    let Some(paths) = table
        .get("tool").and_then(|value| value.get("basilisk"))
        .and_then(|value| value.get("per-path-overrides"))
        .and_then(toml::Value::as_table)
    else { return Vec::new() };
    paths.iter().flat_map(|(pattern, entry)| {
        let adopted = entry.get("adoption").and_then(toml::Value::as_bool) == Some(true);
        entry.get("rules").and_then(toml::Value::as_table).into_iter().flatten()
            .filter(move |_| adopted)
            .map(move |(code, _)| (pattern.clone(), code.clone()))
    }).collect()
}

fn adoption_rules_json(content: &str) -> Vec<(String, String)> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(content) else { return Vec::new() };
    let Some(paths) = value.get("perPathOverrides").or_else(|| value.get("per-path-overrides"))
        .and_then(serde_json::Value::as_object) else { return Vec::new() };
    paths.iter().flat_map(|(pattern, entry)| {
        let adopted = entry.get("adoption").and_then(serde_json::Value::as_bool) == Some(true);
        entry.get("rules").and_then(serde_json::Value::as_object).into_iter().flatten()
            .filter(move |_| adopted)
            .map(move |(code, _)| (pattern.clone(), code.clone()))
    }).collect()
}

fn suppression_count(counts: &HashMap<String, usize>) -> i64 {
    ["BSK-I0060", "BSK-W0061", "BSK-W0062", "BSK-E0063"]
        .iter()
        .map(|code| counts.get(*code).copied().unwrap_or(0))
        .sum::<usize>()
        .try_into()
        .unwrap_or(i64::MAX)
}

fn path_uri(path: &Path) -> String {
    Url::from_file_path(path).map_or_else(|()| path.to_string_lossy().into_owned(), |uri| uri.to_string())
}

fn count_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// Convert a wire severity into the persisted config type.
pub(super) const fn persisted_severity(value: RuleSeverity) -> ConfigSeverity {
    match value {
        RuleSeverity::Error => ConfigSeverity::Error,
        RuleSeverity::Warning => ConfigSeverity::Warning,
        RuleSeverity::Info => ConfigSeverity::Info,
        RuleSeverity::Disabled => ConfigSeverity::Disabled,
    }
}
