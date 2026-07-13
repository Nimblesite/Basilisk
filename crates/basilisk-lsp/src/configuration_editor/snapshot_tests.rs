use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

use basilisk_config::{
    BasiliskConfig, ConfigDocument, ConfigFormat, PathOverride, RuleSeverity as ConfigSeverity,
};

use super::{
    build_snapshot, hypothetical_inventory, inventory, occurrences, page_occurrences,
    path_override_states, presets, suppression_count,
};
use crate::config::AnalysisMode;
use crate::configuration_editor::catalog::descriptors;
use crate::configuration_editor::model::{
    ConfigurationFormat, FixSafety, MutationScope, RuleOccurrence, RuleSelector, RuleSetting,
    RuleSeverity, SourcePosition, SourceRange,
};
use crate::workspace::WorkspaceIndex;

#[test]
fn snapshot_presets_are_one_shot_explicit_recipes() {
    let presets = presets();
    assert_eq!(
        presets
            .iter()
            .map(|preset| preset.id.as_str())
            .collect::<Vec<_>>(),
        vec!["strict", "maximum", "suppression-audit"]
    );
    let mutation = presets
        .iter()
        .find(|preset| preset.id == "strict")
        .and_then(|strict| strict.mutations.first());
    assert_eq!(
        mutation.map(|entry| &entry.selector),
        Some(&RuleSelector::All)
    );
    assert_eq!(
        mutation.map(|entry| entry.setting),
        Some(RuleSetting::Native)
    );
    assert_eq!(
        mutation.map(|entry| &entry.scope),
        Some(&MutationScope::Project)
    );
}

#[test]
fn path_override_inventory_is_sorted_normalized_and_marks_adoption() {
    let document = path_document();
    let adoption = BTreeMap::from([("legacy/**".to_owned(), BTreeMap::new())]);
    let states = path_override_states(&document, &adoption);
    assert_eq!(states.first().map(|state| state.adoption), Some(true));
    assert_eq!(
        states
            .first()
            .and_then(|state| state.rules.first())
            .map(|rule| rule.rule_code.as_str()),
        Some("BSK-E0001")
    );
    assert_eq!(
        states
            .first()
            .and_then(|state| state.rules.get(1))
            .map(|rule| (rule.rule_code.as_str(), rule.severity)),
        Some((
            "BSK-E0002",
            crate::configuration_editor::model::RuleSeverity::Disabled
        ))
    );
}

fn path_document() -> ConfigDocument {
    let mut config = BasiliskConfig::default();
    let _ = config.per_path_overrides.insert(
        "legacy/**".to_owned(),
        PathOverride {
            disabled_rules: vec!["BSK-E0002".to_owned()],
            rule_overrides: HashMap::from([("BSK-E0001".to_owned(), ConfigSeverity::Warning)]),
        },
    );
    ConfigDocument {
        root: PathBuf::from("/workspace"),
        path: PathBuf::from("/workspace/pyproject.toml"),
        format: ConfigFormat::PyprojectToml,
        exists: true,
        read_only: false,
        shadowed_sources: Vec::new(),
        content: "[tool.basilisk]\n".to_owned(),
        revision: "revision".to_owned(),
        config,
    }
}

#[test]
fn suppression_debt_follows_catalog_tags() {
    let catalog = descriptors();
    let counts: HashMap<String, usize> = catalog
        .iter()
        .filter(|rule| rule.tags.iter().any(|tag| tag == "suppressions"))
        .map(|rule| (rule.code.clone(), 2))
        .chain(std::iter::once(("assignment_compatibility".to_owned(), 99)))
        .collect();
    let tagged_rules = catalog
        .iter()
        .filter(|rule| rule.tags.iter().any(|tag| tag == "suppressions"))
        .count();
    assert_eq!(
        suppression_count(&counts, &catalog),
        i64::try_from(tagged_rules * 2).unwrap_or(i64::MAX)
    );
}

#[test]
fn occurrence_inventory_pages_beyond_the_first_hundred() {
    let items: Vec<_> = (0_i64..101).map(occurrence).collect();
    let first = page_occurrences(&items, None, 100);
    assert_eq!(first.items.len(), 100);
    assert_eq!(first.next_cursor.as_deref(), Some("100"));
    let second = page_occurrences(&items, first.next_cursor.as_deref(), 100);
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.next_cursor, None);
}

#[test]
fn hypothetical_inventory_keeps_excluded_open_files_out_of_preview_counts() {
    let root = std::env::temp_dir().join(format!(
        "basilisk-config-preview-scope-{}",
        std::process::id()
    ));
    let excluded = root.join("generated.py");
    assert!(std::fs::create_dir_all(&root).is_ok());
    assert!(std::fs::write(
        root.join("pyproject.toml"),
        "[tool.basilisk]\nexclude = [\"generated.py\"]\n",
    )
    .is_ok());
    let index = WorkspaceIndex::new(
        vec![root.clone()],
        AnalysisMode::OpenFilesOnly,
        BasiliskConfig::default(),
    );
    let uri = tower_lsp::lsp_types::Url::from_file_path(&excluded);
    assert!(uri.is_ok());
    if let Ok(uri) = uri {
        let diagnostics = index.set_open(&uri, "value: int = 'wrong'\n", 1);
        assert!(diagnostics.is_empty());
    }
    let mut strict = index.config_for_file(&excluded).as_ref().clone();
    let _ = strict
        .rules
        .insert("assignment_compatibility".to_owned(), ConfigSeverity::Error);
    let preview = hypothetical_inventory(&index, &root, &strict);
    assert_eq!(preview.total, 0);
    let _ = std::fs::remove_dir_all(root);
}

fn occurrence(line: i64) -> RuleOccurrence {
    RuleOccurrence {
        rule_code: "assignment_compatibility".to_owned(),
        uri: "file:///workspace/source.py".to_owned(),
        range: SourceRange {
            start: SourcePosition { line, character: 0 },
            end: SourcePosition { line, character: 1 },
        },
        effective_severity: RuleSeverity::Error,
        fix_safety: None,
        configuration_source: "file:///workspace/pyproject.toml".to_owned(),
    }
}

/// Temp root with `BSK-E0001` opted in and one open file violating it twice.
fn indexed_root(name: &str) -> Option<(PathBuf, WorkspaceIndex)> {
    let root = std::env::temp_dir().join(format!(
        "basilisk-config-snapshot-{name}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).ok()?;
    std::fs::write(
        root.join("pyproject.toml"),
        "[tool.basilisk.rules]\n\"BSK-E0001\" = \"error\"\n",
    )
    .ok()?;
    let index = WorkspaceIndex::new(
        vec![root.clone()],
        AnalysisMode::OpenFilesOnly,
        BasiliskConfig::default(),
    );
    let uri = tower_lsp::lsp_types::Url::from_file_path(root.join("app.py")).ok()?;
    let diagnostics = index.set_open(
        &uri,
        "def first(x):\n    return 1\n\ndef second(y):\n    return 2\n",
        1,
    );
    (!diagnostics.is_empty()).then_some((root, index))
}

// Implements [CONFIGEDITOR-OPERATIONS]: the inventory counts live diagnostics
// per code with distinct-file attribution inside the requested root only.
#[test]
fn inventory_counts_diagnostics_by_code_and_file() {
    let Some((root, index)) = indexed_root("inventory") else {
        unreachable!("indexed fixture must produce diagnostics");
    };
    let counted = inventory(&index, &root);
    assert_eq!(counted.counts.get("BSK-E0001"), Some(&2));
    assert_eq!(counted.files.get("BSK-E0001"), Some(&1));
    assert_eq!(counted.errors, 2);
    assert_eq!(counted.warnings, 0);
    assert_eq!(counted.total, 2);
    let elsewhere = inventory(&index, &root.join("unrelated"));
    assert_eq!(elsewhere.total, 0);
    let _ = std::fs::remove_dir_all(root);
}

// Implements [CONFIGEDITOR-OPERATIONS]: the snapshot merges the live catalog,
// configured severities, diagnostic debt, and fix classification per rule.
#[test]
fn snapshot_reports_rule_states_debt_and_source_metadata() {
    let Some((root, index)) = indexed_root("snapshot") else {
        unreachable!("indexed fixture must produce diagnostics");
    };
    let document = basilisk_config::discover_config_document(&root);
    let Ok(document) = document else {
        unreachable!("fixture pyproject.toml must parse");
    };
    let snapshot = build_snapshot(&index, &root, &document);
    assert_eq!(snapshot.revision, document.revision);
    assert_eq!(snapshot.source.format, ConfigurationFormat::PyprojectToml);
    assert!(snapshot.source.exists);
    assert!(!snapshot.source.read_only);
    assert!(snapshot.source.uri.ends_with("pyproject.toml"));
    assert_eq!(snapshot.rules.len(), descriptors().len());
    let annotation = snapshot
        .rules
        .iter()
        .find(|state| state.descriptor.code == "BSK-E0001");
    let Some(annotation) = annotation else {
        unreachable!("BSK-E0001 must be present in the snapshot");
    };
    assert_eq!(annotation.configured_severity, Some(RuleSeverity::Error));
    assert_eq!(annotation.effective_severity, RuleSeverity::Error);
    assert!(!annotation.inherited);
    assert_eq!(annotation.diagnostic_count, 2);
    assert_eq!(annotation.affected_file_count, 1);
    assert_eq!(annotation.safe_fix_count, 2);
    assert_eq!(annotation.unsafe_fix_count, 0);
    assert_eq!(snapshot.debt.remaining_diagnostics, 2);
    assert_eq!(snapshot.debt.adopted_files, 0);
    assert!(snapshot.debt.disabled_rules > 0);
    assert!(!snapshot.tags.is_empty());
    assert_eq!(snapshot.presets.len(), 3);
    assert!(snapshot.problems.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

// Implements [CONFIGEDITOR-OPERATIONS]: occurrence pages are stable, carry the
// fix-safety badge, and resume exactly where the previous cursor stopped.
#[test]
fn occurrences_page_stably_with_fix_safety_badges() {
    let Some((root, index)) = indexed_root("occurrences") else {
        unreachable!("indexed fixture must produce diagnostics");
    };
    let selected: HashSet<String> = std::iter::once("BSK-E0001".to_owned()).collect();
    let source_uri = "file:///workspace/pyproject.toml";
    let first = occurrences(&index, &root, source_uri, &selected, None, 1);
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.next_cursor.as_deref(), Some("1"));
    let Some(item) = first.items.first() else {
        unreachable!("first page must hold one occurrence");
    };
    assert_eq!(item.rule_code, "BSK-E0001");
    assert!(item.uri.ends_with("app.py"));
    assert_eq!(item.effective_severity, RuleSeverity::Error);
    assert_eq!(item.fix_safety, Some(FixSafety::Safe));
    assert_eq!(item.configuration_source, source_uri);
    assert_eq!(item.range.start.line, 0);

    let second = occurrences(
        &index,
        &root,
        source_uri,
        &selected,
        first.next_cursor.as_deref(),
        10,
    );
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.next_cursor, None);
    assert_eq!(
        second.items.first().map(|entry| entry.range.start.line),
        Some(3)
    );
    let _ = std::fs::remove_dir_all(root);
}
