//! Server-owned configuration and environment watcher.
//!
//! Implements [LSPARCH-CONFIG]. See
//! docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG.
//!
//! The LSP never relies on clients to observe configuration changes: some
//! clients cannot watch files at all (Zed advertises no file watchers —
//! docs/specs/ZED-SPEC.md), and external tools edit configuration behind
//! every client. The server polls each workspace root's configuration
//! sources itself — the active config (`pyproject.toml`) plus the
//! environment sources `uv.lock` and `.python-version` ([LSPUV-WATCHERS]) —
//! and, on any content change, runs the shared refresh tail — reload →
//! recheck → republish → `basilisk/configurationChanged` — so diagnostics
//! and configuration UIs update in real time on every IDE.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use basilisk_config::active_config_path;
use tokio::sync::RwLock;

use super::transaction::ConfigurationRefreshHandles;

/// Poll interval for the server-owned configuration watcher (milliseconds).
/// A few small-file reads per root per tick — cheap enough to feel real-time.
pub(crate) const CONFIG_WATCH_POLL_MS: u64 = 250;

/// Environment sources watched per root alongside the active configuration:
/// they drive the package registry and target Python version ([LSPUV-WATCHERS]).
const ENVIRONMENT_SOURCES: [&str; 2] = ["uv.lock", ".python-version"];

/// Spawn the watcher task. Returns its abort handle for shutdown.
///
/// The task seeds a baseline from the sources already loaded at
/// initialization, then refreshes any root whose on-disk content later
/// differs — regardless of how the change arrived.
pub(crate) fn spawn_configuration_watcher(
    handles: ConfigurationRefreshHandles,
    roots: Arc<RwLock<Vec<PathBuf>>>,
) -> tokio::task::AbortHandle {
    let task = tokio::spawn(async move {
        seed_baselines(&handles, &roots).await;
        loop {
            tokio::time::sleep(Duration::from_millis(CONFIG_WATCH_POLL_MS)).await;
            let current_roots = roots.read().await.clone();
            for root in &current_roots {
                refresh_root_from_disk(&handles, &current_roots, root, "externalEdit").await;
                refresh_environment_from_disk(&handles, &current_roots, root).await;
            }
        }
    });
    task.abort_handle()
}

/// Record the source content already loaded during initialization so the
/// first poll tick does not refresh a workspace that has not changed.
async fn seed_baselines(handles: &ConfigurationRefreshHandles, roots: &RwLock<Vec<PathBuf>>) {
    let current_roots = roots.read().await.clone();
    for root in current_roots {
        for path in watched_sources(&root) {
            let content = read_source_content(&path).await;
            handles
                .configuration_editor
                .seed_disk_content(&path, content);
        }
    }
}

/// Every file the server watches for one root: the active configuration
/// source first, then the environment sources.
fn watched_sources(root: &Path) -> Vec<PathBuf> {
    let mut sources = vec![active_config_path(root)];
    sources.extend(ENVIRONMENT_SOURCES.iter().map(|name| root.join(name)));
    sources
}

/// Refresh one root when its active configuration content changed on disk.
///
/// The shared baseline in `ConfigurationEditorState` means the poll loop and
/// the client `didChangeWatchedFiles` path observe the same state: whichever
/// sees a disk change first refreshes, and the other becomes a no-op. A
/// config change also refreshes the import search paths before the recheck —
/// `stub-paths`, `typeshed-path`, and dependency edits take effect in the
/// same single recheck ([ANALYSIS-INCR-IMPORTS]).
pub(crate) async fn refresh_root_from_disk(
    handles: &ConfigurationRefreshHandles,
    _roots: &[PathBuf],
    root: &Path,
    reason: &str,
) {
    let path = active_config_path(root);
    let content = read_source_content(&path).await;
    if !handles
        .configuration_editor
        .record_disk_content(&path, &content)
    {
        return;
    }
    tracing::info!(root = %root.display(), reason, "configuration source changed on disk");
    let before = handles
        .index
        .read()
        .await
        .as_ref()
        .and_then(|index| index.root_configs.get(root))
        .cloned()
        .unwrap_or_default();
    let document = handles.configuration_editor.effective_document(root);
    let result = match document {
        Ok(document) => {
            match super::typeshed_acquisition::stage_watched_configuration_change(
                handles,
                root,
                &before,
                &document.config,
            )
            .await
            {
                Ok(Some(staged)) => {
                    let refreshed = super::transaction::refresh_with_document_and_typeshed(
                        handles,
                        root,
                        reason,
                        &document,
                        Some(staged.candidate()),
                    )
                    .await;
                    match refreshed {
                        Ok(()) => {
                            staged.activate_with(handles, root).await;
                            Ok(())
                        }
                        Err(error) => {
                            let cleanup = super::transaction::refresh_with_document(
                                handles,
                                root,
                                "typeshedWatchedConfigurationBlocked",
                                &document,
                            )
                            .await;
                            staged
                                .block_with(handles, root, "configuration refresh failed")
                                .await;
                            if let Err(cleanup_error) = cleanup {
                                tracing::warn!(root = %root.display(), error = %cleanup_error, "failed to clear analysis after watched Typeshed activation failure");
                            }
                            Err(error)
                        }
                    }
                }
                Ok(None) => {
                    super::transaction::refresh_with_document(handles, root, reason, &document)
                        .await
                }
                Err(error) => {
                    let rpc_error = error.rpc_error();
                    let refresh =
                        super::transaction::refresh_with_document(handles, root, reason, &document)
                            .await;
                    if let Some(failure) = error.into_failure() {
                        super::typeshed_acquisition::publish_failure(handles, root, failure).await;
                    }
                    refresh.and(Err(rpc_error))
                }
            }
        }
        Err(error) => Err(super::protocol::config_error(error)),
    };
    if let Err(error) = result {
        // Malformed mid-write content is retried on the next observed change;
        // the recorded baseline prevents a hot refresh loop.
        tracing::debug!(
            root = %root.display(),
            %error,
            "configuration refresh deferred: source not currently valid"
        );
    }
}

/// Refresh one root when an environment source (`uv.lock`,
/// `.python-version`) changed on disk: reload root configs for a Python
/// version change, rebuild the package registry and import search paths, and
/// recheck. Mirrors the client-watcher path ([LSPUV-WATCHERS]) so clients
/// without file watchers behave identically.
pub(crate) async fn refresh_environment_from_disk(
    handles: &ConfigurationRefreshHandles,
    roots: &[PathBuf],
    root: &Path,
) {
    let mut environment_changed = false;
    let mut python_version_changed = false;
    for name in ENVIRONMENT_SOURCES {
        let path = root.join(name);
        let content = read_source_content(&path).await;
        if handles
            .configuration_editor
            .record_disk_content(&path, &content)
        {
            environment_changed = true;
            python_version_changed |= name == ".python-version";
            tracing::info!(root = %root.display(), source = name, "environment source changed on disk");
        }
    }
    if !environment_changed {
        return;
    }
    if python_version_changed {
        let mut guard = handles.index.write().await;
        if let Some(index) = guard.as_mut() {
            index.reload_root_configs();
        }
    }
    refresh_search_paths(handles, roots, None).await;
    recheck_and_publish(handles).await;
}

/// Rebuild the package registry and import search paths from the current
/// on-disk configuration and cache them on the index ([ANALYSIS-INCR-IMPORTS]).
pub(super) async fn refresh_search_paths(
    handles: &ConfigurationRefreshHandles,
    roots: &[PathBuf],
    document: Option<&basilisk_config::ConfigDocument>,
) {
    refresh_search_paths_with_typeshed(handles, roots, document, None).await;
}

pub(super) async fn refresh_search_paths_with_typeshed(
    handles: &ConfigurationRefreshHandles,
    roots: &[PathBuf],
    document: Option<&basilisk_config::ConfigDocument>,
    candidate: Option<&Arc<basilisk_stubs::typeshed::snapshot::Snapshot>>,
) {
    let guard = handles.index.read().await;
    let Some(index) = guard.as_ref() else { return };
    let configs: Vec<_> = roots
        .iter()
        .map(|root| {
            let config = document
                .filter(|document| document.root == *root)
                .map_or_else(
                    || {
                        index.root_configs.get(root).map_or_else(
                            || crate::config::load_config(root),
                            |config| workspace_config_for_basilisk(root, config),
                        )
                    },
                    |document| workspace_config_for_basilisk(root, &document.config),
                );
            (root.clone(), config)
        })
        .collect();
    let generations = handles.typeshed_generations.read().await;
    let bindings = configs
        .iter()
        .filter_map(|(root, root_config)| {
            let snapshot = document
                .filter(|document| document.root == *root)
                .and(candidate)
                .cloned()
                .or_else(|| generations.get(root)?.ready_snapshot().cloned())?;
            let target = crate::import_resolver::stub_target_from_config(root_config);
            Some((root.clone(), snapshot, target))
        })
        .collect();
    let active_typeshed = crate::import_resolver::ActiveTypeshed::from_roots(bindings);
    drop(generations);
    let interpreter = handles.python_interpreter.read().await.clone();
    let search_paths = configs
        .into_iter()
        .map(|(root, config)| {
            let search_paths = crate::server::init::build_root_search_paths(
                roots,
                &root,
                config,
                interpreter.as_deref(),
                active_typeshed.clone(),
            );
            (root, Arc::new(search_paths))
        })
        .collect();
    index.set_search_paths_by_root(search_paths);
}

pub(super) fn workspace_config_for_basilisk(
    root: &Path,
    basilisk: &basilisk_config::BasiliskConfig,
) -> crate::config::WorkspaceConfig {
    let mut config = crate::config::load_config(root);
    if config.python_version.is_none() {
        config.python_version = basilisk_uv::python_version::resolve_target_python_version(root);
    }
    config.stub_paths = basilisk
        .stub_paths
        .iter()
        .map(|path| {
            if path.is_absolute() {
                path.clone()
            } else {
                root.join(path)
            }
        })
        .collect();
    config.typeshed_path = basilisk.typeshed_path.as_ref().map(|path| {
        if path.is_absolute() {
            path.clone()
        } else {
            root.join(path)
        }
    });
    config.typeshed_commit.clone_from(&basilisk.typeshed_commit);
    config.typeshed_url.clone_from(&basilisk.typeshed_url);
    config.typeshed_cache_path = basilisk.typeshed_cache_path.as_ref().map(|path| {
        if path.is_absolute() {
            path.clone()
        } else {
            root.join(path)
        }
    });
    config.typeshed_cache = basilisk.typeshed_cache.unwrap_or(true);
    config.typeshed_verify = basilisk.typeshed_verify.unwrap_or(true);
    if basilisk.python_version.is_some() {
        config.python_version.clone_from(&basilisk.python_version);
    }
    if basilisk.python_platform.is_some() {
        config.python_platform.clone_from(&basilisk.python_platform);
    }
    config
}

/// Recheck every indexed file and publish the diagnostics that changed.
async fn recheck_and_publish(handles: &ConfigurationRefreshHandles) {
    let results = {
        let guard = handles.index.read().await;
        let Some(index) = guard.as_ref() else { return };
        index.reresolve_imports_and_recheck()
    };
    for (uri, diagnostics) in results {
        crate::server::publish_diagnostics_gated(
            &handles.client,
            &handles.type_checking_enabled,
            &handles.analyze_enabled,
            uri,
            diagnostics,
        )
        .await;
    }
}

/// A watched source's current bytes; a missing or unreadable file is the
/// empty content (deleting a source is itself a change).
async fn read_source_content(path: &Path) -> String {
    tokio::fs::read_to_string(path).await.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use basilisk_config::BasiliskConfig;

    use super::workspace_config_for_basilisk;

    #[test]
    fn editor_refresh_preserves_discovered_target_evidence() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let root = std::env::temp_dir().join(format!(
            "basilisk_config_editor_target_{}_{}",
            std::process::id(),
            unique
        ));
        let setup = std::fs::create_dir_all(&root)
            .and_then(|()| std::fs::write(root.join(".python-version"), "3.11\n"));
        assert!(setup.is_ok(), "fixture setup failed: {setup:?}");
        if setup.is_err() {
            return;
        }

        let discovered = workspace_config_for_basilisk(&root, &BasiliskConfig::default());
        assert_eq!(discovered.python_version.as_deref(), Some("3.11"));

        let explicit = BasiliskConfig {
            python_version: Some("3.12".to_owned()),
            ..BasiliskConfig::default()
        };
        let overridden = workspace_config_for_basilisk(&root, &explicit);
        assert_eq!(overridden.python_version.as_deref(), Some("3.12"));
        let _ = std::fs::remove_dir_all(root);
    }
}
