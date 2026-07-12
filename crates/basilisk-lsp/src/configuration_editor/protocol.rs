//! Typed v1 custom-request handlers and revision-checked apply transaction.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use basilisk_config::{
    build_rule_patch, discover_config_document, ConfigDocument, ConfigDocumentError, ConfigPatch,
    RuleConfigScope, RuleConfigUpdate,
};
use serde::Deserialize;
use tower_lsp::jsonrpc::{Error, ErrorCode, Result as LspResult};
use tower_lsp::lsp_types::{
    CreateFile, CreateFileOptions, DocumentChangeOperation, DocumentChanges, OneOf,
    OptionalVersionedTextDocumentIdentifier, Range, ResourceOp, TextDocumentEdit, TextEdit, Url,
    WorkspaceEdit,
};

use super::catalog::{descriptors, expand_selector, setting_severity, severities, SelectionError};
use super::model::{
    ApplyConfigurationRequest, ConfigurationChanged, ConfigurationImpact, ConfigurationPreview,
    ConfigurationSnapshot, MutationScope, PreviewConfigurationRequest, RuleOccurrencesRequest,
    RuleOccurrencesResponse,
};
use super::snapshot::{
    build_snapshot, hypothetical_inventory, inventory, occurrences as build_occurrences,
};
use crate::server::LspServer;

/// Root-only request used by `basilisk/configurationSnapshot`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConfigurationSnapshotRequest {
    root_uri: String,
}

/// Fully expanded and validated preview retained for apply.
#[derive(Debug, Clone)]
pub(crate) struct PreparedPreview {
    pub(crate) root: PathBuf,
    pub(crate) patch: ConfigPatch,
    pub(crate) expanded_rule_codes: Vec<String>,
}

/// Per-server optimistic preview cache.
pub(crate) struct ConfigurationEditorState {
    next_preview: AtomicU64,
    previews: Mutex<BTreeMap<String, PreparedPreview>>,
}

impl Default for ConfigurationEditorState {
    fn default() -> Self {
        Self {
            next_preview: AtomicU64::new(1),
            previews: Mutex::new(BTreeMap::new()),
        }
    }
}

impl ConfigurationEditorState {
    fn insert(&self, preview: PreparedPreview) -> String {
        let id = format!("configuration-preview-{}", self.next_preview.fetch_add(1, Ordering::Relaxed));
        let mut previews = self.previews.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = previews.insert(id.clone(), preview);
        while previews.len() > 64 {
            let _ = previews.pop_first();
        }
        id
    }

    fn get(&self, id: &str) -> Option<PreparedPreview> {
        self.previews
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(id)
            .cloned()
    }

    fn remove(&self, id: &str) {
        let _ = self
            .previews
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(id);
    }
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
        let document = discover_config_document(&root).map_err(config_error)?;
        require_revision(&document, &request.base_revision)?;
        let catalog = descriptors();
        let guard = self.index.read().await;
        let index = guard.as_ref().ok_or_else(|| rpc_error("invalidMutation", "workspace index is not ready"))?;
        let before = inventory(index, &root);
        let (updates, expanded_rule_codes) =
            expand_mutations(&request, &catalog, &before.counts)?;
        let patch = build_rule_patch(&document, &updates).map_err(config_error)?;
        let after = hypothetical_inventory(index, &root, &patch.config);
        let impact = build_impact(
            &document,
            &patch,
            &catalog,
            &expanded_rule_codes,
            &before,
            &after,
            request.run_safe_fixes,
            index,
            &root,
        );
        drop(guard);
        let prepared = PreparedPreview {
            root,
            patch,
            expanded_rule_codes: expanded_rule_codes.clone(),
        };
        let preview_id = self.configuration_editor.insert(prepared);
        Ok(ConfigurationPreview {
            preview_id,
            base_revision: request.base_revision,
            expanded_rule_codes,
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
            .get(&request.preview_id)
            .ok_or_else(|| rpc_error("previewExpired", "configuration preview is unknown or expired"))?;
        if prepared.root != root || prepared.patch.base_revision != request.base_revision {
            return Err(rpc_error("invalidMutation", "preview does not match root and base revision"));
        }
        let current = discover_config_document(&root).map_err(config_error)?;
        require_revision(&current, &request.base_revision)?;
        let edit = replacement_edit(&current, &prepared.patch)?;
        let response = self.client.apply_edit(edit).await.map_err(|error| {
            rpc_error_data("clientRejectedEdit", "client failed to apply configuration edit", serde_json::json!({
                "error": error.to_string()
            }))
        })?;
        if !response.applied {
            return Err(rpc_error_data(
                "clientRejectedEdit",
                "client rejected configuration edit",
                serde_json::json!({ "reason": response.failure_reason }),
            ));
        }
        self.configuration_editor.remove(&request.preview_id);
        refresh_after_configuration_change(self, &root, "apiApply").await?;
        snapshot_for_root(self, &root).await
    }

    /// Handle `basilisk/ruleOccurrences`.
    pub(crate) async fn rule_occurrences(
        &self,
        request: RuleOccurrencesRequest,
    ) -> LspResult<RuleOccurrencesResponse> {
        let root = resolve_root(self, &request.root_uri).await?;
        let document = discover_config_document(&root).map_err(config_error)?;
        let guard = self.index.read().await;
        let index = guard.as_ref().ok_or_else(|| rpc_error("invalidMutation", "workspace index is not ready"))?;
        let current = inventory(index, &root);
        let catalog = descriptors();
        let codes = expand_selector(&request.selector, &catalog, &current.counts).map_err(selection_error)?;
        let selected: HashSet<String> = codes.into_iter().collect();
        let limit = usize::try_from(request.limit).unwrap_or(100).clamp(1, 1_000);
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
    let document = discover_config_document(root).map_err(config_error)?;
    let guard = server.index.read().await;
    let index = guard.as_ref().ok_or_else(|| rpc_error("invalidMutation", "workspace index is not ready"))?;
    Ok(build_snapshot(index, root, &document))
}

async fn resolve_root(server: &LspServer, root_uri: &str) -> LspResult<PathBuf> {
    let uri = Url::parse(root_uri).map_err(|_| rpc_error("invalidMutation", "rootUri is not a valid URI"))?;
    let path = uri.to_file_path().map_err(|()| rpc_error("invalidMutation", "rootUri must be a file URI"))?;
    server
        .workspace_roots
        .read()
        .await
        .iter()
        .find(|root| **root == path)
        .cloned()
        .ok_or_else(|| rpc_error("invalidMutation", "rootUri is not an active workspace root"))
}

fn expand_mutations(
    request: &PreviewConfigurationRequest,
    catalog: &[super::model::RuleDescriptor],
    counts: &HashMap<String, usize>,
) -> LspResult<(Vec<RuleConfigUpdate>, Vec<String>)> {
    let by_code: HashMap<&str, &super::model::RuleDescriptor> =
        catalog.iter().map(|descriptor| (descriptor.code.as_str(), descriptor)).collect();
    let mut updates: Vec<RuleConfigUpdate> = Vec::new();
    let mut expanded = HashSet::new();
    for mutation in &request.mutations {
        let codes = expand_selector(&mutation.selector, catalog, counts).map_err(selection_error)?;
        let scope = match &mutation.scope {
            MutationScope::Project => RuleConfigScope::Project,
            MutationScope::Path { pattern } if !pattern.trim().is_empty() => RuleConfigScope::Path {
                pattern: pattern.clone(),
                adoption: false,
            },
            MutationScope::Path { .. } => {
                return Err(rpc_error("invalidMutation", "path mutation pattern cannot be empty"));
            }
        };
        let position = updates.iter().position(|update| update.scope == scope);
        let target = match position {
            Some(index) => &mut updates[index],
            None => {
                updates.push(RuleConfigUpdate { scope, rules: BTreeMap::new() });
                let index = updates.len().saturating_sub(1);
                &mut updates[index]
            }
        };
        for code in codes {
            let descriptor = by_code.get(code.as_str()).ok_or_else(|| {
                rpc_error_data("unknownRule", "rule disappeared during selector expansion", serde_json::json!({ "rule": code }))
            })?;
            let _ = target.rules.insert(code.clone(), setting_severity(mutation.setting, descriptor));
            let _ = expanded.insert(code);
        }
    }
    let expanded_rule_codes = catalog
        .iter()
        .filter(|rule| expanded.contains(&rule.code))
        .map(|rule| rule.code.clone())
        .collect();
    Ok((updates, expanded_rule_codes))
}

#[expect(clippy::too_many_arguments, reason = "wire impact is a flat typeDiagram projection")]
fn build_impact(
    before_document: &ConfigDocument,
    patch: &ConfigPatch,
    catalog: &[super::model::RuleDescriptor],
    expanded: &[String],
    before: &super::snapshot::Inventory,
    after: &super::snapshot::Inventory,
    run_safe_fixes: bool,
    index: &crate::workspace::WorkspaceIndex,
    root: &Path,
) -> ConfigurationImpact {
    let changed_rules = expanded.iter().filter(|code| {
        before_document.config.rules.get(*code) != patch.config.rules.get(*code)
            || before_document.config.per_path_overrides != patch.config.per_path_overrides
    }).count();
    let enabled_rules = catalog.iter().filter(|rule| {
        severities(rule, &patch.config).1 != super::model::RuleSeverity::Disabled
    }).count();
    let disabled_rules = catalog.len().saturating_sub(enabled_rules);
    let files_changed_by_safe_fixes = if run_safe_fixes {
        index.files.iter().filter(|entry| {
            entry.key().starts_with(root)
                && entry.diagnostics.iter().any(|diag| super::catalog::is_safe_fixable(diag.code.code))
        }).count()
    } else { 0 };
    ConfigurationImpact {
        changed_rules: count_i64(changed_rules),
        enabled_rules: count_i64(enabled_rules),
        disabled_rules: count_i64(disabled_rules),
        diagnostics_before: count_i64(before.total),
        diagnostics_after: count_i64(after.total),
        errors_before: count_i64(before.errors),
        errors_after: count_i64(after.errors),
        warnings_before: count_i64(before.warnings),
        warnings_after: count_i64(after.warnings),
        files_changed_by_safe_fixes: count_i64(files_changed_by_safe_fixes),
    }
}

fn replacement_edit(document: &ConfigDocument, patch: &ConfigPatch) -> LspResult<WorkspaceEdit> {
    let uri = Url::from_file_path(&patch.path)
        .map_err(|()| rpc_error("invalidMutation", "configuration path cannot be represented as a URI"))?;
    let end = crate::util::byte_offset_to_position(&document.content, document.content.len());
    let text_edit = TextDocumentEdit {
        text_document: OptionalVersionedTextDocumentIdentifier { uri: uri.clone(), version: None },
        edits: vec![OneOf::Left(TextEdit {
            range: Range { start: Default::default(), end },
            new_text: patch.content.clone(),
        })],
    };
    let mut operations = Vec::new();
    if !document.exists {
        operations.push(DocumentChangeOperation::Op(ResourceOp::Create(CreateFile {
            uri,
            options: Some(CreateFileOptions { overwrite: Some(false), ignore_if_exists: Some(false) }),
            annotation_id: None,
        })));
    }
    operations.push(DocumentChangeOperation::Edit(text_edit));
    Ok(WorkspaceEdit {
        changes: None,
        document_changes: Some(DocumentChanges::Operations(operations)),
        change_annotations: None,
    })
}

/// Reload, recheck, publish, and notify after an API or external config edit.
pub(crate) async fn refresh_after_configuration_change(
    server: &LspServer,
    root: &Path,
    reason: &str,
) -> LspResult<()> {
    let results = {
        let mut guard = server.index.write().await;
        let index = guard.as_mut().ok_or_else(|| rpc_error("invalidMutation", "workspace index is not ready"))?;
        index.reload_root_configs();
        index.reresolve_imports_and_recheck()
    };
    for (uri, diagnostics) in results {
        server.publish_diagnostics_if_enabled(uri, diagnostics).await;
    }
    let document = discover_config_document(root).map_err(config_error)?;
    server
        .client
        .send_notification::<ConfigurationChangedNotification>(ConfigurationChanged {
            root_uri: path_uri(root),
            revision: document.revision,
            reason: reason.to_owned(),
        })
        .await;
    Ok(())
}

pub(crate) struct ConfigurationChangedNotification;

impl tower_lsp::lsp_types::notification::Notification for ConfigurationChangedNotification {
    type Params = ConfigurationChanged;
    const METHOD: &'static str = basilisk_common::configuration_editor::CHANGED;
}

fn require_revision(document: &ConfigDocument, expected: &str) -> LspResult<()> {
    if document.revision == expected {
        Ok(())
    } else {
        Err(rpc_error_data(
            "revisionConflict",
            "configuration changed; refresh and preview again",
            serde_json::json!({ "expected": expected, "actual": document.revision }),
        ))
    }
}

fn config_error(error: ConfigDocumentError) -> Error {
    let kind = match error {
        ConfigDocumentError::Invalid { .. } | ConfigDocumentError::Read { .. } => "invalidConfiguration",
        ConfigDocumentError::RevisionConflict { .. } => "revisionConflict",
        ConfigDocumentError::ReadOnly { .. } => "readOnlySource",
    };
    rpc_error_data(kind, &error.to_string(), serde_json::json!({}))
}

fn selection_error(error: SelectionError) -> Error {
    match error {
        SelectionError::UnknownRule(rule) => rpc_error_data(
            "unknownRule",
            "selector contains an unknown rule",
            serde_json::json!({ "rule": rule }),
        ),
        SelectionError::UnknownTag(tag) => rpc_error_data(
            "unknownTag",
            "selector contains an unknown tag",
            serde_json::json!({ "tag": tag }),
        ),
    }
}

fn rpc_error(kind: &str, message: &str) -> Error {
    rpc_error_data(kind, message, serde_json::json!({}))
}

fn rpc_error_data(kind: &str, message: &str, context: serde_json::Value) -> Error {
    Error {
        code: ErrorCode::ServerError(-32020),
        message: message.to_owned().into(),
        data: Some(serde_json::json!({ "kind": kind, "context": context })),
    }
}

fn path_uri(path: &Path) -> String {
    Url::from_file_path(path).map_or_else(|()| path.to_string_lossy().into_owned(), |uri| uri.to_string())
}

fn count_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}
