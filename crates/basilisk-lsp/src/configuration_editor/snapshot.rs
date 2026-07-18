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
    ConfigurationProblem, ConfigurationSnapshot, ConfigurationSource, DebtSummary,
    PathOverrideState, PathRuleSetting, PathTagSetting, RuleDescriptor, RuleOccurrence,
    RuleOccurrencesResponse, RuleSeverity, RuleState, SourcePosition, SourceRange, TagState,
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
    let tags = tag_states(&rules, entries);
    let debt = debt_summary(&rules, &inventory);
    let problems = configuration_problems(entries, &catalog, &path_uri(&document.path));
    ConfigurationSnapshot {
        root_uri: path_uri(root),
        config_uri: path_uri(&document.path),
        revision: document.revision.clone(),
        source: ConfigurationSource {
            uri: path_uri(&document.path),
            exists: document.exists,
            read_only: document.read_only,
        },
        tags,
        rules,
        path_overrides: path_override_states(document),
        debt,
        problems,
    }
}

/// Fold the resolved rule states and diagnostic inventory into the Overview /
/// Adoption counters. Every value is a real count of live checker state — the
/// exact effective state, never a synthetic score ([CONFIGEDITOR-VSIX-EXPERIENCE]).
fn debt_summary(rules: &[RuleState], inventory: &Inventory) -> DebtSummary {
    let disabled_rules = rules
        .iter()
        .filter(|state| state.effective_severity == RuleSeverity::Disabled)
        .count();
    // A `pep` rule graded below `error` is the adoption signature: pep rules
    // always run, so a warning/info entry is a deliberately-adopted exception.
    let adopted_rules = rules
        .iter()
        .filter(|state| {
            state.descriptor.tags.iter().any(|tag| tag == "pep")
                && matches!(
                    state.effective_severity,
                    RuleSeverity::Warning | RuleSeverity::Info
                )
        })
        .count();
    DebtSummary {
        remaining_diagnostics: count_i64(inventory.total()),
        error_diagnostics: count_i64(inventory.errors),
        warning_diagnostics: count_i64(inventory.warnings),
        info_diagnostics: count_i64(inventory.infos),
        adopted_rules: count_i64(adopted_rules),
        disabled_rules: count_i64(disabled_rules),
    }
}

/// Real configuration problems for the Project view: entries in the active
/// `[tool.basilisk.rules]` table naming a rule code the catalog does not know.
fn configuration_problems(
    entries: Option<&RuleTables>,
    catalog: &[RuleDescriptor],
    uri: &str,
) -> Vec<ConfigurationProblem> {
    let known: HashSet<&str> = catalog
        .iter()
        .map(|descriptor| descriptor.code.as_str())
        .collect();
    let Some(tables) = entries else {
        return Vec::new();
    };
    let mut problems: Vec<ConfigurationProblem> = tables
        .rules
        .keys()
        .filter(|code| !known.contains(code.as_str()))
        .map(|code| ConfigurationProblem {
            code: code.clone(),
            message: format!("`{code}` is not a known rule code"),
            uri: uri.to_owned(),
            line: 0,
            character: 0,
        })
        .collect();
    problems.sort_by(|left, right| left.code.cmp(&right.code));
    problems
}

/// Enumerate nested per-directory `[tool.basilisk]` tables under the root — the
/// path-scoped config the checker honors for subtrees via nearest-first
/// discovery ([CHKARCH-CONFIG-DISCOVERY]). Excludes the root's own active
/// document. Built only when the editor requests a snapshot (never on the check
/// hot path), so the bounded filesystem walk is off the benchmark path.
fn path_override_states(document: &ConfigDocument) -> Vec<PathOverrideState> {
    let root = document.root.as_path();
    let mut overrides: Vec<PathOverrideState> = nested_config_dirs(root)
        .into_iter()
        .filter_map(|dir| {
            let nested = basilisk_config::discover_config_document(&dir).ok()?;
            if !nested.exists {
                return None;
            }
            let tables = nested.config.nearest_tables()?;
            if tables.is_empty() {
                return None;
            }
            let mut rules: Vec<PathRuleSetting> = tables
                .rules
                .iter()
                .map(|(code, severity)| PathRuleSetting {
                    code: code.clone(),
                    severity: super::catalog::config_to_wire(*severity),
                })
                .collect();
            rules.sort_by(|left, right| left.code.cmp(&right.code));
            let mut tags: Vec<PathTagSetting> = tables
                .rule_tags
                .iter()
                .map(|(tag, severity)| PathTagSetting {
                    tag: tag.clone(),
                    severity: super::catalog::config_to_wire(*severity),
                })
                .collect();
            tags.sort_by(|left, right| left.tag.cmp(&right.tag));
            let path = dir
                .strip_prefix(root)
                .unwrap_or(&dir)
                .to_string_lossy()
                .replace('\\', "/");
            Some(PathOverrideState {
                path,
                config_uri: path_uri(&nested.path),
                rules,
                tags,
            })
        })
        .collect();
    overrides.sort_by(|left, right| left.path.cmp(&right.path));
    overrides
}

/// Directories strictly below `root` that hold a `pyproject.toml`, skipping the
/// root itself, dot-directories, and default-excluded dirs (venv, caches, …).
fn nested_config_dirs(root: &Path) -> Vec<std::path::PathBuf> {
    fn excluded(name: &std::ffi::OsStr) -> bool {
        name.to_str().is_none_or(|name| {
            name.starts_with('.') || basilisk_config::DEFAULT_EXCLUDES.contains(&name)
        })
    }
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut has_config = false;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if !excluded(&entry.file_name()) {
                    stack.push(path);
                }
            } else if entry.file_name() == "pyproject.toml" {
                has_config = true;
            }
        }
        if has_config && dir != root {
            found.push(dir);
        }
    }
    found
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
