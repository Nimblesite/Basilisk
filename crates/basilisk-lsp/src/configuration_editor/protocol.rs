//! Typed v2 custom-request handlers and revision-checked apply transaction.
//!
//! Implements [LSPARCH-CONFIG-EDITOR-PROTOCOL] / [CONFIGEDITOR-OPERATIONS].

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use basilisk_config::{build_configuration_patch, ConfigDocument, ConfigDocumentError};
use serde::Deserialize;
use tower_lsp::jsonrpc::{Error, ErrorCode, Result as LspResult};
use tower_lsp::lsp_types::Url;

use super::catalog::{descriptors, expand_selector};
use super::model::{
    ApplyConfigurationRequest, ConfigurationPreview, ConfigurationSnapshot, EditorMutation,
    PreviewConfigurationRequest, RuleOccurrencesRequest, RuleOccurrencesResponse, TypeshedAction,
    TypeshedActionRequest, TypeshedActionResult, TypeshedLicenseDocument, TypeshedSettingKey,
    TypeshedSettingValue,
};
use super::mutation::{
    build_impact, build_update, require_mutations, require_no_pep_disable, require_revision,
    require_valid_typeshed_configuration, resolved_changes, resolved_typeshed_changes,
    selection_error, validate_document_rules,
};
use super::snapshot::{
    build_snapshot, hypothetical_inventory, inventory, occurrences as build_occurrences,
};
use super::state::PreparedPreview;
use super::transaction::apply_prepared_patch;
use crate::server::LspServer;

/// Root-only request used by `basilisk/configurationSnapshot`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConfigurationSnapshotRequest {
    root_uri: String,
}

impl LspServer {
    /// Handle `basilisk/configurationSnapshot`.
    pub(crate) async fn configuration_snapshot(
        &self,
        request: ConfigurationSnapshotRequest,
    ) -> LspResult<ConfigurationSnapshot> {
        let root = resolve_root(self, &request.root_uri).await?;
        snapshot_for_root(self, &root).await
    }

    /// Handle `basilisk/previewConfigurationChange` without any side effects.
    pub(crate) async fn preview_configuration_change(
        &self,
        request: PreviewConfigurationRequest,
    ) -> LspResult<ConfigurationPreview> {
        let root = resolve_root(self, &request.root_uri).await?;
        let document = self
            .configuration_editor
            .effective_document(&root)
            .map_err(config_error)?;
        validate_document_rules(&document)?;
        require_revision(&document, &request.base_revision)?;
        require_mutations(&request.mutations)?;
        let catalog = descriptors();
        let update = build_update(&request.mutations, &catalog)?;
        let patch = build_configuration_patch(&document, &update).map_err(config_error)?;
        require_no_pep_disable(&patch.config)?;
        require_valid_typeshed_configuration(&patch.config)?;
        let disk_revision = self
            .configuration_editor
            .disk_revision_for(&root, &document.revision);
        let guard = self.index.read().await;
        let index = guard
            .as_ref()
            .ok_or_else(|| rpc_error("invalidMutation", "workspace index is not ready"))?;
        let _ = index.preload_root_for_configuration(&root);
        let before = inventory(index, &root);
        let after = hypothetical_inventory(index, &root, &patch.config);
        drop(guard);
        let changes = resolved_changes(&catalog, &document.config, &patch.config);
        let typeshed_changes = resolved_typeshed_changes(&document.config, &patch.config);
        let impact = build_impact(&before, &after);
        let prepared = PreparedPreview {
            root,
            patch,
            disk_revision,
        };
        let preview_id = self.configuration_editor.insert(prepared);
        Ok(ConfigurationPreview {
            preview_id,
            base_revision: request.base_revision,
            changes,
            typeshed_changes,
            impact,
        })
    }

    /// Handle `basilisk/applyConfigurationChange`.
    ///
    /// The preview pins its base revision ([CONFIGEDITOR-MODEL]); apply
    /// rejects it when the current document has moved past that revision.
    pub(crate) async fn apply_configuration_change(
        &self,
        request: ApplyConfigurationRequest,
    ) -> LspResult<ConfigurationSnapshot> {
        let root = resolve_root(self, &request.root_uri).await?;
        let prepared = self
            .configuration_editor
            .take(&request.preview_id)
            .ok_or_else(|| {
                rpc_error(
                    "previewExpired",
                    "configuration preview is unknown or expired",
                )
            })?;
        if prepared.root != root {
            return Err(rpc_error(
                "invalidMutation",
                "preview belongs to a different workspace root",
            ));
        }
        let current = self
            .configuration_editor
            .effective_document_with_version(&root)
            .map_err(config_error)?;
        require_revision(&current.document, &prepared.patch.base_revision)?;
        let applied = apply_prepared_patch(
            self,
            &root,
            &current.document,
            &prepared.patch,
            prepared.disk_revision,
            current.version,
            "apiApply",
        )
        .await?;
        snapshot_with_document(self, &root, &applied).await
    }

    /// Handle `basilisk/ruleOccurrences`.
    pub(crate) async fn rule_occurrences(
        &self,
        request: RuleOccurrencesRequest,
    ) -> LspResult<RuleOccurrencesResponse> {
        let root = resolve_root(self, &request.root_uri).await?;
        let document = self
            .configuration_editor
            .effective_document(&root)
            .map_err(config_error)?;
        validate_document_rules(&document)?;
        let guard = self.index.read().await;
        let index = guard
            .as_ref()
            .ok_or_else(|| rpc_error("invalidMutation", "workspace index is not ready"))?;
        let _ = index.preload_root_for_configuration(&root);
        let catalog = descriptors();
        let codes = expand_selector(&request.selector, &catalog).map_err(selection_error)?;
        let selected: HashSet<String> = codes.into_iter().collect();
        let limit = usize::try_from(request.limit).map_err(|_conversion_error| {
            rpc_error(
                "invalidMutation",
                "occurrence limit must be between 1 and 1000",
            )
        })?;
        if !(1..=1_000).contains(&limit) {
            return Err(rpc_error(
                "invalidMutation",
                "occurrence limit must be between 1 and 1000",
            ));
        }
        Ok(build_occurrences(
            index,
            &root,
            &selected,
            request.cursor.as_deref(),
            limit,
        ))
    }

    /// Handle the closed Typeshed action union ([LSPCFGED-TYPESHED]).
    pub(crate) async fn typeshed_action(
        &self,
        request: TypeshedActionRequest,
    ) -> LspResult<TypeshedActionResult> {
        let root = resolve_root(self, &request.root_uri).await?;
        let document = self
            .configuration_editor
            .effective_document(&root)
            .map_err(config_error)?;
        require_revision(&document, &request.base_revision)?;
        match request.action {
            TypeshedAction::PinCurrent => self.pin_current_action(&root, &document, request).await,
            TypeshedAction::AcquireFresh => self.acquire_fresh_action(&root, &document).await,
            TypeshedAction::ViewLicense => self.view_license_action(&root, &document).await,
        }
    }

    async fn pin_current_action(
        &self,
        root: &Path,
        document: &ConfigDocument,
        request: TypeshedActionRequest,
    ) -> LspResult<TypeshedActionResult> {
        if document.config.typeshed_path.is_some() {
            return Err(rpc_error(
                "typeshedActionUnavailable",
                "a custom Typeshed folder has no upstream commit to pin",
            ));
        }
        let commit = self
            .typeshed_generations
            .read()
            .await
            .get(root)
            .and_then(crate::server::typeshed_status::TypeshedGeneration::ready_snapshot)
            .filter(|snapshot| snapshot_matches_document(snapshot, document))
            .and_then(|snapshot| snapshot.status.commit)
            .ok_or_else(|| {
                rpc_error(
                    "typeshedActionUnavailable",
                    "the active Typeshed source has no commit to pin",
                )
            })?
            .to_hex();
        let preview = self
            .preview_configuration_change(PreviewConfigurationRequest {
                root_uri: request.root_uri,
                base_revision: request.base_revision,
                mutations: vec![
                    EditorMutation::SetTypeshedSetting {
                        key: TypeshedSettingKey::TypeshedCommit,
                        value: TypeshedSettingValue::Text { value: commit },
                    },
                    EditorMutation::RemoveTypeshedSetting {
                        key: TypeshedSettingKey::TypeshedPath,
                    },
                ],
            })
            .await?;
        Ok(TypeshedActionResult::Preview { preview })
    }

    async fn acquire_fresh_action(
        &self,
        root: &Path,
        document: &ConfigDocument,
    ) -> LspResult<TypeshedActionResult> {
        let staged = match super::typeshed_acquisition::acquire_fresh(self, root, &document.config)
            .await
        {
            Ok(staged) => staged,
            Err(error) => {
                let rpc_error = error.rpc_error();
                let Some(failure) = error.into_failure() else {
                    return Err(rpc_error);
                };
                let handles = self.refresh_handles();
                let refresh = super::transaction::refresh_with_document(
                    &handles,
                    root,
                    "typeshedAcquireFreshFailed",
                    document,
                )
                .await;
                super::typeshed_acquisition::publish_failure(&handles, root, failure).await;
                if let Err(error) = refresh {
                    tracing::warn!(root = %root.display(), error = %error, "failed to clear analysis after Typeshed refresh failure");
                }
                return Err(rpc_error);
            }
        };
        let handles = self.refresh_handles();
        let refresh = super::transaction::refresh_with_document_and_typeshed(
            &handles,
            root,
            "typeshedAcquireFresh",
            document,
            Some(staged.candidate()),
        )
        .await;
        if let Err(error) = refresh {
            let cleanup = super::transaction::refresh_with_document(
                &handles,
                root,
                "typeshedAcquireFreshBlocked",
                document,
            )
            .await;
            staged
                .block(self, root, "configuration refresh failed")
                .await;
            if let Err(cleanup_error) = cleanup {
                tracing::warn!(root = %root.display(), error = %cleanup_error, "failed to clear analysis after Typeshed activation failure");
            }
            return Err(error);
        }
        staged.activate(self, root).await;
        Ok(TypeshedActionResult::Snapshot {
            snapshot: snapshot_with_document(self, root, document).await?,
        })
    }

    async fn view_license_action(
        &self,
        root: &Path,
        document: &ConfigDocument,
    ) -> LspResult<TypeshedActionResult> {
        if document.config.typeshed_path.is_some() {
            return Ok(TypeshedActionResult::License {
                license: TypeshedLicenseDocument {
                    title: "User-managed Typeshed terms — not supplied".to_owned(),
                    uri: None,
                    content: "not supplied".to_owned(),
                    read_only: true,
                },
            });
        }
        let generations = self.typeshed_generations.read().await;
        let snapshot = generations
            .get(root)
            .and_then(crate::server::typeshed_status::TypeshedGeneration::ready_snapshot)
            .filter(|snapshot| snapshot_matches_document(snapshot, document))
            .ok_or_else(|| {
                rpc_error(
                    "typeshedActionUnavailable",
                    "Typeshed acquisition for this workspace has not reached a terminal source",
                )
            })?;
        let content = snapshot
            .vfs
            .read_str("LICENSE")
            .unwrap_or("not supplied")
            .to_owned();
        Ok(TypeshedActionResult::License {
            license: TypeshedLicenseDocument {
                title: "Typeshed License".to_owned(),
                uri: snapshot.status.license_reference.clone(),
                content,
                read_only: true,
            },
        })
    }
}

fn snapshot_matches_document(
    snapshot: &basilisk_stubs::typeshed::snapshot::Snapshot,
    document: &ConfigDocument,
) -> bool {
    if document.config.typeshed_path.is_some() {
        return snapshot.status.active_source
            == basilisk_stubs::typeshed::source::SourceKind::Custom;
    }
    match document.config.typeshed_commit.as_deref() {
        Some(commit) => snapshot
            .status
            .commit
            .is_some_and(|oid| oid.to_hex() == commit),
        None => matches!(
            snapshot.status.active_source,
            basilisk_stubs::typeshed::source::SourceKind::Latest
                | basilisk_stubs::typeshed::source::SourceKind::Bundled
        ),
    }
}

async fn snapshot_for_root(server: &LspServer, root: &Path) -> LspResult<ConfigurationSnapshot> {
    let document = server
        .configuration_editor
        .effective_document(root)
        .map_err(config_error)?;
    snapshot_with_document(server, root, &document).await
}

async fn snapshot_with_document(
    server: &LspServer,
    root: &Path,
    document: &ConfigDocument,
) -> LspResult<ConfigurationSnapshot> {
    validate_document_rules(document)?;
    let typeshed_generation = server.typeshed_generations.read().await.get(root).cloned();
    let guard = server.index.read().await;
    let index = guard
        .as_ref()
        .ok_or_else(|| rpc_error("invalidMutation", "workspace index is not ready"))?;
    let _ = index.preload_root_for_configuration(root);
    Ok(build_snapshot(
        index,
        root,
        document,
        typeshed_generation.as_ref(),
    ))
}

async fn resolve_root(server: &LspServer, root_uri: &str) -> LspResult<PathBuf> {
    let uri = Url::parse(root_uri)
        .map_err(|_parse_error| rpc_error("invalidMutation", "rootUri is not a valid URI"))?;
    let path = uri
        .to_file_path()
        .map_err(|()| rpc_error("invalidMutation", "rootUri must be a file URI"))?;
    server
        .workspace_roots
        .read()
        .await
        .iter()
        .find(|root| **root == path)
        .cloned()
        .ok_or_else(|| rpc_error("invalidMutation", "rootUri is not an active workspace root"))
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "used directly as the Result::map_err adapter"
)]
pub(super) fn config_error(error: ConfigDocumentError) -> Error {
    let kind = match &error {
        ConfigDocumentError::Invalid { .. } | ConfigDocumentError::Read { .. } => {
            "invalidConfiguration"
        }
        ConfigDocumentError::RevisionConflict { .. } => "revisionConflict",
        ConfigDocumentError::ReadOnly { .. } => "readOnlySource",
    };
    let source_uri = match &error {
        ConfigDocumentError::Invalid { path, .. }
        | ConfigDocumentError::Read { path, .. }
        | ConfigDocumentError::ReadOnly { path } => Some(path_uri(path)),
        ConfigDocumentError::RevisionConflict { .. } => None,
    };
    rpc_error_data(
        kind,
        &error.to_string(),
        serde_json::json!({ "sourceUri": source_uri }),
    )
}

pub(super) fn rpc_error(kind: &str, message: &str) -> Error {
    rpc_error_data(kind, message, serde_json::json!({}))
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "the JSON-RPC error takes ownership of its structured context"
)]
pub(super) fn rpc_error_data(kind: &str, message: &str, context: serde_json::Value) -> Error {
    Error {
        code: ErrorCode::ServerError(-32020),
        message: message.to_owned().into(),
        data: Some(serde_json::json!({ "kind": kind, "context": context })),
    }
}

pub(super) fn path_uri(path: &Path) -> String {
    super::snapshot::path_uri(path)
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
