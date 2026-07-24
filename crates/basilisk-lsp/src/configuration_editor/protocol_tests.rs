//! Unit tests for configuration-editor protocol guards and the atomic
//! Typeshed configuration transaction ([LSPCFGED-TYPESHED]).

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

type ClientMessages = tokio::sync::mpsc::UnboundedReceiver<(String, serde_json::Value)>;

/// Spin up an initialized in-process server whose client pump records every
/// client-bound message and answers `workspace/applyEdit` with `approve`.
async fn initialized_test_service(
    approve_edits: bool,
) -> TestResult<(
    tower_lsp::LspService<crate::server::LspServer>,
    ClientMessages,
    tokio::task::JoinHandle<()>,
)> {
    use futures_util::{SinkExt as _, StreamExt as _};
    use tower_service::Service as _;

    let (mut service, socket) = tower_lsp::LspService::new(crate::server::LspServer::new);
    let (mut requests, mut responses) = socket.split();
    let (message_tx, message_rx) = tokio::sync::mpsc::unbounded_channel();
    let pump = tokio::spawn(async move {
        while let Some(request) = requests.next().await {
            let method = request.method().to_owned();
            let params = request.params().cloned().unwrap_or(serde_json::Value::Null);
            let _ = message_tx.send((method.clone(), params));
            let Some(id) = request.id().cloned() else {
                continue;
            };
            let result = if method == "workspace/applyEdit" {
                serde_json::json!({ "applied": approve_edits })
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
    Ok((service, message_rx, pump))
}

/// Register one temporary workspace root with an active bundled generation.
async fn install_bundled_root(
    server: &crate::server::LspServer,
) -> TestResult<(tempfile::TempDir, String, std::path::PathBuf)> {
    use std::sync::Arc;

    use crate::config::AnalysisMode;
    use crate::server::typeshed_status::TypeshedGeneration;
    use crate::workspace::WorkspaceIndex;

    let root = tempfile::tempdir()?;
    std::fs::write(root.path().join("pyproject.toml"), "[tool.basilisk]\n")?;
    let root_path = root.path().to_path_buf();
    let root_uri = tower_lsp::lsp_types::Url::from_file_path(&root_path)
        .map_err(|()| "temporary root has no file URI")?
        .to_string();
    *server.workspace_roots.write().await = vec![root_path.clone()];
    *server.index.write().await = Some(WorkspaceIndex::new(
        vec![root_path.clone()],
        AnalysisMode::WholeModule,
        BasiliskConfig::default(),
    ));
    let snapshot = basilisk_stubs::typeshed::bundle::bundled_snapshot()?;
    let _ = server.typeshed_generations.write().await.insert(
        root_path.clone(),
        TypeshedGeneration::Ready(Arc::new(snapshot)),
    );
    Ok((root, root_uri, root_path))
}

/// Drain recorded client messages until `method` is seen or the budget ends.
async fn drain_messages_until(
    messages: &mut ClientMessages,
    method: &str,
) -> Vec<(String, serde_json::Value)> {
    let mut seen = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, messages.recv()).await {
            Ok(Some(message)) => {
                let matched = message.0 == method;
                seen.push(message);
                if matched {
                    return seen;
                }
            }
            Ok(None) | Err(_) => return seen,
        }
    }
}

fn status_lifecycles(messages: &[(String, serde_json::Value)]) -> Vec<String> {
    messages
        .iter()
        .filter(|(method, _)| {
            method == basilisk_common::configuration_editor::TYPESHED_STATUS_CHANGED
        })
        .filter_map(|(_, params)| params.pointer("/status/lifecycle/kind"))
        .filter_map(serde_json::Value::as_str)
        .map(str::to_owned)
        .collect()
}

async fn preview_and_apply(
    server: &crate::server::LspServer,
    root_uri: &str,
    mutations: Vec<super::super::model::EditorMutation>,
) -> tower_lsp::jsonrpc::Result<super::super::model::ConfigurationSnapshot> {
    use super::super::model::{ApplyConfigurationRequest, PreviewConfigurationRequest};

    let current = server
        .configuration_snapshot(ConfigurationSnapshotRequest {
            root_uri: root_uri.to_owned(),
        })
        .await?;
    let preview = server
        .preview_configuration_change(PreviewConfigurationRequest {
            root_uri: root_uri.to_owned(),
            base_revision: current.revision,
            mutations,
        })
        .await?;
    server
        .apply_configuration_change(ApplyConfigurationRequest {
            root_uri: root_uri.to_owned(),
            preview_id: preview.preview_id,
        })
        .await
}

/// [LSPCFGED-TYPESHED] — the UI-glitch proof: setting the pin lands the edit
/// and publishes ONLY the terminal Ready status. No acquiring, blocked, or
/// downloading state ever reaches the wire on a configuration change.
#[tokio::test]
async fn pin_edit_applies_atomically_without_intermediate_states() -> TestResult<()> {
    use super::super::model::{EditorMutation, TypeshedLifecycle, TypeshedSettingKey};

    let expected = basilisk_stubs::typeshed::bundle::bundled_commit_sha();
    let (service, mut messages, pump) = initialized_test_service(true).await?;
    let server = service.inner();
    let (_root, root_uri, root_path) = install_bundled_root(server).await?;

    let applied = preview_and_apply(
        server,
        &root_uri,
        vec![EditorMutation::SetTypeshedSetting {
            key: TypeshedSettingKey::TypeshedCommit,
            value: expected.to_owned(),
        }],
    )
    .await?;
    assert_eq!(applied.typeshed.status.lifecycle, TypeshedLifecycle::Ready);

    let seen = drain_messages_until(
        &mut messages,
        basilisk_common::configuration_editor::TYPESHED_STATUS_CHANGED,
    )
    .await;
    let edit_text = seen
        .iter()
        .find(|(method, _)| method == "workspace/applyEdit")
        .and_then(|(_, params)| params.pointer("/edit/documentChanges/0/edits/0/newText"))
        .and_then(serde_json::Value::as_str)
        .ok_or("pin apply sent no workspace edit")?;
    assert!(
        edit_text.contains(&format!("typeshed-commit = \"{expected}\"")),
        "workspace edit must write the pin: {edit_text}"
    );
    let lifecycles = status_lifecycles(&seen);
    assert_eq!(
        lifecycles,
        vec!["Ready".to_owned()],
        "one terminal publish, nothing intermediate"
    );
    let document = server.configuration_editor.effective_document(&root_path)?;
    assert_eq!(document.config.typeshed_commit.as_deref(), Some(expected));
    pump.abort();
    Ok(())
}

/// [STUBRES-TYPESHED-PIN]: a valid pin that is not on this machine is VALID
/// configuration — the edit lands, nothing downloads, and the root publishes
/// the terminal NO SOURCE state naming the recovery commands.
#[tokio::test]
async fn missing_pin_tanks_to_terminal_no_source_without_downloading() -> TestResult<()> {
    use super::super::model::{EditorMutation, TypeshedLifecycle, TypeshedSettingKey};

    let missing = "0123456789012345678901234567890123456789";
    let empty_store = tempfile::tempdir()?;
    let (service, mut messages, pump) = initialized_test_service(true).await?;
    let server = service.inner();
    let (_root, root_uri, root_path) = install_bundled_root(server).await?;

    let applied = preview_and_apply(
        server,
        &root_uri,
        vec![
            EditorMutation::SetTypeshedSetting {
                key: TypeshedSettingKey::TypeshedCommit,
                value: missing.to_owned(),
            },
            EditorMutation::SetTypeshedSetting {
                key: TypeshedSettingKey::TypeshedStorePath,
                value: empty_store.path().to_string_lossy().into_owned(),
            },
        ],
    )
    .await?;
    assert_eq!(
        applied.typeshed.status.lifecycle,
        TypeshedLifecycle::NoSource
    );

    let seen = drain_messages_until(
        &mut messages,
        basilisk_common::configuration_editor::TYPESHED_STATUS_CHANGED,
    )
    .await;
    let lifecycles = status_lifecycles(&seen);
    assert_eq!(
        lifecycles,
        vec!["NoSource".to_owned()],
        "a missing pin publishes exactly one terminal state and never Downloading"
    );
    let reason = seen
        .iter()
        .rev()
        .find(|(method, _)| {
            method == basilisk_common::configuration_editor::TYPESHED_STATUS_CHANGED
        })
        .and_then(|(_, params)| params.pointer("/status/noSourceReason"))
        .and_then(serde_json::Value::as_str)
        .ok_or("NoSource status carried no reason")?;
    assert!(reason.contains("NO SOURCE"), "loud reason: {reason}");
    assert!(reason.contains(missing), "reason names the pin: {reason}");
    assert!(
        reason.contains("basilisk typeshed download"),
        "reason names the recovery command: {reason}"
    );
    // The store stayed byte-empty: nothing attempted a download.
    assert_eq!(
        std::fs::read_dir(empty_store.path())?.count(),
        0,
        "resolution must never write or fetch"
    );
    let document = server.configuration_editor.effective_document(&root_path)?;
    assert_eq!(document.config.typeshed_commit.as_deref(), Some(missing));
    pump.abort();
    Ok(())
}

/// A rejected client edit publishes NOTHING: the staged generation is
/// dropped, the previous source keeps serving, and no status flickers.
#[tokio::test]
async fn rejected_edit_publishes_no_status_change() -> TestResult<()> {
    use super::super::model::{EditorMutation, TypeshedSettingKey};
    use crate::server::typeshed_status::TypeshedGeneration;

    let (service, mut messages, pump) = initialized_test_service(false).await?;
    let server = service.inner();
    let (_root, root_uri, root_path) = install_bundled_root(server).await?;

    let result = preview_and_apply(
        server,
        &root_uri,
        vec![EditorMutation::SetTypeshedSetting {
            key: TypeshedSettingKey::TypeshedCommit,
            value: "0123456789012345678901234567890123456789".to_owned(),
        }],
    )
    .await;
    assert!(result.is_err(), "a rejected edit must fail the apply");

    let seen = drain_messages_until(&mut messages, "workspace/applyEdit").await;
    assert!(
        status_lifecycles(&seen).is_empty(),
        "no status may be published for an unapplied edit"
    );
    let generation = server
        .typeshed_generations
        .read()
        .await
        .get(&root_path)
        .cloned();
    assert!(
        generation
            .as_ref()
            .and_then(TypeshedGeneration::ready_snapshot)
            .is_some(),
        "the previous Ready generation must survive a rejected edit"
    );
    pump.abort();
    Ok(())
}

/// [LSPCFGED-TYPESHED]: `ViewLicense` returns the active immutable license
/// document for the effective bundled pin.
#[tokio::test]
async fn view_license_returns_the_active_immutable_license() -> TestResult<()> {
    use super::super::model::{TypeshedAction, TypeshedActionRequest, TypeshedActionResult};

    let (service, mut messages, pump) = initialized_test_service(true).await?;
    let server = service.inner();
    let (_root, root_uri, _root_path) = install_bundled_root(server).await?;

    let current = server
        .configuration_snapshot(ConfigurationSnapshotRequest {
            root_uri: root_uri.clone(),
        })
        .await?;
    let action = server
        .typeshed_action(TypeshedActionRequest {
            root_uri,
            base_revision: current.revision,
            action: TypeshedAction::ViewLicense,
        })
        .await?;
    let TypeshedActionResult::License { license } = action else {
        return Err("ViewLicense did not return a license document".into());
    };
    assert_eq!(license.title, "Typeshed License");
    assert!(license.read_only);
    assert!(
        !license.content.trim().is_empty(),
        "bundled license text must be present"
    );
    messages.close();
    pump.abort();
    Ok(())
}
