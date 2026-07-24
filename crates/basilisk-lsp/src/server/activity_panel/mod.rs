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
/// Implements [EXTACT-LSP-COMMANDS-WORKSPACE-MODULES] — the client->server
/// request returning the module tree with the folded type-health rollup.
/// The optional `scope` param is the spec's module-name prefix filter.
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

    // The module tree's error/warning rollup mirrors the publish gate: with type
    // checking disabled the counts must read empty, just like the cleared editor
    // diagnostics ([ANALYSIS-ENABLED], GitHub #119).
    let type_checking_enabled = server.is_type_checking_enabled().await;

    // Whether a zero-file rollup is final ("no Python files") or merely "not
    // scanned yet" ([EXTACT-MODULES-HEADER-LOADING], GitHub #144).
    let scan_complete = server
        .initial_scan_complete
        .load(std::sync::atomic::Ordering::Relaxed);

    let tree = server
        .with_index(|idx| {
            Some(build_module_tree(
                idx,
                scope,
                type_checking_enabled,
                scan_complete,
            ))
        })
        .await;

    let response = match tree {
        Some(tree) => {
            info!(module_count = tree.modules.len(), scope, "workspaceModules");
            serde_json::json!({ "modules": tree.modules, "workspace": tree.workspace })
        }
        None => serde_json::json!({ "modules": [], "workspace": empty_health_stats() }),
    };

    Ok(Some(response))
}

/// Handle `basilisk.typeHealth`.
///
/// Implements [EXTACT-LSP-COMMANDS-TYPE-HEALTH] — the standalone workspace
/// health command for editors without a unified panel (Zed `/health`, Neovim
/// `:BasiliskHealth`); computed from the same per-file figures as the rollup
/// folded into `basilisk.workspaceModules`.
///
/// Computes type coverage statistics (annotated vs unannotated symbols) and
/// error/warning counts for each file in the workspace.
/// Gated on the Type Checking toggle like `basilisk.workspaceModules`
/// ([ANALYSIS-ENABLED], #119): while disabled it serves no grading.
pub(super) async fn execute_type_health(
    server: &LspServer,
    _args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let type_checking_enabled = server.is_type_checking_enabled().await;

    let result: Option<serde_json::Value> = server
        .with_index(|idx| Some(build_type_health(idx, type_checking_enabled)))
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

// Implements [LSPARCH-NOTIFS]
/// Notification type for `basilisk/moduleChanged`.
///
/// Implements [EXTACT-LSP-COMMANDS-MODULE-CHANGED] — the server->client
/// notification carrying `{ module: ModuleNode }` after file-save re-analysis.
pub(crate) struct ModuleChangedNotification;

impl tower_lsp::lsp_types::notification::Notification for ModuleChangedNotification {
    type Params = serde_json::Value;
    const METHOD: &'static str = basilisk_common::notifications::MODULE_CHANGED;
}

// Implements [LSPARCH-NOTIFS]
/// Notification type for `basilisk/scanComplete`.
///
/// Implements [EXTACT-LSP-COMMANDS-SCAN-COMPLETE] — the server->client
/// notification that a workspace scan finished. Panels showing the loading
/// state refetch on receipt; required because a genuinely empty workspace
/// publishes no diagnostics, so nothing else would ever settle the loading
/// message into the honest empty-state
/// ([EXTACT-MODULES-HEADER-LOADING], GitHub #144).
pub(crate) struct ScanCompleteNotification;

impl tower_lsp::lsp_types::notification::Notification for ScanCompleteNotification {
    type Params = serde_json::Value;
    const METHOD: &'static str = basilisk_common::notifications::SCAN_COMPLETE;
}

/// Send a debounced `basilisk/moduleChanged` notification for a file that was
/// just re-analysed. Waits 300 ms after the last save before sending, so rapid
/// saves don't flood the client.
///
/// Implements [EXTACT-PERFORMANCE] (and [EXTACT-LSP-COMMANDS-MODULE-CHANGED]):
/// the 300 ms debounce the spec mandates lives in `MODULE_CHANGED_DEBOUNCE_MS`.
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
