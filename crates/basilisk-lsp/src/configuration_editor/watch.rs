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

use super::transaction::{refresh_after_configuration_change_with, ConfigurationRefreshHandles};

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
    roots: &[PathBuf],
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
    refresh_search_paths(handles, roots).await;
    if let Err(error) = refresh_after_configuration_change_with(handles, root, reason).await {
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
    refresh_search_paths(handles, roots).await;
    recheck_and_publish(handles).await;
}

/// Rebuild the package registry and import search paths from the current
/// on-disk configuration and cache them on the index ([ANALYSIS-INCR-IMPORTS]).
async fn refresh_search_paths(handles: &ConfigurationRefreshHandles, roots: &[PathBuf]) {
    let guard = handles.index.read().await;
    let Some(index) = guard.as_ref() else { return };
    let config = roots
        .first()
        .map(|root| crate::config::load_config(root))
        .unwrap_or_default();
    let registry = crate::server::init::build_uv_registry(roots);
    let search_paths = crate::import_resolver::search_paths_from_config(roots, &config, registry);
    index.set_search_paths(search_paths);
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
