//! Tests for [CONFIGEDITOR-OPERATIONS] / [CONFIGEDITOR-MODEL] snapshot,
//! inventory, and occurrence projections. See `snapshot.rs`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use basilisk_config::{BasiliskConfig, RuleSeverity as ConfigSeverity};
use basilisk_stubs::typeshed::gittree::Oid;
use basilisk_stubs::typeshed::source::{
    LicenseStatus, Provenance, SourceKind, Transport, TypeshedStatus,
};
use basilisk_stubs::typeshed::warning::WarningSeverity;

use super::{build_snapshot, hypothetical_inventory, inventory, occurrences, page_occurrences};
use crate::config::AnalysisMode;
use crate::configuration_editor::catalog::descriptors;
use crate::configuration_editor::model::{
    RuleOccurrence, RuleSeverity, SourcePosition, SourceRange, TypeshedLicenseStatus,
    TypeshedLifecycle, TypeshedSource,
};
use crate::server::typeshed_status::{TypeshedFailure, TypeshedGeneration};
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
    let snapshot = build_snapshot(&index, &root, &document, None);
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

/// The settled, unpinned download the fixture root resolves to.
fn assert_downloaded_latest(snapshot: &crate::configuration_editor::model::ConfigurationSnapshot) {
    assert_eq!(snapshot.typeshed.source, TypeshedSource::Latest);
    assert_eq!(snapshot.typeshed.status.lifecycle, TypeshedLifecycle::Ready);
    assert_eq!(
        snapshot.typeshed.status.commit_identity.as_deref(),
        Some("83c2518a9e6abbda0c44592c3483de459198f887")
    );
    assert_eq!(
        snapshot
            .typeshed
            .status
            .warnings
            .first()
            .map(|warning| warning.code.as_str()),
        Some("UNPINNED")
    );
    // A downloaded source states its complete download policy — explicit
    // entries and defaults resolved, no per-control widget descriptions.
    let downloads = snapshot.typeshed.downloads.clone();
    assert_eq!(
        downloads.as_ref().map(|policy| policy.reuse_downloads),
        Some(false)
    );
    assert_eq!(
        downloads.as_ref().map(|policy| policy.verify_content),
        Some(false)
    );
    // Pinning is offered because a gate-accepted commit is active, and the
    // offer carries the exact SHA it would write.
    assert_eq!(
        snapshot.typeshed.pinnable_commit.as_deref(),
        Some("83c2518a9e6abbda0c44592c3483de459198f887")
    );
    assert!(snapshot.typeshed.license_available);
}

/// [LSPCFGED-TYPESHED]: the snapshot's Typeshed source, download policy, and
/// status come entirely from the server's parsed config plus one shared
/// terminal runtime status.
#[test]
fn snapshot_describes_typeshed_controls_and_terminal_status() {
    let Some((root, index)) = indexed_root("typeshed") else {
        unreachable!("indexed fixture must produce diagnostics");
    };
    let Ok(mut document) = basilisk_config::discover_config_document(&root) else {
        unreachable!("fixture configuration must parse");
    };
    document.config.typeshed_cache = Some(false);
    document.config.typeshed_verify = Some(false);
    let Ok(commit) = Oid::from_hex("83c2518a9e6abbda0c44592c3483de459198f887") else {
        unreachable!("fixture SHA must parse");
    };
    let status = TypeshedStatus {
        active_source: SourceKind::Bundled,
        commit: Some(commit),
        tree: None,
        transport: Transport::EmbeddedZip,
        license_status: LicenseStatus::Approved,
        license_reference: Some("typeshed://license/83c2518".to_owned()),
        provenance: Provenance::BundleVetted,
        signed_release: false,
        warnings: vec![basilisk_stubs::typeshed::source::StatusWarning {
            code: "UNPINNED".to_owned(),
            message: "UNPINNED — choose the pinned-commit source to make this reproducible"
                .to_owned(),
            severity: WarningSeverity::Advisory,
        }],
    };
    let Ok(mut runtime_snapshot) = basilisk_stubs::typeshed::bundle::bundled_snapshot() else {
        unreachable!("bundled snapshot must activate");
    };
    runtime_snapshot.status = status;
    let generation = TypeshedGeneration::Ready(Arc::new(runtime_snapshot));
    let snapshot = build_snapshot(&index, &root, &document, Some(&generation));
    assert_downloaded_latest(&snapshot);
    // A pinned commit is carried BY the active source, and an already-pinned
    // source offers no second pin.
    document.config.typeshed_commit = Some("83c2518a9e6abbda0c44592c3483de459198f887".to_owned());
    let pinned = build_snapshot(&index, &root, &document, Some(&generation));
    assert_eq!(
        pinned.typeshed.source,
        TypeshedSource::ExactCommit {
            commit: "83c2518a9e6abbda0c44592c3483de459198f887".to_owned(),
        }
    );
    assert_eq!(pinned.typeshed.pinnable_commit, None);
    assert!(
        pinned.typeshed.downloads.is_some(),
        "a pinned commit is still downloaded, so it keeps its download policy"
    );
    document.config.typeshed_commit = None;

    // A user-managed folder downloads nothing and has no upstream commit.
    document.config.typeshed_path = Some(root.join("custom-typeshed"));
    let custom = build_snapshot(&index, &root, &document, Some(&generation));
    assert_eq!(
        custom.typeshed.source,
        TypeshedSource::CustomFolder {
            path: root.join("custom-typeshed").to_string_lossy().into_owned(),
        }
    );
    assert_eq!(custom.typeshed.downloads, None);
    assert_eq!(custom.typeshed.pinnable_commit, None);
    assert!(
        custom.typeshed.license_available,
        "a custom tree still reports its user-managed terms"
    );
    document.config.typeshed_path = None;

    let blocked = TypeshedGeneration::Blocked {
        failure: TypeshedFailure::acquisition("exact commit unavailable"),
    };
    let snapshot = build_snapshot(&index, &root, &document, Some(&blocked));
    assert_eq!(
        snapshot.typeshed.status.lifecycle,
        TypeshedLifecycle::Blocked
    );
    assert_eq!(
        snapshot.typeshed.status.license_status,
        TypeshedLicenseStatus::Unavailable
    );
    assert_eq!(
        snapshot.typeshed.status.blocked_reason.as_deref(),
        Some("exact commit unavailable")
    );
    let _ = std::fs::remove_dir_all(root);
}

/// [LSPCFGED-TYPESHED]: acquisition is an atomic source transition. While a
/// candidate is being acquired, no source-policy control may start a second
/// mutation against the in-flight generation.
#[test]
fn acquiring_typeshed_offers_no_source_transition() {
    let Some((root, index)) = indexed_root("typeshed-acquiring") else {
        unreachable!("indexed fixture must produce diagnostics");
    };
    let Ok(document) = basilisk_config::discover_config_document(&root) else {
        unreachable!("fixture configuration must parse");
    };

    let snapshot = build_snapshot(
        &index,
        &root,
        &document,
        Some(&TypeshedGeneration::Acquiring),
    );

    assert_eq!(
        snapshot.typeshed.status.lifecycle,
        TypeshedLifecycle::Acquiring
    );
    assert_eq!(
        snapshot.typeshed.pinnable_commit, None,
        "an in-flight generation has no settled commit to pin"
    );
    assert!(
        !snapshot.typeshed.license_available,
        "no license document exists until the candidate settles"
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
