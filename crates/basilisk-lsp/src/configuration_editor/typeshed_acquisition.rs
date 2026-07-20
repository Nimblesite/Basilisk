//! Root-keyed production acquisition with candidate staging and rollback.

use std::path::Path;
use std::sync::Arc;

use basilisk_config::BasiliskConfig;
use basilisk_stubs::typeshed::snapshot::Snapshot;
use tower_lsp::jsonrpc::Result as LspResult;

use super::protocol::rpc_error;
use super::transaction::ConfigurationRefreshHandles;
use crate::server::typeshed_status::{self, TypeshedFailure, TypeshedGeneration};
use crate::server::LspServer;

/// A fully gated candidate held outside the active root map until the client
/// accepts the matching configuration edit.
pub(super) struct StagedGeneration {
    previous: Option<TypeshedGeneration>,
    candidate: Arc<Snapshot>,
}

pub(super) enum WatchedStageError {
    Busy(&'static str),
    Failed(TypeshedFailure),
}

impl WatchedStageError {
    pub(super) fn rpc_error(&self) -> tower_lsp::jsonrpc::Error {
        match self {
            Self::Busy(message) => rpc_error("typeshedAcquisitionBusy", message),
            Self::Failed(failure) => rpc_error(failure.rpc_code(), failure.reason()),
        }
    }

    pub(super) fn into_failure(self) -> Option<TypeshedFailure> {
        match self {
            Self::Busy(_) => None,
            Self::Failed(failure) => Some(failure),
        }
    }
}

impl StagedGeneration {
    pub(super) fn candidate(&self) -> &Arc<Snapshot> {
        &self.candidate
    }

    pub(super) async fn activate(self, server: &LspServer, root: &Path) {
        let status = self.candidate.status.clone();
        publish_generation(
            &server.typeshed_generations,
            &server.client,
            root,
            TypeshedGeneration::Ready(self.candidate),
        )
        .await;
        typeshed_status::show_high_warnings(&server.client, &status).await;
    }

    pub(super) async fn activate_with(self, handles: &ConfigurationRefreshHandles, root: &Path) {
        let status = self.candidate.status.clone();
        publish_generation(
            &handles.typeshed_generations,
            &handles.client,
            root,
            TypeshedGeneration::Ready(self.candidate),
        )
        .await;
        typeshed_status::show_high_warnings(&handles.client, &status).await;
    }

    pub(super) async fn block(self, server: &LspServer, root: &Path, reason: &str) {
        publish_generation(
            &server.typeshed_generations,
            &server.client,
            root,
            TypeshedGeneration::Blocked {
                failure: TypeshedFailure::acquisition(reason),
            },
        )
        .await;
    }

    pub(super) async fn block_with(
        self,
        handles: &ConfigurationRefreshHandles,
        root: &Path,
        reason: &str,
    ) {
        publish_generation(
            &handles.typeshed_generations,
            &handles.client,
            root,
            TypeshedGeneration::Blocked {
                failure: TypeshedFailure::acquisition(reason),
            },
        )
        .await;
    }

    pub(super) async fn rollback(self, server: &LspServer, root: &Path) {
        let generation = self
            .previous
            .unwrap_or_else(|| TypeshedGeneration::Blocked {
                failure: TypeshedFailure::acquisition("configuration edit was not applied"),
            });
        publish_generation(
            &server.typeshed_generations,
            &server.client,
            root,
            generation,
        )
        .await;
    }
}

/// Stage a candidate only when one of the six source-policy settings changed.
pub(super) async fn stage_configuration_change(
    server: &LspServer,
    root: &Path,
    before: &BasiliskConfig,
    after: &BasiliskConfig,
) -> LspResult<Option<StagedGeneration>> {
    if !typeshed_policy_changed(before, after) {
        return Ok(None);
    }
    let previous = begin_acquiring(server, root).await?;
    if let Some(candidate) = pinned_active_candidate(previous.as_ref(), before, after) {
        return Ok(Some(StagedGeneration {
            previous,
            candidate,
        }));
    }
    match acquire(root, after, after.typeshed_cache.unwrap_or(true)).await {
        Ok(candidate) => Ok(Some(StagedGeneration {
            previous,
            candidate,
        })),
        Err(error) => {
            restore_after_failure(server, root, previous, error.reason()).await;
            Err(rpc_error(error.rpc_code(), error.reason()))
        }
    }
}

/// Reclassify the already-active immutable generation when `Pin current` is
/// the only policy change. The bytes and commit are already gate-accepted, so
/// downloading the identical archive again would add failure modes without
/// changing the selected source.
fn pinned_active_candidate(
    previous: Option<&TypeshedGeneration>,
    before: &BasiliskConfig,
    after: &BasiliskConfig,
) -> Option<Arc<Snapshot>> {
    let requested = after.typeshed_commit.as_deref()?;
    if before.typeshed_commit.is_some()
        || before.typeshed_path.is_some()
        || after.typeshed_path.is_some()
        || before.typeshed_url != after.typeshed_url
        || before.typeshed_cache_path != after.typeshed_cache_path
        || before.typeshed_cache != after.typeshed_cache
        || before.typeshed_verify != after.typeshed_verify
    {
        return None;
    }
    let active = previous.and_then(TypeshedGeneration::ready_snapshot)?;
    if active
        .status
        .commit
        .map(|commit| commit.to_hex())
        .as_deref()
        != Some(requested)
    {
        return None;
    }

    let mut candidate = (**active).clone();
    match &candidate.identity {
        basilisk_stubs::typeshed::source::SourceIdentity::Commit { commit, .. } => {
            candidate.identity = basilisk_stubs::typeshed::source::SourceIdentity::Commit {
                commit: *commit,
                pinned: true,
            };
            candidate.status.active_source =
                basilisk_stubs::typeshed::source::SourceKind::ExactCommit;
            candidate
                .status
                .warnings
                .retain(|warning| warning.code != "UNPINNED");
        }
        basilisk_stubs::typeshed::source::SourceIdentity::Bundled { .. } => {
            candidate.status.active_source = basilisk_stubs::typeshed::source::SourceKind::Bundled;
            candidate.status.warnings.clear();
        }
        basilisk_stubs::typeshed::source::SourceIdentity::Custom { .. } => return None,
    }
    Some(Arc::new(candidate))
}

/// Cache-bypassing one-run refresh used by the closed `AcquireFresh` action.
pub(super) async fn acquire_fresh(
    server: &LspServer,
    root: &Path,
    config: &BasiliskConfig,
) -> Result<StagedGeneration, WatchedStageError> {
    let previous = begin_acquiring_with(&server.typeshed_generations, &server.client, root)
        .await
        .map_err(WatchedStageError::Busy)?;
    match acquire(root, config, false).await {
        Ok(candidate) => Ok(StagedGeneration {
            previous,
            candidate,
        }),
        Err(error) => Err(WatchedStageError::Failed(error)),
    }
}

/// Stage an already-observed on-disk configuration change. Unlike an editor
/// transaction, failure cannot restore the previous generation because the
/// source document has already changed, so the root becomes explicitly
/// blocked and its old snapshot is never rebound to the new policy.
pub(super) async fn stage_watched_configuration_change(
    handles: &ConfigurationRefreshHandles,
    root: &Path,
    before: &BasiliskConfig,
    after: &BasiliskConfig,
) -> Result<Option<StagedGeneration>, WatchedStageError> {
    if !typeshed_policy_changed(before, after) {
        return Ok(None);
    }
    let _previous = begin_acquiring_with(&handles.typeshed_generations, &handles.client, root)
        .await
        .map_err(WatchedStageError::Busy)?;
    acquire(root, after, after.typeshed_cache.unwrap_or(true))
        .await
        .map(|candidate| {
            Some(StagedGeneration {
                previous: None,
                candidate,
            })
        })
        .map_err(WatchedStageError::Failed)
}

pub(super) async fn publish_failure(
    handles: &ConfigurationRefreshHandles,
    root: &Path,
    failure: TypeshedFailure,
) {
    publish_generation(
        &handles.typeshed_generations,
        &handles.client,
        root,
        TypeshedGeneration::Blocked { failure },
    )
    .await;
}

async fn begin_acquiring(server: &LspServer, root: &Path) -> LspResult<Option<TypeshedGeneration>> {
    begin_acquiring_with(&server.typeshed_generations, &server.client, root)
        .await
        .map_err(|message| rpc_error("typeshedAcquisitionBusy", message))
}

async fn begin_acquiring_with(
    generations: &tokio::sync::RwLock<typeshed_status::TypeshedGenerations>,
    client: &tower_lsp::Client,
    root: &Path,
) -> Result<Option<TypeshedGeneration>, &'static str> {
    let acquiring = TypeshedGeneration::Acquiring;
    let previous = {
        let mut generations = generations.write().await;
        replace_with_acquiring(&mut generations, root)?
    };
    typeshed_status::notify_generation(client, root, &acquiring).await;
    Ok(previous)
}

fn replace_with_acquiring(
    generations: &mut typeshed_status::TypeshedGenerations,
    root: &Path,
) -> Result<Option<TypeshedGeneration>, &'static str> {
    if matches!(generations.get(root), Some(TypeshedGeneration::Acquiring)) {
        return Err("Typeshed acquisition is already in progress");
    }
    Ok(generations.insert(root.to_path_buf(), TypeshedGeneration::Acquiring))
}

async fn restore_after_failure(
    server: &LspServer,
    root: &Path,
    previous: Option<TypeshedGeneration>,
    error: &str,
) {
    let generation = previous.unwrap_or_else(|| TypeshedGeneration::Blocked {
        failure: TypeshedFailure::acquisition(error),
    });
    publish_generation(
        &server.typeshed_generations,
        &server.client,
        root,
        generation,
    )
    .await;
}

async fn publish_generation(
    generations: &tokio::sync::RwLock<typeshed_status::TypeshedGenerations>,
    client: &tower_lsp::Client,
    root: &Path,
    generation: TypeshedGeneration,
) {
    let _ = generations
        .write()
        .await
        .insert(root.to_path_buf(), generation.clone());
    typeshed_status::notify_generation(client, root, &generation).await;
}

async fn acquire(
    root: &Path,
    config: &BasiliskConfig,
    use_cache: bool,
) -> Result<Arc<Snapshot>, TypeshedFailure> {
    let workspace = super::watch::workspace_config_for_basilisk(root, config);
    let cache_path = workspace.typeshed_cache_path.clone();
    let mut request =
        crate::config::typeshed_request(&workspace).map_err(TypeshedFailure::acquisition)?;
    request.use_cache = use_cache;
    tokio::task::spawn_blocking(move || {
        let manager = basilisk_stubs::typeshed::runtime::production_manager(request, cache_path)
            .map_err(|error| TypeshedFailure::acquisition(error.to_string()))?;
        manager
            .snapshot()
            .map_err(|error| TypeshedFailure::from_selection(&error))
    })
    .await
    .map_err(|_join_error| TypeshedFailure::acquisition("Typeshed acquisition task failed"))?
}

fn typeshed_policy_changed(before: &BasiliskConfig, after: &BasiliskConfig) -> bool {
    before.typeshed_path != after.typeshed_path
        || before.typeshed_commit != after.typeshed_commit
        || before.typeshed_url != after.typeshed_url
        || before.typeshed_cache_path != after.typeshed_cache_path
        || before.typeshed_cache != after.typeshed_cache
        || before.typeshed_verify != after.typeshed_verify
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{replace_with_acquiring, typeshed_policy_changed, WatchedStageError};
    use crate::server::typeshed_status::{TypeshedFailure, TypeshedGenerations};
    use basilisk_config::BasiliskConfig;
    use basilisk_stubs::typeshed::gittree::Oid;
    use basilisk_stubs::typeshed::selector::{BackendError, SelectionError};

    #[test]
    fn candidate_staging_is_limited_to_the_six_typeshed_settings() {
        let before = BasiliskConfig::default();
        let mut rule_only = before.clone();
        rule_only.python_version = Some("3.12".to_owned());
        assert!(!typeshed_policy_changed(&before, &rule_only));

        let mut source = before.clone();
        source.typeshed_cache = Some(false);
        assert!(typeshed_policy_changed(&before, &source));
    }

    #[test]
    fn concurrent_acquisition_cannot_overwrite_the_active_candidate() {
        let root = Path::new("/workspace");
        let mut generations = TypeshedGenerations::new();
        let first = replace_with_acquiring(&mut generations, root);
        assert!(first.is_ok());
        let second = replace_with_acquiring(&mut generations, root);
        assert_eq!(
            second.err(),
            Some("Typeshed acquisition is already in progress")
        );
    }

    #[test]
    fn license_drift_retains_its_typed_rpc_category() {
        let Ok(commit) = Oid::from_hex("0123456789012345678901234567890123456789") else {
            return;
        };
        let failure = TypeshedFailure::from_selection(&SelectionError::Exact {
            commit,
            reason: BackendError::LicenseChanged,
        });
        assert_eq!(failure.rpc_code(), "typeshedLicenseChanged");
    }

    #[test]
    fn failed_fresh_acquisition_carries_failure_instead_of_a_previous_generation() {
        let error = WatchedStageError::Failed(TypeshedFailure::acquisition("fresh failed"));
        let failure = error.into_failure();
        assert_eq!(
            failure.as_ref().map(TypeshedFailure::reason),
            Some("fresh failed")
        );
    }
}
