//! Root-keyed local Typeshed resolution — never a download.
//!
//! Implements [STUBRES-TYPESHED-PIN] activation for the LSP: resolving a
//! source is a local store/bundle read ([STUBRES-TYPESHED-OFFLINE]), so a
//! configuration change computes the NEXT terminal generation off the message
//! loop and publishes nothing until the matching edit has actually landed
//! ([LSPCFGED-TYPESHED]). There is no acquiring state, no busy error, and no
//! rollback — the previous generation keeps serving analysis until its
//! terminal replacement is swapped in atomically.

use std::path::Path;
use std::sync::Arc;

use basilisk_config::{BasiliskConfig, ConfigDocument};
use basilisk_stubs::typeshed::snapshot::Snapshot;
use tower_lsp::jsonrpc::Result as LspResult;

use super::transaction::ConfigurationRefreshHandles;
use crate::server::typeshed_status::{self, TypeshedFailure, TypeshedGeneration};
use crate::server::LspServer;

/// The next terminal generation, computed locally and held privately until
/// the matching configuration edit lands. Dropping it publishes nothing —
/// which is exactly what a rejected edit requires.
pub(super) struct StagedResolution {
    next: TypeshedGeneration,
}

impl StagedResolution {
    /// The candidate snapshot when the next generation is Ready.
    pub(super) fn candidate(&self) -> Option<&Arc<Snapshot>> {
        self.next.ready_snapshot()
    }

    /// Atomically replace the root's generation and notify clients. Elevated
    /// source warnings surface here, once, on activation.
    pub(super) async fn publish(self, handles: &ConfigurationRefreshHandles, root: &Path) {
        let next = self.next;
        let status = next.ready_status().cloned();
        let _ = handles
            .typeshed_generations
            .write()
            .await
            .insert(root.to_path_buf(), next.clone());
        typeshed_status::notify_generation(&handles.client, root, &next).await;
        if let Some(status) = status {
            typeshed_status::show_high_warnings(&handles.client, &status).await;
        }
    }
}

/// Compute the next generation only when one of the three Typeshed source
/// settings changed. A valid-but-missing pin is VALID configuration: the
/// result is a `NoSource` generation, never a request error.
pub(super) async fn stage_configuration_change(
    root: &Path,
    before: &BasiliskConfig,
    after: &BasiliskConfig,
) -> Option<StagedResolution> {
    if !typeshed_policy_changed(before, after) {
        return None;
    }
    Some(StagedResolution {
        next: resolve(root, after).await,
    })
}

/// Resolve one root's checker configuration to its terminal generation.
pub(crate) async fn resolve(root: &Path, config: &BasiliskConfig) -> TypeshedGeneration {
    resolve_workspace(super::watch::workspace_config_for_basilisk(root, config)).await
}

/// Resolve a workspace configuration to its terminal generation — a fast
/// local store/bundle read on a blocking thread, never the network.
pub(crate) async fn resolve_workspace(
    config: crate::config::WorkspaceConfig,
) -> TypeshedGeneration {
    let outcome = tokio::task::spawn_blocking(move || {
        let request =
            crate::config::typeshed_request(&config).map_err(TypeshedFailure::resolution)?;
        basilisk_stubs::typeshed::runtime::production_manager(request)
            .snapshot()
            .map_err(|error| TypeshedFailure::from_selection(&error))
    })
    .await
    .unwrap_or_else(|_join_error| {
        Err(TypeshedFailure::resolution("Typeshed resolution task failed"))
    });
    match outcome {
        Ok(snapshot) => TypeshedGeneration::Ready(snapshot),
        Err(failure) => TypeshedGeneration::NoSource { failure },
    }
}

/// Re-resolve one root locally and activate the outcome: publish the terminal
/// generation, then run the shared refresh tail. Used after a user-invoked
/// download completes ([LSPCFGED-TYPESHED-DOWNLOAD]).
pub(crate) async fn resolve_and_activate(
    server: &LspServer,
    root: &Path,
    document: &ConfigDocument,
) -> LspResult<()> {
    let staged = StagedResolution {
        next: resolve(root, &document.config).await,
    };
    let candidate = staged.candidate().cloned();
    let handles = server.refresh_handles();
    staged.publish(&handles, root).await;
    super::transaction::refresh_with_document_and_typeshed(
        &handles,
        root,
        "typeshedDownloadActivate",
        document,
        candidate.as_ref(),
    )
    .await
}

/// Whether a configuration change touches the Typeshed source policy — the
/// whole surface is three keys ([LSPCFGED-TYPESHED]).
fn typeshed_policy_changed(before: &BasiliskConfig, after: &BasiliskConfig) -> bool {
    before.typeshed_path != after.typeshed_path
        || before.typeshed_commit != after.typeshed_commit
        || before.typeshed_store_path != after.typeshed_store_path
}

#[cfg(test)]
mod tests {
    use basilisk_config::BasiliskConfig;

    use super::{stage_configuration_change, typeshed_policy_changed};

    #[test]
    fn staging_is_limited_to_the_three_typeshed_settings() {
        let before = BasiliskConfig::default();
        let mut rule_only = before.clone();
        rule_only.python_version = Some("3.12".to_owned());
        assert!(!typeshed_policy_changed(&before, &rule_only));

        let mut pin = before.clone();
        pin.typeshed_commit = Some("83c2518a9e6abbda0c44592c3483de459198f887".to_owned());
        assert!(typeshed_policy_changed(&before, &pin));

        let mut store = before.clone();
        store.typeshed_store_path = Some(std::path::PathBuf::from("stores/typeshed"));
        assert!(typeshed_policy_changed(&before, &store));
    }

    /// [LSPCFGED-TYPESHED]: a non-typeshed configuration change stages
    /// NOTHING — no resolution runs, no status is published, so the editor
    /// cannot flicker through any intermediate state.
    #[tokio::test]
    async fn unrelated_configuration_change_stages_nothing() {
        let before = BasiliskConfig::default();
        let mut after = before.clone();
        after.python_version = Some("3.13".to_owned());
        let staged =
            stage_configuration_change(std::path::Path::new("/workspace"), &before, &after).await;
        assert!(staged.is_none());
    }

    /// A valid-but-missing pin stages a terminal `NoSource` generation
    /// instead of failing the transaction: the config is valid, the machine
    /// just does not hold that commit ([STUBRES-TYPESHED-PIN]).
    #[tokio::test]
    async fn missing_pin_stages_a_terminal_no_source_generation() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let store = std::env::temp_dir().join(format!(
            "basilisk_resolution_empty_store_{}_{unique}",
            std::process::id()
        ));
        let before = BasiliskConfig::default();
        let mut after = before.clone();
        after.typeshed_commit = Some("0123456789012345678901234567890123456789".to_owned());
        after.typeshed_store_path = Some(store.clone());
        let staged =
            stage_configuration_change(std::path::Path::new("/workspace"), &before, &after).await;
        let Some(staged) = staged else {
            unreachable!("a pin change must stage a resolution");
        };
        assert!(staged.candidate().is_none());
        let state = staged.next.status_state();
        assert_eq!(
            state.lifecycle,
            crate::configuration_editor::model::TypeshedLifecycle::NoSource
        );
        assert!(state
            .no_source_reason
            .is_some_and(|reason| reason.contains("NO SOURCE")
                && reason.contains("0123456789012345678901234567890123456789")));
        let _ = std::fs::remove_dir_all(store);
    }
}
