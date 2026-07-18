//! Unit tests for configuration-editor protocol guards.

use basilisk_config::{BasiliskConfig, ConfigDocument};

use super::{config_error, snapshot_matches_document, ConfigurationSnapshotRequest};

#[test]
fn invalid_configuration_error_identifies_its_repair_source() {
    let source = std::path::PathBuf::from("/workspace/pyproject.toml");
    let error = config_error(basilisk_config::ConfigDocumentError::Invalid {
        path: source,
        message: "rules must be a table".to_owned(),
    });
    assert!(error.data.is_some());
    let Some(data) = error.data else { return };
    assert_eq!(
        data.pointer("/context/sourceUri"),
        Some(&serde_json::json!("file:///workspace/pyproject.toml"))
    );
}

#[test]
fn snapshot_guards_reject_cross_mode_and_cross_commit_actions() {
    let snapshot = basilisk_stubs::typeshed::bundle::bundled_snapshot();
    assert!(
        snapshot.is_ok(),
        "release bundle must activate: {snapshot:?}"
    );
    let Ok(snapshot) = snapshot else { return };
    let mut config = BasiliskConfig::default();
    let document = |config| ConfigDocument {
        root: std::path::PathBuf::from("/workspace"),
        path: std::path::PathBuf::from("/workspace/pyproject.toml"),
        exists: true,
        read_only: false,
        content: String::new(),
        revision: "revision".to_owned(),
        config,
    };
    assert!(snapshot_matches_document(
        &snapshot,
        &document(config.clone())
    ));
    config.typeshed_path = Some(std::path::PathBuf::from("custom"));
    assert!(!snapshot_matches_document(
        &snapshot,
        &document(config.clone())
    ));
    config.typeshed_path = None;
    config.typeshed_commit = snapshot.status.commit.map(|oid| oid.to_hex());
    assert!(snapshot_matches_document(
        &snapshot,
        &document(config.clone())
    ));
    config.typeshed_commit = Some("0123456789012345678901234567890123456789".to_owned());
    assert!(!snapshot_matches_document(&snapshot, &document(config)));
}

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn latest_snapshot(
    commit: basilisk_stubs::typeshed::gittree::Oid,
) -> TestResult<basilisk_stubs::typeshed::snapshot::Snapshot> {
    use basilisk_stubs::typeshed::archive::ArchiveVfs;
    use basilisk_stubs::typeshed::snapshot::Snapshot;
    use basilisk_stubs::typeshed::source::{
        Provenance, SourceIdentity, SourceKind, StatusWarning, Transport,
    };
    use basilisk_stubs::typeshed::warning::{TypeshedWarning, UnpinnedKind};

    let bundled = basilisk_stubs::typeshed::bundle::bundled_snapshot()?;
    let archive = bundled.vfs.archive().clone();
    let tree = archive.root_tree_oid()?;
    let identity = SourceIdentity::Commit {
        commit,
        pinned: false,
    };
    let mut status = bundled.status.clone();
    status.active_source = SourceKind::Latest;
    status.commit = Some(commit);
    status.tree = Some(tree);
    status.transport = Transport::Codeload;
    status.provenance = Provenance::GithubTlsAttested;
    status.license_reference = Some(format!(
        "https://github.com/python/typeshed/blob/{}/LICENSE",
        commit.to_hex()
    ));
    status.warnings =
        StatusWarning::list(&[TypeshedWarning::Unpinned(UnpinnedKind::LatestOrBundled)]);
    let identity_key = identity.uri_component();
    Ok(Snapshot::build(
        identity,
        status,
        ArchiveVfs::new(identity_key, archive),
        None,
    )?)
}

async fn initialized_test_service() -> TestResult<(
    tower_lsp::LspService<crate::server::LspServer>,
    tokio::sync::mpsc::UnboundedReceiver<serde_json::Value>,
    tokio::task::JoinHandle<()>,
)> {
    use futures_util::{SinkExt as _, StreamExt as _};
    use tower_service::Service as _;

    let (mut service, socket) = tower_lsp::LspService::new(crate::server::LspServer::new);
    let (mut requests, mut responses) = socket.split();
    let (apply_tx, apply_rx) = tokio::sync::mpsc::unbounded_channel();
    let pump = tokio::spawn(async move {
        while let Some(request) = requests.next().await {
            let Some(id) = request.id().cloned() else {
                continue;
            };
            let result = if request.method() == "workspace/applyEdit" {
                if let Some(params) = request.params().cloned() {
                    let _ = apply_tx.send(params);
                }
                serde_json::json!({ "applied": true })
            } else {
                serde_json::Value::Null
            };
            if responses
                .send(tower_lsp::jsonrpc::Response::from_ok(id, result))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    let initialize = tower_lsp::jsonrpc::Request::build("initialize")
        .id(1_i64)
        .params(serde_json::json!({
            "processId": null,
            "rootUri": null,
            "capabilities": {},
            "trace": "off"
        }))
        .finish();
    let response = service.call(initialize).await?;
    if response
        .as_ref()
        .is_none_or(tower_lsp::jsonrpc::Response::is_error)
    {
        return Err("test LSP initialization failed".into());
    }
    let initialized = tower_lsp::jsonrpc::Request::build("initialized")
        .params(serde_json::json!({}))
        .finish();
    let _ = service.call(initialized).await?;
    if let Some(watcher) = service.inner().config_watcher.lock().await.take() {
        watcher.abort();
    }
    Ok((service, apply_rx, pump))
}

async fn pin_current_round_trip(
    snapshot: basilisk_stubs::typeshed::snapshot::Snapshot,
) -> TestResult<()> {
    use std::sync::Arc;

    use super::super::model::{
        ApplyConfigurationRequest, TypeshedAction, TypeshedActionRequest, TypeshedActionResult,
    };
    use crate::config::AnalysisMode;
    use crate::server::typeshed_status::TypeshedGeneration;
    use crate::workspace::WorkspaceIndex;

    let expected = snapshot
        .status
        .commit
        .ok_or("active snapshot has no commit")?
        .to_hex();
    let root = tempfile::tempdir()?;
    std::fs::write(root.path().join("pyproject.toml"), "[tool.basilisk]\n")?;
    let root_path = root.path().to_path_buf();
    let root_uri = tower_lsp::lsp_types::Url::from_file_path(&root_path)
        .map_err(|()| "temporary root has no file URI")?
        .to_string();
    let (service, mut apply_rx, pump) = initialized_test_service().await?;
    let server = service.inner();
    *server.workspace_roots.write().await = vec![root_path.clone()];
    *server.index.write().await = Some(WorkspaceIndex::new(
        vec![root_path.clone()],
        AnalysisMode::WholeModule,
        BasiliskConfig::default(),
    ));
    let _ = server.typeshed_generations.write().await.insert(
        root_path.clone(),
        TypeshedGeneration::Ready(Arc::new(snapshot)),
    );

    let current = server
        .configuration_snapshot(ConfigurationSnapshotRequest {
            root_uri: root_uri.clone(),
        })
        .await?;
    let action = server
        .typeshed_action(TypeshedActionRequest {
            root_uri: root_uri.clone(),
            base_revision: current.revision,
            action: TypeshedAction::PinCurrent,
        })
        .await?;
    let TypeshedActionResult::Preview { preview } = action else {
        return Err("Pin current did not return a preview".into());
    };
    let _applied = server
        .apply_configuration_change(ApplyConfigurationRequest {
            root_uri,
            preview_id: preview.preview_id,
        })
        .await?;

    let apply_edit = tokio::time::timeout(std::time::Duration::from_secs(2), apply_rx.recv())
        .await?
        .ok_or("Pin current sent no workspace edit")?;
    let edited_text = apply_edit
        .pointer("/edit/documentChanges/0/edits/0/newText")
        .and_then(serde_json::Value::as_str)
        .ok_or("Pin current workspace edit carried no replacement text")?;
    assert!(
        edited_text.contains(&format!("typeshed-commit = \"{expected}\"")),
        "workspace edit must write the active commit: {edited_text}"
    );
    let document = server.configuration_editor.effective_document(&root_path)?;
    assert_eq!(
        document.config.typeshed_commit.as_deref(),
        Some(expected.as_str())
    );
    assert!(document
        .content
        .contains(&format!("typeshed-commit = \"{expected}\"")));
    pump.abort();
    Ok(())
}

#[tokio::test]
async fn pin_current_applies_the_latest_active_commit() -> TestResult<()> {
    let commit = basilisk_stubs::typeshed::gittree::Oid::from_hex(
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    )?;
    pin_current_round_trip(latest_snapshot(commit)?).await
}

#[tokio::test]
async fn pin_current_offline_applies_the_bundled_commit() -> TestResult<()> {
    let snapshot = basilisk_stubs::typeshed::bundle::bundled_snapshot()?;
    pin_current_round_trip(snapshot).await
}
