use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use basilisk_config::{
    BasiliskConfig, ConfigDocument, ConfigFormat, PathOverride, RuleSeverity as ConfigSeverity,
};

use super::{
    hypothetical_inventory, page_occurrences, path_override_states, presets, suppression_count,
};
use crate::config::AnalysisMode;
use crate::configuration_editor::catalog::descriptors;
use crate::configuration_editor::model::{
    MutationScope, RuleOccurrence, RuleSelector, RuleSetting, RuleSeverity, SourcePosition,
    SourceRange,
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
        path: PathBuf::from("/workspace/basilisk.json"),
        format: ConfigFormat::BasiliskJson,
        exists: true,
        read_only: false,
        shadowed_sources: Vec::new(),
        content: "{}".to_owned(),
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
        root.join("basilisk.json"),
        r#"{"exclude":["generated.py"]}"#,
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
    let mut strict = index.config_for_file(&excluded).clone();
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
