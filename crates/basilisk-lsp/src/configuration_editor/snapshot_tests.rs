//! Tests for [CONFIGEDITOR-OPERATIONS] / [CONFIGEDITOR-MODEL] snapshot,
//! inventory, and occurrence projections. See `snapshot.rs`.

use std::collections::HashSet;
use std::path::PathBuf;

use basilisk_config::{BasiliskConfig, RuleSeverity as ConfigSeverity};

use super::{
    build_snapshot, hypothetical_inventory, inventory, occurrences, page_occurrences,
};
use crate::config::AnalysisMode;
use crate::configuration_editor::catalog::descriptors;
use crate::configuration_editor::model::{
    RuleOccurrence, RuleSeverity, SourcePosition, SourceRange,
};
use crate::workspace::WorkspaceIndex;

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

/// [CONFIGEDITOR-OPERATIONS]: hypothetical previews respect the project's
/// exclude globs — excluded files never contribute to impact counts.
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
    let strict = BasiliskConfig::with_rule_entries(std::collections::HashMap::from([(
        "assignment_compatibility".to_owned(),
        ConfigSeverity::Error,
    )]));
    let preview = hypothetical_inventory(&index, &root, &strict);
    assert_eq!(preview.total(), 0);
    let _ = std::fs::remove_dir_all(root);
}

fn occurrence(line: i64) -> RuleOccurrence {
    RuleOccurrence {
        code: "assignment_compatibility".to_owned(),
        uri: "file:///workspace/source.py".to_owned(),
        range: SourceRange {
            start: SourcePosition { line, character: 0 },
            end: SourcePosition { line, character: 1 },
        },
        severity: RuleSeverity::Error,
    }
}

/// Temp root with `BSK-0001` opted in and one open file violating it twice.
fn indexed_root(name: &str) -> Option<(PathBuf, WorkspaceIndex)> {
    let root = std::env::temp_dir().join(format!(
        "basilisk-config-snapshot-{name}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).ok()?;
    std::fs::write(
        root.join("pyproject.toml"),
        "[tool.basilisk.rules]\n\"BSK-0001\" = \"error\"\n\n[tool.basilisk.rule-tags]\n\"suppressions\" = \"warning\"\n",
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
// per code with a severity partition inside the requested root only.
#[test]
fn inventory_counts_diagnostics_by_code_and_severity() {
    let Some((root, index)) = indexed_root("inventory") else {
        unreachable!("indexed fixture must produce diagnostics");
    };
    let counted = inventory(&index, &root);
    assert_eq!(counted.counts.get("BSK-0001"), Some(&2));
    assert_eq!(counted.errors, 2);
    assert_eq!(counted.warnings, 0);
    assert_eq!(counted.infos, 0);
    assert_eq!(counted.total(), 2);
    let elsewhere = inventory(&index, &root.join("unrelated"));
    assert_eq!(elsewhere.total(), 0);
    let _ = std::fs::remove_dir_all(root);
}

// Implements [CONFIGEDITOR-MODEL]: the snapshot is the root, the config
// document URI, a revision, rule states (entry + effective severity +
// diagnostic count), and tag states with their entries — nothing else.
#[test]
fn snapshot_reports_rule_entries_effective_severities_and_tag_entries() {
    let Some((root, index)) = indexed_root("snapshot") else {
        unreachable!("indexed fixture must produce diagnostics");
    };
    let document = basilisk_config::discover_config_document(&root);
    let Ok(document) = document else {
        unreachable!("fixture pyproject.toml must parse");
    };
    let snapshot = build_snapshot(&index, &root, &document);
    assert_eq!(snapshot.revision, document.revision);
    assert!(snapshot.config_uri.ends_with("pyproject.toml"));
    assert!(snapshot.root_uri.starts_with("file://"));
    assert_eq!(snapshot.rules.len(), descriptors().len());
    let annotation = snapshot
        .rules
        .iter()
        .find(|state| state.descriptor.code == "BSK-0001");
    let Some(annotation) = annotation else {
        unreachable!("BSK-0001 must be present in the snapshot");
    };
    assert_eq!(annotation.entry, Some(RuleSeverity::Error));
    assert_eq!(annotation.effective_severity, RuleSeverity::Error);
    assert_eq!(annotation.diagnostic_count, 2);
    let suppressions = snapshot.tags.iter().find(|tag| tag.name == "suppressions");
    let Some(suppressions) = suppressions else {
        unreachable!("suppressions tag must be present in the snapshot");
    };
    assert_eq!(suppressions.entry, Some(RuleSeverity::Warning));
    assert!(suppressions.rule_count >= 1);
    // A pep rule with no entry runs at error; an analyze rule with no entry
    // and no matching tag entry is disabled ([CHKARCH-COMMANDS]).
    let pep = snapshot
        .rules
        .iter()
        .find(|state| basilisk_checker::is_pep_rule(&state.descriptor.code));
    assert_eq!(
        pep.map(|state| state.effective_severity),
        Some(RuleSeverity::Error)
    );
    let _ = std::fs::remove_dir_all(root);
}

// Implements [CONFIGEDITOR-OPERATIONS]: occurrence pages are stable and
// resume exactly where the previous cursor stopped.
#[test]
fn occurrences_page_stably_with_cursor_resume() {
    let Some((root, index)) = indexed_root("occurrences") else {
        unreachable!("indexed fixture must produce diagnostics");
    };
    let selected: HashSet<String> = std::iter::once("BSK-0001".to_owned()).collect();
    let first = occurrences(&index, &root, &selected, None, 1);
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.next_cursor.as_deref(), Some("1"));
    let Some(item) = first.items.first() else {
        unreachable!("first page must hold one occurrence");
    };
    assert_eq!(item.code, "BSK-0001");
    assert!(item.uri.ends_with("app.py"));
    assert_eq!(item.severity, RuleSeverity::Error);
    assert_eq!(item.range.start.line, 0);

    let second = occurrences(&index, &root, &selected, first.next_cursor.as_deref(), 10);
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.next_cursor, None);
    assert_eq!(
        second.items.first().map(|entry| entry.range.start.line),
        Some(3)
    );
    let _ = std::fs::remove_dir_all(root);
}
