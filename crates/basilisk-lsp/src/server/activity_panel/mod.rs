//! Implements [LSPARCH-ARCH-MODSTRUCT]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-MODSTRUCT
//!
//! Activity panel command handlers for the Basilisk LSP server.
//!
//! Implements `basilisk.workspaceModules` and `basilisk.typeHealth` execute-command
//! handlers that power the Module Explorer and Type Health panels in editor extensions.

mod benchmarks;
mod helpers;
mod module_tree;
mod type_health;

use tower_lsp::jsonrpc::Result as LspResult;
use tracing::info;

use self::helpers::module_name_from_path;
use self::module_tree::{build_module_tree, build_symbol_list};
use self::type_health::{build_type_health, empty_health_stats};

use super::LspServer;

/// Handle `basilisk.workspaceModules`.
///
/// Walks the workspace index and builds a hierarchical module tree from the
/// resolved symbol tables. Supports an optional `scope` parameter for prefix
/// filtering (used for lazy child loading).
pub(super) async fn execute_workspace_modules(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let scope = args
        .first()
        .and_then(|v| v.get("scope"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let modules: Option<Vec<serde_json::Value>> = server
        .with_index(|idx| Some(build_module_tree(idx, scope)))
        .await;

    let modules = modules.unwrap_or_default();
    info!(module_count = modules.len(), scope, "workspaceModules");

    Ok(Some(serde_json::json!({ "modules": modules })))
}

/// Handle `basilisk.typeHealth`.
///
/// Computes type coverage statistics (annotated vs unannotated symbols),
/// error/warning counts, and adoption state for each file in the workspace.
pub(super) async fn execute_type_health(
    server: &LspServer,
    _args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let roots = server.workspace_roots.read().await;
    let project_root = roots.first().cloned();
    drop(roots);

    let result: Option<serde_json::Value> = server
        .with_index(|idx| Some(build_type_health(idx, project_root.as_deref())))
        .await;

    let result = result.unwrap_or_else(|| {
        serde_json::json!({
            "workspace": empty_health_stats(),
            "modules": []
        })
    });

    info!("typeHealth computed");
    Ok(Some(result))
}

// ── Module change notification ────────────────────────────────────────────

/// Notification type for `basilisk/moduleChanged`.
pub(crate) struct ModuleChangedNotification;

impl tower_lsp::lsp_types::notification::Notification for ModuleChangedNotification {
    type Params = serde_json::Value;
    const METHOD: &'static str = basilisk_common::notifications::MODULE_CHANGED;
}

/// Send a debounced `basilisk/moduleChanged` notification for a file that was
/// just re-analysed. Waits 300 ms after the last save before sending, so rapid
/// saves don't flood the client.
pub(crate) async fn send_module_changed(server: &LspServer, uri: &tower_lsp::lsp_types::Url) {
    let uri = uri.clone();
    let index_lock = std::sync::Arc::clone(&server.index);
    let client = server.client.clone();

    let task = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(
            super::MODULE_CHANGED_DEBOUNCE_MS,
        ))
        .await;

        let module_data: Option<serde_json::Value> = {
            let guard = index_lock.read().await;
            guard.as_ref().and_then(|idx| {
                let path = uri.to_file_path().ok()?;
                let entry = idx.files.get(&path)?;
                let resolved = entry.resolved.as_ref()?;
                let module_name = module_name_from_path(&path, &idx.roots);
                if module_name.is_empty() {
                    return None;
                }

                let symbols = build_symbol_list(resolved, &entry.text);
                let kind = if path
                    .file_name()
                    .is_some_and(|n| n == "__init__.py" || n == "__init__.pyi")
                {
                    "package"
                } else {
                    "module"
                };

                Some(serde_json::json!({
                    "module": {
                        "name": module_name,
                        "path": path.display().to_string(),
                        "kind": kind,
                        "symbols": symbols,
                    }
                }))
            })
        };

        if let Some(data) = module_data {
            client
                .send_notification::<ModuleChangedNotification>(data)
                .await;
        }
    });

    // Abort any pending notification and replace with this new one.
    let abort_handle = task.abort_handle();
    let mut debounce = server.module_changed_debounce.lock().await;
    if let Some(old) = debounce.take() {
        old.abort();
    }
    *debounce = Some(abort_handle);
}
