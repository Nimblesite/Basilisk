//! Typed v1 custom-request handlers and revision-checked apply transaction.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use basilisk_config::{build_rule_patch, ConfigDocument, ConfigDocumentError};
use serde::Deserialize;
use tower_lsp::jsonrpc::{Error, ErrorCode, Result as LspResult};
use tower_lsp::lsp_types::Url;

use super::catalog::{descriptors, expand_selector};
use super::model::{
    ApplyConfigurationRequest, ConfigurationPreview, ConfigurationSnapshot,
    PreviewConfigurationRequest, RuleOccurrencesRequest, RuleOccurrencesResponse,
};
use super::mutation::{
    build_impact, expand_mutations, require_revision, resolved_changes, selection_error,
    validate_document_rules, validate_selector,
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
        if request.mutations.is_empty() {
            return Err(rpc_error(
                "invalidMutation",
                "configuration preview requires at least one mutation",
            ));
        }
        for mutation in &request.mutations {
            validate_selector(&mutation.selector)?;
        }
        let disk_revision = self
            .configuration_editor
            .disk_revision_for(&root, &document.revision);
        let catalog = descriptors();
        let guard = self.index.read().await;
        let index = guard
            .as_ref()
            .ok_or_else(|| rpc_error("invalidMutation", "workspace index is not ready"))?;
        let _ = index.preload_root_for_configuration(&root);
        let before = inventory(index, &root);
        let (updates, expanded_rule_codes) = expand_mutations(&request, &catalog, &before.counts)?;
        let patch = build_rule_patch(&document, &updates).map_err(config_error)?;
        let changes = resolved_changes(&document, &updates);
        let after = hypothetical_inventory(index, &root, &patch.config);
        let impact = build_impact(&patch, &catalog, &changes, &before, &after);
        drop(guard);
        let prepared = PreparedPreview {
            root,
            patch,
            disk_revision,
        };
        let preview_id = self.configuration_editor.insert(prepared);
        Ok(ConfigurationPreview {
            preview_id,
            base_revision: request.base_revision,
            expanded_rule_codes,
            changes,
            impact,
            problems: Vec::new(),
        })
    }

    /// Handle `basilisk/applyConfigurationChange`.
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
        if prepared.root != root || prepared.patch.base_revision != request.base_revision {
            return Err(rpc_error(
                "invalidMutation",
                "preview does not match root and base revision",
            ));
        }
        let current = self
            .configuration_editor
            .effective_document_with_version(&root)
            .map_err(config_error)?;
        require_revision(&current.document, &request.base_revision)?;
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
        validate_selector(&request.selector)?;
        let guard = self.index.read().await;
        let index = guard
            .as_ref()
            .ok_or_else(|| rpc_error("invalidMutation", "workspace index is not ready"))?;
        let _ = index.preload_root_for_configuration(&root);
        let current = inventory(index, &root);
        let catalog = descriptors();
        let codes = expand_selector(&request.selector, &catalog, &current.counts)
            .map_err(selection_error)?;
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
            &path_uri(&document.path),
            &selected,
            request.cursor.as_deref(),
            limit,
        ))
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
    let guard = server.index.read().await;
    let index = guard
        .as_ref()
        .ok_or_else(|| rpc_error("invalidMutation", "workspace index is not ready"))?;
    let _ = index.preload_root_for_configuration(root);
    Ok(build_snapshot(index, root, document))
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
    Url::from_file_path(path).map_or_else(
        |()| path.to_string_lossy().into_owned(),
        |uri| uri.to_string(),
    )
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "isolated filesystem fixture failures should abort the test"
)]
mod tests {
    use super::config_error;

    #[test]
    fn invalid_configuration_error_identifies_its_repair_source() {
        let source = std::path::PathBuf::from("/workspace/pyproject.toml");
        let error = config_error(basilisk_config::ConfigDocumentError::Invalid {
            path: source,
            message: "rules must be a table".to_owned(),
        });
        let data = error.data.unwrap();
        assert_eq!(
            data.pointer("/context/sourceUri"),
            Some(&serde_json::json!("file:///workspace/pyproject.toml"))
        );
    }
}
