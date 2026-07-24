//! User-invoked Typeshed downloads ([LSPCFGED-TYPESHED-DOWNLOAD]).
//!
//! Lives OUTSIDE the configuration editor ([TYPESHEDRT-SEGREGATION]): the
//! editor only reads and writes the three Typeshed keys, while this module is
//! the only LSP code that reaches the network — and it runs only when a
//! person invokes one of the two Download buttons. The root's generation map
//! is untouched while a download runs, so the active source keeps serving
//! analysis and the transient `Downloading` status exists only on the
//! invoking control, never as a panel-blocking mode.

use std::path::{Path, PathBuf};

use basilisk_config::{BasiliskConfig, ConfigDocument, ConfigurationUpdate, TypeshedConfigKey};
use basilisk_stubs::typeshed::gittree::Oid;
use basilisk_typeshed_fetch::{DownloadError, DownloadOutcome, DownloadPhase, GithubClient};
use tower_lsp::jsonrpc::Result as LspResult;

use crate::configuration_editor::rpc_error;
use crate::server::typeshed_status::{downloading_state, notify_status, TypeshedGeneration};
use crate::server::LspServer;

/// Download `python/typeshed@main` into the store and write the returned SHA
/// as the pin — the Download latest contract ([STUBRES-TYPESHED-DOWNLOAD]).
/// The pin write rides the same validated editor transaction as any other
/// configuration edit, which re-resolves locally and publishes the terminal
/// generation.
pub(crate) async fn download_latest_and_pin(
    server: &LspServer,
    root: &Path,
    document: &ConfigDocument,
) -> LspResult<()> {
    notify_status(&server.client, root, downloading_state(None)).await;
    let result = match run_download(store_path_for(root, &document.config), None).await {
        Ok(outcome) => crate::configuration_editor::apply_configuration_update(
            server,
            root,
            &pin_update(&outcome),
            "typeshedDownloadLatest",
        )
        .await
        .map(|_applied| ()),
        Err(error) => Err(error),
    };
    // Always settle on a terminal status so the invoking control's transient
    // download state clears — even when the pin was already current and the
    // transaction therefore staged nothing.
    republish_terminal(server, root).await;
    result
}

/// Materialise the existing pin on a machine that never downloaded it. Writes
/// no configuration; on success the root re-resolves locally and activates.
pub(crate) async fn download_pinned(
    server: &LspServer,
    root: &Path,
    document: &ConfigDocument,
) -> LspResult<()> {
    if document.config.typeshed_path.is_some() {
        return Err(rpc_error(
            "typeshedActionUnavailable",
            "a custom Typeshed folder has no commit to download",
        ));
    }
    let Some(commit_hex) = document.config.typeshed_commit.clone() else {
        return Err(rpc_error(
            "typeshedActionUnavailable",
            "no pinned commit to download — the unset pin resolves to the bundled source",
        ));
    };
    let commit = Oid::from_hex(&commit_hex).map_err(|_invalid_oid| {
        rpc_error(
            "invalidTypeshedSetting",
            "typeshed-commit must be a full 40-character hexadecimal SHA",
        )
    })?;
    notify_status(&server.client, root, downloading_state(Some(&commit_hex))).await;
    let result = match run_download(store_path_for(root, &document.config), Some(commit)).await {
        Ok(_outcome) => {
            crate::configuration_editor::resolve_and_activate(server, root, document).await
        }
        Err(error) => Err(error),
    };
    republish_terminal(server, root).await;
    result
}

/// The workspace-resolved store override, when configured. `None` lets the
/// download component fall back to the canonical per-user store
/// ([STUBRES-TYPESHED-STORE]).
fn store_path_for(root: &Path, config: &BasiliskConfig) -> Option<PathBuf> {
    config.typeshed_store_path.as_ref().map(|path| {
        if path.is_absolute() {
            path.clone()
        } else {
            root.join(path)
        }
    })
}

async fn run_download(
    store_path: Option<PathBuf>,
    commit: Option<Oid>,
) -> LspResult<DownloadOutcome> {
    tokio::task::spawn_blocking(move || {
        let client = GithubClient::new();
        let progress = |phase: DownloadPhase| {
            tracing::info!(?phase, "typeshed download progress");
        };
        match commit {
            Some(commit) => {
                basilisk_typeshed_fetch::download_commit(commit, store_path, &client, &progress)
            }
            None => basilisk_typeshed_fetch::download_latest(store_path, &client, &progress),
        }
    })
    .await
    .map_err(|_join_error| rpc_error("typeshedDownloadFailed", "typeshed download task failed"))?
    .map_err(|error| rpc_error(download_error_code(error), &error.to_string()))
}

const fn download_error_code(error: DownloadError) -> &'static str {
    match error {
        DownloadError::LicenseChanged => "typeshedLicenseChanged",
        DownloadError::Metadata
        | DownloadError::Download
        | DownloadError::Validation
        | DownloadError::Store => "typeshedDownloadFailed",
    }
}

fn pin_update(outcome: &DownloadOutcome) -> ConfigurationUpdate {
    let mut update = ConfigurationUpdate::default();
    let _ = update.typeshed.entries.insert(
        TypeshedConfigKey::TypeshedCommit,
        Some(outcome.commit.to_hex()),
    );
    let _ = update
        .typeshed
        .entries
        .insert(TypeshedConfigKey::TypeshedPath, None);
    update
}

/// Re-notify the root's current terminal status after a download settles.
async fn republish_terminal(server: &LspServer, root: &Path) {
    let status = server
        .typeshed_generations
        .read()
        .await
        .get(root)
        .map(TypeshedGeneration::status_state);
    match status {
        Some(status) => notify_status(&server.client, root, status).await,
        None => {
            tracing::warn!(root = %root.display(), "no Typeshed generation to republish after download");
        }
    }
}

#[cfg(test)]
mod tests {
    use basilisk_stubs::typeshed::gittree::Oid;
    use basilisk_typeshed_fetch::{DownloadError, DownloadOutcome};

    use super::{download_error_code, pin_update, store_path_for};
    use basilisk_config::{BasiliskConfig, TypeshedConfigKey};

    /// [STUBRES-TYPESHED-DOWNLOAD]: Download latest writes the returned SHA
    /// as the pin and clears any custom folder — the two keys are mutually
    /// exclusive, so the update must retire one while setting the other.
    #[test]
    fn download_latest_pin_update_sets_commit_and_clears_custom_path() {
        let Ok(commit) = Oid::from_hex("0123456789012345678901234567890123456789") else {
            return;
        };
        let Ok(tree) = Oid::from_hex("abcdefabcdefabcdefabcdefabcdefabcdefabcd") else {
            return;
        };
        let update = pin_update(&DownloadOutcome { commit, tree });
        assert_eq!(
            update
                .typeshed
                .entries
                .get(&TypeshedConfigKey::TypeshedCommit),
            Some(&Some("0123456789012345678901234567890123456789".to_owned()))
        );
        assert_eq!(
            update
                .typeshed
                .entries
                .get(&TypeshedConfigKey::TypeshedPath),
            Some(&None)
        );
        assert!(!update
            .typeshed
            .entries
            .contains_key(&TypeshedConfigKey::TypeshedStorePath));
    }

    #[test]
    fn license_drift_keeps_its_typed_error_code() {
        assert_eq!(
            download_error_code(DownloadError::LicenseChanged),
            "typeshedLicenseChanged"
        );
        assert_eq!(
            download_error_code(DownloadError::Metadata),
            "typeshedDownloadFailed"
        );
    }

    #[test]
    fn relative_store_override_roots_at_the_workspace() {
        let mut config = BasiliskConfig::default();
        assert!(store_path_for(std::path::Path::new("/workspace"), &config).is_none());
        config.typeshed_store_path = Some(std::path::PathBuf::from("stores/typeshed"));
        assert_eq!(
            store_path_for(std::path::Path::new("/workspace"), &config),
            Some(std::path::PathBuf::from("/workspace/stores/typeshed"))
        );
    }
}
