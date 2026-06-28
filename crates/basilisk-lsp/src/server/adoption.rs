//! Implements [LSPARCH-ARCH-MODSTRUCT]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-MODSTRUCT
//!
//! Adoption command handlers for gradual adoption mode.
//!
//! Implements `basilisk.adoptFile`, `basilisk.adoptWorkspace`, and
//! `basilisk.unadoptFile` execute-command handlers.

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::MessageType;
use tracing::{error, info, warn};

use super::LspServer;

/// Handle `basilisk.adoptFile`.
///
/// Records all current error codes for the given file in the adoption store,
/// demoting them from errors to warnings until the user fixes them.
// Implements [AUTOFIX-ADOPTION-FLOW] (Adopt File) + [AUTOFIX-ADOPTION-VSCODE]
// command `basilisk.adoptFile`. DEVIATION vs spec step 2: this does NOT run
// Mass Autofix (safe) first — it records the file's current error codes
// directly, then re-publishes with demoted severities. See report.
pub(super) async fn execute_adopt_file(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let Some(uri_str) = args.first().and_then(|v| v.as_str()) else {
        warn!("adoptFile: missing URI argument");
        return Ok(None);
    };
    let Ok(uri) = tower_lsp::lsp_types::Url::parse(uri_str) else {
        warn!("adoptFile: invalid URI");
        return Ok(None);
    };

    let roots = server.workspace_roots.read().await;
    let Some(project_root) = roots.first().cloned() else {
        warn!("adoptFile: no workspace root available");
        return Ok(None);
    };
    drop(roots);

    // Collect error codes from the file's current diagnostics.
    let error_codes: Option<Vec<String>> = server
        .with_index(|idx| {
            let path = uri.to_file_path().ok()?;
            let entry = idx.files.get(&path)?;
            let codes: Vec<String> = entry
                .diagnostics
                .iter()
                .filter(|d| d.severity == basilisk_checker::Severity::Error)
                .map(|d| d.code.code.to_owned())
                .collect();
            Some(codes)
        })
        .await;

    let Some(error_codes) = error_codes else {
        warn!(uri = %uri, "adoptFile: file not found in workspace index");
        return Ok(Some(serde_json::json!({ "adopted": false, "demoted": 0 })));
    };

    let demoted_count = error_codes.len();

    if error_codes.is_empty() {
        info!(uri = %uri, "adoptFile: no errors to adopt");
        return Ok(Some(serde_json::json!({ "adopted": true, "demoted": 0 })));
    }

    // Load the adoption store, record the codes, and save.
    let mut store = match basilisk_config::AdoptionStore::load(&project_root) {
        Ok(s) => s,
        Err(err) => {
            error!(%err, "adoptFile: failed to load adoption store");
            return Ok(None);
        }
    };

    let Ok(file_path) = uri.to_file_path() else {
        return Ok(None);
    };
    let relative = file_path.strip_prefix(&project_root).unwrap_or(&file_path);
    store.adopt_file(relative, error_codes);

    if let Err(err) = store.save() {
        error!(%err, "adoptFile: failed to save adoption store");
        return Ok(None);
    }

    // Re-publish diagnostics with demoted severities.
    republish_diagnostics_for_uri(server, &uri, &project_root, &store).await;

    info!(uri = %uri, demoted_count, "adoptFile: adopted file");
    server
        .client
        .log_message(
            MessageType::INFO,
            format!("Basilisk: adopted file with {demoted_count} demoted error(s)"),
        )
        .await;

    Ok(Some(
        serde_json::json!({ "adopted": true, "demoted": demoted_count }),
    ))
}

/// Handle `basilisk.adoptWorkspace`.
///
/// Adopts all files in the workspace that have errors, recording their error
/// codes in the adoption store.
// Implements [AUTOFIX-ADOPTION-FLOW] (Adopt Workspace) + [AUTOFIX-ADOPTION-VSCODE]
// command `basilisk.adoptWorkspace`. Same step-2 deviation as adopt_file (no
// pre-pass Mass Autofix).
pub(super) async fn execute_adopt_workspace(
    server: &LspServer,
    _args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    info!("adoptWorkspace: starting");

    let roots = server.workspace_roots.read().await;
    let Some(project_root) = roots.first().cloned() else {
        warn!("adoptWorkspace: no workspace root available");
        return Ok(None);
    };
    drop(roots);

    // Collect error codes per file from all files in the workspace index.
    let file_codes: Option<Vec<(std::path::PathBuf, Vec<String>)>> = server
        .with_index(|idx| {
            let mut results = Vec::new();
            for entry in &idx.files {
                let path = entry.key().clone();
                let codes: Vec<String> = entry
                    .value()
                    .diagnostics
                    .iter()
                    .filter(|d| d.severity == basilisk_checker::Severity::Error)
                    .map(|d| d.code.code.to_owned())
                    .collect();
                if !codes.is_empty() {
                    results.push((path, codes));
                }
            }
            Some(results)
        })
        .await;

    let Some(file_codes) = file_codes else {
        warn!("adoptWorkspace: workspace index not available");
        return Ok(Some(
            serde_json::json!({ "adopted": false, "files": 0, "demoted": 0 }),
        ));
    };

    if file_codes.is_empty() {
        info!("adoptWorkspace: no files with errors");
        return Ok(Some(
            serde_json::json!({ "adopted": true, "files": 0, "demoted": 0 }),
        ));
    }

    let mut store = match basilisk_config::AdoptionStore::load(&project_root) {
        Ok(s) => s,
        Err(err) => {
            error!(%err, "adoptWorkspace: failed to load adoption store");
            return Ok(None);
        }
    };

    let mut total_demoted: usize = 0;
    let file_count = file_codes.len();

    for (path, codes) in &file_codes {
        let relative = path.strip_prefix(&project_root).unwrap_or(path);
        total_demoted += codes.len();
        store.adopt_file(relative, codes.clone());
    }

    if let Err(err) = store.save() {
        error!(%err, "adoptWorkspace: failed to save adoption store");
        return Ok(None);
    }

    // Re-publish diagnostics for all adopted files.
    for (path, _) in &file_codes {
        if let Ok(uri) = tower_lsp::lsp_types::Url::from_file_path(path) {
            republish_diagnostics_for_uri(server, &uri, &project_root, &store).await;
        }
    }

    info!(file_count, total_demoted, "adoptWorkspace: completed");
    server
        .client
        .log_message(
            MessageType::INFO,
            format!("Basilisk: adopted {file_count} file(s) with {total_demoted} demoted error(s)"),
        )
        .await;

    Ok(Some(
        serde_json::json!({ "adopted": true, "files": file_count, "demoted": total_demoted }),
    ))
}

/// Handle `basilisk.unadoptFile`.
///
/// Removes all adoption overrides for the given file, restoring full
/// strictness.
// Implements [AUTOFIX-ADOPTION-RULES] (Manual un-adoption) +
// [AUTOFIX-ADOPTION-VSCODE] command `basilisk.unadoptFile`.
pub(super) async fn execute_unadopt_file(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let Some(uri_str) = args.first().and_then(|v| v.as_str()) else {
        warn!("unadoptFile: missing URI argument");
        return Ok(None);
    };
    let Ok(uri) = tower_lsp::lsp_types::Url::parse(uri_str) else {
        warn!("unadoptFile: invalid URI");
        return Ok(None);
    };

    let roots = server.workspace_roots.read().await;
    let Some(project_root) = roots.first().cloned() else {
        warn!("unadoptFile: no workspace root available");
        return Ok(None);
    };
    drop(roots);

    let mut store = match basilisk_config::AdoptionStore::load(&project_root) {
        Ok(s) => s,
        Err(err) => {
            error!(%err, "unadoptFile: failed to load adoption store");
            return Ok(None);
        }
    };

    let Ok(file_path) = uri.to_file_path() else {
        return Ok(None);
    };
    let relative = file_path.strip_prefix(&project_root).unwrap_or(&file_path);
    store.unadopt_file(relative);

    if let Err(err) = store.save() {
        error!(%err, "unadoptFile: failed to save adoption store");
        return Ok(None);
    }

    // Re-publish diagnostics with original severities (no demotions).
    republish_diagnostics_for_uri(server, &uri, &project_root, &store).await;

    info!(uri = %uri, "unadoptFile: un-adopted file");
    server
        .client
        .log_message(MessageType::INFO, format!("Basilisk: un-adopted {uri}"))
        .await;

    Ok(Some(serde_json::json!({ "unadopted": true })))
}

/// Re-publish LSP diagnostics for a single URI, applying adoption overrides.
// Implements [AUTOFIX-ADOPTION-FLOW] step 5 — demote adopted codes to Warning
// via `apply_adoptions`. DEVIATION: only invoked from these adopt/unadopt
// command handlers; the normal diagnostic-publish path (server/document.rs,
// server/init.rs) does NOT call apply_adoptions, so demotions are lost on the
// next edit/re-check and `auto_graduate` ([AUTOFIX-ADOPTION-RULES]) never runs
// in production. See report.
async fn republish_diagnostics_for_uri(
    server: &LspServer,
    uri: &tower_lsp::lsp_types::Url,
    project_root: &std::path::Path,
    store: &basilisk_config::AdoptionStore,
) {
    let lsp_diags: Option<Vec<tower_lsp::lsp_types::Diagnostic>> = server
        .with_index(|idx| {
            let path = uri.to_file_path().ok()?;
            let entry = idx.files.get(&path)?;
            let mut checker_diags = entry.diagnostics.clone();
            crate::workspace_analysis::apply_adoptions(
                &mut checker_diags,
                &path,
                project_root,
                store,
            );
            let diags = checker_diags
                .iter()
                .map(|d| crate::workspace_analysis::bsk_to_lsp(d, &entry.text))
                .collect();
            Some(diags)
        })
        .await;

    if let Some(diags) = lsp_diags {
        server
            .client
            .publish_diagnostics(uri.clone(), diags, None)
            .await;
    }
}
