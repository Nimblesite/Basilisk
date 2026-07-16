//! Workspace-edit apply and deterministic configuration refresh tail.

use std::path::Path;
use std::sync::Arc;

use basilisk_config::{build_rule_patch, ConfigDocument, ConfigPatch, RuleConfigUpdate};
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    CreateFile, CreateFileOptions, DocumentChangeOperation, DocumentChanges, OneOf,
    OptionalVersionedTextDocumentIdentifier, Position, Range, ResourceOp, TextDocumentEdit,
    TextEdit, Url, WorkspaceEdit,
};

use super::model::ConfigurationChanged;
use super::protocol::{config_error, path_uri, rpc_error, rpc_error_data};
use super::state::ConfigurationEditorState;
use crate::server::LspServer;

/// Cloneable server handles for the shared configuration refresh tail, so the
/// server-owned configuration watcher ([LSPARCH-CONFIG]) and request
/// handlers run the exact same reload → recheck → republish → notify sequence.
#[derive(Clone)]
pub(crate) struct ConfigurationRefreshHandles {
    /// The workspace index holding per-root checker configuration.
    pub(crate) index: Arc<tokio::sync::RwLock<Option<crate::workspace::WorkspaceIndex>>>,
    /// LSP client for publishing diagnostics and notifications.
    pub(crate) client: tower_lsp::Client,
    /// The `basilisk.enabled` toggle gate ([ANALYSIS-ENABLED]).
    pub(crate) type_checking_enabled: Arc<tokio::sync::RwLock<bool>>,
    /// The analyze-scope publication gate ([LSPARCH-DIAGNOSTIC-SCOPE]).
    pub(crate) analyze_enabled: Arc<std::sync::atomic::AtomicBool>,
    /// Open-buffer overlays and disk baselines for configuration sources.
    pub(crate) configuration_editor: Arc<ConfigurationEditorState>,
}

pub(crate) fn configuration_document(server: &LspServer, root: &Path) -> LspResult<ConfigDocument> {
    server
        .configuration_editor
        .effective_document(root)
        .map_err(config_error)
}

fn applied_document(document: &ConfigDocument, patch: &ConfigPatch) -> ConfigDocument {
    ConfigDocument {
        root: document.root.clone(),
        path: patch.path.clone(),
        exists: true,
        read_only: false,
        content: patch.content.clone(),
        revision: patch.revision.clone(),
        config: patch.config.clone(),
    }
}

fn replacement_edit(
    document: &ConfigDocument,
    patch: &ConfigPatch,
    document_version: Option<i32>,
) -> LspResult<WorkspaceEdit> {
    let uri = Url::from_file_path(&patch.path).map_err(|()| {
        rpc_error(
            "invalidMutation",
            "configuration path cannot be represented as a URI",
        )
    })?;
    let end = crate::util::byte_offset_to_position(&document.content, document.content.len());
    let text_edit = TextDocumentEdit {
        text_document: OptionalVersionedTextDocumentIdentifier {
            uri: uri.clone(),
            version: document_version,
        },
        edits: vec![OneOf::Left(TextEdit {
            range: Range {
                start: Position::default(),
                end,
            },
            new_text: patch.content.clone(),
        })],
    };
    let mut operations = Vec::new();
    if !document.exists {
        operations.push(DocumentChangeOperation::Op(ResourceOp::Create(
            CreateFile {
                uri,
                options: Some(CreateFileOptions {
                    overwrite: Some(false),
                    ignore_if_exists: Some(false),
                }),
                annotation_id: None,
            },
        )));
    }
    operations.push(DocumentChangeOperation::Edit(text_edit));
    Ok(WorkspaceEdit {
        changes: None,
        document_changes: Some(DocumentChanges::Operations(operations)),
        change_annotations: None,
    })
}

/// Apply one validated entry update through the same client-edit service
/// used by the typed preview/apply protocol.
pub(crate) async fn apply_rule_updates(
    server: &LspServer,
    root: &Path,
    update: &RuleConfigUpdate,
    reason: &str,
) -> LspResult<ConfigDocument> {
    let effective = server
        .configuration_editor
        .effective_document_with_version(root)
        .map_err(config_error)?;
    let document = effective.document;
    let disk_revision = server
        .configuration_editor
        .disk_revision_for(root, &document.revision);
    let patch = build_rule_patch(&document, update).map_err(config_error)?;
    apply_prepared_patch(
        server,
        root,
        &document,
        &patch,
        disk_revision,
        effective.version,
        reason,
    )
    .await
}

pub(super) async fn apply_prepared_patch(
    server: &LspServer,
    root: &Path,
    document: &ConfigDocument,
    patch: &ConfigPatch,
    disk_revision: String,
    document_version: Option<i32>,
    reason: &str,
) -> LspResult<ConfigDocument> {
    let edit = replacement_edit(document, patch, document_version)?;
    let response = server.client.apply_edit(edit).await.map_err(|error| {
        rpc_error_data(
            "clientRejectedEdit",
            "client failed to apply configuration edit",
            serde_json::json!({ "error": error.to_string() }),
        )
    })?;
    if !response.applied {
        return Err(rpc_error_data(
            "clientRejectedEdit",
            "client rejected configuration edit",
            serde_json::json!({ "reason": response.failure_reason }),
        ));
    }
    let applied = applied_document(document, patch);
    if let Some(version) = document_version {
        server.configuration_editor.remember_open_edit(
            patch.path.clone(),
            version,
            applied.clone(),
        );
    } else {
        server.configuration_editor.remember_applied(
            root.to_path_buf(),
            disk_revision,
            applied.clone(),
        );
    }
    refresh_with_document(&server.refresh_handles(), root, reason, &applied).await?;
    Ok(applied)
}

/// Reload, recheck, publish, and notify after an API or external config edit.
pub(crate) async fn refresh_after_configuration_change(
    server: &LspServer,
    root: &Path,
    reason: &str,
) -> LspResult<()> {
    refresh_after_configuration_change_with(&server.refresh_handles(), root, reason).await
}

/// Handle-based variant of [`refresh_after_configuration_change`] for spawned
/// tasks (the server-owned configuration watcher, [LSPARCH-CONFIG]).
pub(crate) async fn refresh_after_configuration_change_with(
    handles: &ConfigurationRefreshHandles,
    root: &Path,
    reason: &str,
) -> LspResult<()> {
    let document = handles
        .configuration_editor
        .effective_document(root)
        .map_err(config_error)?;
    refresh_with_document(handles, root, reason, &document).await
}

pub(super) async fn refresh_with_document(
    handles: &ConfigurationRefreshHandles,
    root: &Path,
    reason: &str,
    document: &ConfigDocument,
) -> LspResult<()> {
    let results = {
        let mut guard = handles.index.write().await;
        let index = guard
            .as_mut()
            .ok_or_else(|| rpc_error("invalidMutation", "workspace index is not ready"))?;
        index.set_root_config(root.to_path_buf(), document.config.clone());
        let mut results = index.reresolve_imports_and_recheck();
        if index.mode() == crate::config::AnalysisMode::OpenFilesOnly {
            results.retain(|(uri, _)| configuration_result_is_publishable(index, uri));
        }
        results
    };
    for (uri, diagnostics) in results {
        crate::server::publish_diagnostics_gated(
            &handles.client,
            &handles.type_checking_enabled,
            &handles.analyze_enabled,
            uri,
            diagnostics,
        )
        .await;
    }
    tracing::info!(
        root = %root.display(),
        revision = %document.revision,
        reason,
        "configuration refresh complete"
    );
    handles
        .client
        .send_notification::<ConfigurationChangedNotification>(ConfigurationChanged {
            root_uri: path_uri(root),
            revision: document.revision.clone(),
        })
        .await;
    Ok(())
}

fn configuration_result_is_publishable(
    index: &crate::workspace::WorkspaceIndex,
    uri: &Url,
) -> bool {
    index.mode() != crate::config::AnalysisMode::OpenFilesOnly
        || uri
            .to_file_path()
            .ok()
            .and_then(|path| index.files.get(&path).map(|entry| entry.is_open))
            .unwrap_or(false)
}

pub(crate) struct ConfigurationChangedNotification;

impl tower_lsp::lsp_types::notification::Notification for ConfigurationChangedNotification {
    type Params = ConfigurationChanged;
    const METHOD: &'static str = basilisk_common::configuration_editor::CHANGED;
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use basilisk_config::{BasiliskConfig, ConfigDocument, ConfigPatch};
    use tower_lsp::lsp_types::{DocumentChangeOperation, DocumentChanges, Url};

    use super::{configuration_result_is_publishable, replacement_edit};
    use crate::config::AnalysisMode;
    use crate::workspace::WorkspaceIndex;

    #[test]
    fn open_files_only_never_publishes_preloaded_closed_file_diagnostics() {
        let index = WorkspaceIndex::new(
            Vec::new(),
            AnalysisMode::OpenFilesOnly,
            BasiliskConfig::default(),
        );
        let uri = Url::parse("file:///workspace/source.py");
        assert!(uri.is_ok());
        let Ok(uri) = uri else {
            return;
        };
        let _ = index.set_open(&uri, "value: int = 'wrong'\n", 1);
        assert!(configuration_result_is_publishable(&index, &uri));
        if let Ok(path) = uri.to_file_path() {
            if let Some(mut entry) = index.files.get_mut(&path) {
                entry.is_open = false;
            }
        }
        assert!(!configuration_result_is_publishable(&index, &uri));
        index.set_mode(AnalysisMode::WholeModule);
        assert!(configuration_result_is_publishable(&index, &uri));
    }

    #[test]
    fn open_configuration_replacement_is_versioned() {
        let source_path = PathBuf::from("/workspace/pyproject.toml");
        let document = ConfigDocument {
            root: PathBuf::from("/workspace"),
            path: source_path.clone(),
            exists: true,
            read_only: false,
            content: "[project]\n".to_owned(),
            revision: "before".to_owned(),
            config: BasiliskConfig::default(),
        };
        let rendered = ConfigPatch {
            path: source_path,
            base_revision: "before".to_owned(),
            content: "[project]\nname = \"demo\"\n".to_owned(),
            revision: "after".to_owned(),
            config: BasiliskConfig::default(),
        };

        let edit = replacement_edit(&document, &rendered, Some(42));
        assert!(edit.is_ok());
        let Ok(edit) = edit else { return };
        let version = match edit.document_changes {
            Some(DocumentChanges::Operations(operations)) => {
                operations
                    .into_iter()
                    .find_map(|operation| match operation {
                        DocumentChangeOperation::Edit(replacement) => {
                            replacement.text_document.version
                        }
                        DocumentChangeOperation::Op(_) => None,
                    })
            }
            Some(DocumentChanges::Edits(edits)) => edits
                .into_iter()
                .find_map(|replacement| replacement.text_document.version),
            None => None,
        };
        assert_eq!(version, Some(42));
    }
}
