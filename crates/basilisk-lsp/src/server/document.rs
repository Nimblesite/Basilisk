//! Implements [LSPARCH-ARCH-MODSTRUCT]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-MODSTRUCT
//!
//! Text document lifecycle handlers for the Basilisk LSP server.
//!
//! Covers `did_open`, `did_change`, `did_save`, `did_close`, and
//! `did_change_watched_files`.

use std::sync::Arc;

use tower_lsp::lsp_types::{
    DidChangeTextDocumentParams, DidChangeWatchedFilesParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, FileChangeType, Url,
};
use tracing::info;

use crate::config::AnalysisMode;
use crate::util::position_to_byte_offset;

use super::{LspServer, FILE_WATCHER_DEBOUNCE_MS};

/// Handle `textDocument/didOpen`: index the document and publish diagnostics.
pub(super) async fn did_open(server: &LspServer, params: DidOpenTextDocumentParams) {
    let uri = params.text_document.uri;
    let text = params.text_document.text;
    let version = params.text_document.version;
    let guard = server.index.read().await;
    let Some(index) = guard.as_ref() else { return };
    let diags = index.set_open(&uri, &text, version);
    drop(guard);
    server.client.publish_diagnostics(uri, diags, None).await;
}

/// Handle `textDocument/didChange`: apply incremental edits and republish diagnostics.
pub(super) async fn did_change(server: &LspServer, params: DidChangeTextDocumentParams) {
    let uri = params.text_document.uri;
    let version = params.text_document.version;

    // Get current text for incremental edits.
    let guard = server.index.read().await;
    let Some(index) = guard.as_ref() else { return };
    let mut text = index.get_text(&uri).unwrap_or_default();
    drop(guard);

    // Apply incremental changes.
    for change in params.content_changes {
        if let Some(range) = change.range {
            let start = position_to_byte_offset(&text, range.start);
            let end = position_to_byte_offset(&text, range.end);
            text.replace_range(start..end, &change.text);
        } else {
            text = change.text;
        }
    }

    let guard = server.index.read().await;
    let Some(index) = guard.as_ref() else { return };
    // In cross-module mode an export-changing edit must refresh dependents live,
    // even though the edited file is open (GitHub #56). [ANALYSIS-SYMBOLS-INVAL]
    let results = index.set_open_refresh_dependents(&uri, &text, version);
    drop(guard);
    for (target, diags) in results {
        server.client.publish_diagnostics(target, diags, None).await;
    }
}

/// Handle `textDocument/didSave`: re-run the pipeline on the cached text.
pub(super) async fn did_save(server: &LspServer, params: DidSaveTextDocumentParams) {
    let uri = params.text_document.uri;
    // Re-run the pipeline on the cached in-memory text (already up-to-date).
    let guard = server.index.read().await;
    let Some(index) = guard.as_ref() else { return };
    let Some(text) = index.get_text(&uri) else {
        return;
    };
    let results = index.set_open_refresh_dependents(&uri, &text, 0);
    drop(guard);
    for (target, diags) in results {
        server.client.publish_diagnostics(target, diags, None).await;
    }

    // Notify activity panels that this module changed.
    super::activity_panel::send_module_changed(server, &uri).await;

    // Re-discover tests if this is a test file and auto-discover is enabled.
    let test_config = server.test_config.read().await;
    let should_discover = test_config.enabled && test_config.auto_discover_on_save;
    drop(test_config);

    if should_discover {
        if let Ok(path) = uri.to_file_path() {
            if is_test_file(&path) {
                let source = server
                    .with_index(|idx| idx.get_text(&uri))
                    .await
                    .unwrap_or_default();
                if !source.is_empty() {
                    let items = crate::test_discovery::discover_tests_in_file(&path, &source);
                    super::test_handlers::send_test_discovery_notification(server, items).await;
                }
            }
        }
    }
}

/// Check if a path is a test file (`test_*.py` or `*_test.py`).
fn is_test_file(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|ext| ext == "py")
        && path
            .file_stem()
            .and_then(|s| s.to_str())
            .is_some_and(|stem| stem.starts_with("test_") || stem.ends_with("_test"))
}

/// Handle `textDocument/didClose`: clear or re-publish diagnostics according to
/// the current analysis mode and whether the file is inside a workspace root.
pub(super) async fn did_close(server: &LspServer, params: DidCloseTextDocumentParams) {
    let uri = params.text_document.uri;
    super::diaglog!("[DIAG] did_close ENTER uri={uri}");
    let guard = server.index.read().await;
    let Some(index) = guard.as_ref() else {
        super::diaglog!("[DIAG] did_close: no index, clearing");
        info!(uri = %uri, "did_close: no index, clearing diagnostics");
        server.client.publish_diagnostics(uri, vec![], None).await;
        return;
    };
    let mode = index.mode;
    let roots_debug: Vec<_> = index
        .roots
        .iter()
        .map(|r| r.display().to_string())
        .collect();
    super::diaglog!("[DIAG] did_close: mode={mode:?} roots={roots_debug:?} uri={uri}");
    info!(uri = %uri, ?mode, "did_close: processing");

    match mode {
        AnalysisMode::OpenFilesOnly => {
            // Remove from index so no subsequent event republishes.
            if let Ok(path) = uri.to_file_path() {
                let _ = index.files.remove(&path);
            }
            drop(guard);
            super::diaglog!("[DIAG] did_close: OpenFilesOnly -> clearing uri={uri}");
            info!(uri = %uri, "did_close: openFilesOnly — clearing diagnostics");
            server.client.publish_diagnostics(uri, vec![], None).await;
        }
        AnalysisMode::WholeModule | AnalysisMode::CrossModule => {
            let in_workspace = uri
                .to_file_path()
                .is_ok_and(|path| index.roots.iter().any(|root| path.starts_with(root)));
            super::diaglog!("[DIAG] did_close: WholeModule in_workspace={in_workspace} uri={uri}");
            if in_workspace {
                let (publish_uri, diags) = index.set_closed(&uri);
                let diag_count = diags.len();
                drop(guard);
                super::diaglog!(
                    "[DIAG] did_close: WholeModule in-workspace republishing {diag_count} diags"
                );
                info!(uri = %uri, diag_count, "did_close: wholeModule in-workspace — republishing");
                server
                    .client
                    .publish_diagnostics(publish_uri, diags, None)
                    .await;
            } else {
                // Remove from index so no subsequent event republishes.
                if let Ok(path) = uri.to_file_path() {
                    let _ = index.files.remove(&path);
                }
                drop(guard);
                super::diaglog!(
                    "[DIAG] did_close: WholeModule out-of-workspace -> clearing uri={uri}"
                );
                info!(uri = %uri, "did_close: wholeModule out-of-workspace — clearing");
                server.client.publish_diagnostics(uri, vec![], None).await;
            }
        }
    }
}

/// Handle `workspace/didChangeWatchedFiles`: debounce and republish diagnostics
/// for changed/created Python files, and clear diagnostics for deleted files.
pub(super) async fn did_change_watched_files(
    server: &LspServer,
    params: DidChangeWatchedFilesParams,
) {
    // Quick mode check — bail early for openFilesOnly without taking the write lock.
    {
        let guard = server.index.read().await;
        let Some(index) = guard.as_ref() else { return };
        if index.mode == AnalysisMode::OpenFilesOnly {
            return; // File-watcher events are irrelevant in openFilesOnly mode.
        }
    }

    // A change to any project config file must update diagnostics without an LSP
    // restart: reload each root's checker config so version-aware rules and
    // severity overrides take effect (issue #93 reactivity), then rebuild the uv
    // package registry and re-resolve + re-check every file.
    let needs_config_refresh = params.changes.iter().any(|change| {
        let path = change.uri.path();
        path.ends_with("uv.lock")
            || path.ends_with("pyproject.toml")
            || path.ends_with("basilisk.json")
            || path.ends_with(".python-version")
    });

    log_uv_config_changes(&params);
    if needs_config_refresh {
        reload_index_configs(server).await;
        super::init::rebuild_registry_and_resolve(server).await;
    }

    // Classify the incoming changes, filtering to Python files only.
    let mut reload_targets: Vec<Url> = Vec::new();
    let mut delete_targets: Vec<Url> = Vec::new();
    // A newly-created module changes what dependents can resolve, so it requires
    // re-resolving the whole workspace — not just reloading the new file. A pure
    // content change (CHANGED) leaves the module path untouched, so the cheaper
    // per-file reload suffices. Implements [ANALYSIS-INCR-IMPORTS] (GitHub #53).
    let mut has_created_module = false;

    for change in &params.changes {
        let uri = &change.uri;
        let path = uri.to_file_path().unwrap_or_default();
        if !path
            .extension()
            .is_some_and(|ext| ext == "py" || ext == "pyi")
        {
            continue;
        }
        match change.typ {
            FileChangeType::CREATED => {
                has_created_module = true;
                reload_targets.push(uri.clone());
            }
            FileChangeType::CHANGED => {
                reload_targets.push(uri.clone());
            }
            FileChangeType::DELETED => {
                delete_targets.push(uri.clone());
            }
            _ => {}
        }
    }

    if reload_targets.is_empty() && delete_targets.is_empty() {
        return;
    }

    // Debounce: abort any pending watcher task and replace with a new one
    // that fires after FILE_WATCHER_DEBOUNCE_MS milliseconds.
    let index_lock = Arc::clone(&server.index);
    let client = server.client.clone();

    let task = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(FILE_WATCHER_DEBOUNCE_MS)).await;

        let guard = index_lock.read().await;
        let Some(index) = guard.as_ref() else { return };

        // Reload changed files while diffing their exported symbol sets: in
        // cross-module mode an export change must refresh dependents' stale
        // symbol diagnostics. Implements [ANALYSIS-SYMBOLS-INVAL] (GitHub #56).
        let mut exports_changed = false;
        let cross_module = matches!(index.mode, crate::config::AnalysisMode::CrossModule);
        let reload_results: Vec<_> = reload_targets
            .iter()
            .filter_map(|uri| {
                let (result, changed) = index.reload_and_diff_exports(uri)?;
                exports_changed |= changed && cross_module;
                Some(result)
            })
            .collect();

        // When a new module appeared, its dependents may now resolve imports
        // that were previously unresolved. Re-resolve the whole workspace so
        // their stale BSK-E0010 clears without an LSP reload (GitHub #53).
        // Likewise when an edited module's exports changed (GitHub #56).
        let publish_set = if has_created_module || exports_changed {
            // Drop deleted entries first so the recheck cannot resurrect them.
            for uri in &delete_targets {
                index.forget_file(uri);
            }
            index.reresolve_imports_and_recheck()
        } else {
            reload_results
        };

        drop(guard);

        for (uri, diags) in publish_set {
            client.publish_diagnostics(uri, diags, None).await;
        }
        for uri in delete_targets {
            client.publish_diagnostics(uri, vec![], None).await;
        }
    });

    let abort_handle = task.abort_handle();
    let mut debounce = server.watcher_debounce.lock().await;
    if let Some(old) = debounce.take() {
        old.abort();
    }
    *debounce = Some(abort_handle);
}

/// Re-read each workspace root's checker config from disk after a watched config
/// file changed, so version-aware rules and severity overrides update without an
/// LSP restart. Implements [CHKARCH-VERSION-TARGET] reactivity.
async fn reload_index_configs(server: &LspServer) {
    let mut guard = server.index.write().await;
    if let Some(index) = guard.as_mut() {
        index.reload_root_configs();
    }
}

/// Log changes to uv-related configuration files.
///
/// Inspects watched file events for `uv.lock`, `.python-version`, and
/// `pyproject.toml` and emits structured log messages. The actual registry
/// rebuild will be wired up when the workspace holds the package registry.
fn log_uv_config_changes(params: &DidChangeWatchedFilesParams) {
    for change in &params.changes {
        let path_str = change.uri.path();
        if path_str.ends_with("uv.lock") {
            info!("uv.lock changed — rebuilding package registry");
        } else if path_str.ends_with(".python-version") {
            info!(".python-version changed — updating Python version");
        } else if path_str.ends_with("pyproject.toml") {
            info!("pyproject.toml changed — checking for dependency changes");
        }
    }
}
