//! Snapshot, impact, and occurrence projections over one workspace root.
//!
//! Implements [CONFIGEDITOR-OPERATIONS] / [CONFIGEDITOR-MODEL]: a snapshot is
//! the catalog with per-rule entries, effective severities, diagnostic
//! counts, and tag states with their entries — nothing else.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use basilisk_config::{BasiliskConfig, ConfigDocument, RuleTables};
use tower_lsp::lsp_types::Url;

use super::catalog::{descriptors, effective_severity, tag_kind, wire_severity};
use super::model::{
    ConfigurationSnapshot, RuleOccurrence, RuleOccurrencesResponse, RuleState, SourcePosition,
    SourceRange, TagState,
};
use crate::workspace::WorkspaceIndex;

/// Per-root diagnostic inventory used by snapshots and previews.
pub(super) struct Inventory {
    /// Occurrence count by code.
    pub counts: HashMap<String, usize>,
    /// Error diagnostics.
    pub errors: usize,
    /// Warning diagnostics.
    pub warnings: usize,
    /// Info diagnostics.
    pub infos: usize,
}

impl Inventory {
    fn new() -> Self {
        Self {
            counts: HashMap::new(),
            errors: 0,
            warnings: 0,
            infos: 0,
        }
    }

    fn record(&mut self, code: &str, severity: basilisk_checker::Severity) {
        *self.counts.entry(code.to_owned()).or_insert(0) += 1;
        match severity {
            basilisk_checker::Severity::Error | basilisk_checker::Severity::SafetyViolation => {
                self.errors += 1;
            }
            basilisk_checker::Severity::Warning => self.warnings += 1,
            basilisk_checker::Severity::Info => self.infos += 1,
        }
    }

    /// Total emitted diagnostics — the sum of the three severity partitions.
    #[cfg(test)]
    pub(super) fn total(&self) -> usize {
        self.errors + self.warnings + self.infos
    }
}

/// Build current counts from the indexed diagnostics owned by `root`.
pub(super) fn inventory(index: &WorkspaceIndex, root: &Path) -> Inventory {
    let mut inventory = Inventory::new();
    for entry in index.files.iter().filter(|entry| {
        entry.key().starts_with(root) && index.configuration_path_is_in_scope(entry.key())
    }) {
        for diagnostic in &entry.diagnostics {
            inventory.record(diagnostic.code.code, diagnostic.severity);
        }
    }
    inventory
}

/// Analyse current indexed modules using a hypothetical root config.
pub(super) fn hypothetical_inventory(
    index: &WorkspaceIndex,
    root: &Path,
    config: &BasiliskConfig,
) -> Inventory {
    let mut inventory = Inventory::new();
    for entry in index.files.iter().filter(|entry| {
        entry.key().starts_with(root) && index.configuration_path_is_in_scope(entry.key())
    }) {
        let Some(resolved) = &entry.resolved else {
            continue;
        };
        for diagnostic in basilisk_checker::check_with_config(resolved, config) {
            inventory.record(diagnostic.code.code, diagnostic.severity);
        }
    }
    inventory
}

/// Build a complete wire snapshot from a validated active document.
pub(super) fn build_snapshot(
    index: &WorkspaceIndex,
    root: &Path,
    document: &ConfigDocument,
) -> ConfigurationSnapshot {
    let catalog = descriptors();
    let inventory = inventory(index, root);
    let entries = document.config.nearest_tables();
    let rules: Vec<RuleState> = catalog
        .iter()
        .map(|descriptor| RuleState {
            descriptor: descriptor.clone(),
            entry: entries
                .and_then(|tables| tables.rules.get(&descriptor.code))
                .copied()
                .map(super::catalog::config_to_wire),
            effective_severity: effective_severity(descriptor, &document.config),
            diagnostic_count: count_i64(*inventory.counts.get(&descriptor.code).unwrap_or(&0)),
        })
        .collect();
    ConfigurationSnapshot {
        root_uri: path_uri(root),
        config_uri: path_uri(&document.path),
        revision: document.revision.clone(),
        tags: tag_states(&rules, entries),
        rules,
    }
}

/// Fold rule states into per-tag facets with their explicit tag entries.
///
/// Implements [CONFIGEDITOR-TAGS]: a tag is both a facet (grouping) and,
/// when written into `[tool.basilisk.rule-tags]`, one visible entry.
fn tag_states(rules: &[RuleState], entries: Option<&RuleTables>) -> Vec<TagState> {
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
            entry: entries
                .and_then(|tables| tables.rule_tags.get(&name))
                .copied()
                .map(super::catalog::config_to_wire),
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
                code: diagnostic.code.code.to_owned(),
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
                severity: wire_severity(diagnostic.severity),
            });
        }
    }
    items.sort_by(|left, right| {
        (
            &left.uri,
            left.range.start.line,
            left.range.start.character,
            &left.code,
        )
            .cmp(&(
                &right.uri,
                right.range.start.line,
                right.range.start.character,
                &right.code,
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

pub(super) fn path_uri(path: &Path) -> String {
    Url::from_file_path(path).map_or_else(
        |()| path.to_string_lossy().into_owned(),
        |uri| uri.to_string(),
    )
}

pub(super) fn count_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
