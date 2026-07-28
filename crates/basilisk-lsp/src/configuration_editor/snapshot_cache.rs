//! Server-described caching state for the Project view ([LSPCFGED-CACHE]).
//!
//! Two layers, both reported: the persistent cross-session result cache
//! ([CHKCACHE]) that `[tool.basilisk]` configures, and the in-session Salsa
//! memo layer ([CHKARCH-INCREMENTAL-SALSA]) that is always on and configures
//! nothing. Every value here is read from live state — the resolved config and
//! the live database — never a client guess about what caching probably does.

use std::path::Path;

use basilisk_config::BasiliskConfig;

use super::model::{CacheConfigurationState, InSessionCacheState, PersistentCacheState};

/// Project both caching layers for one root.
///
/// `tracked_files` is the live count of files the root's Salsa database holds
/// memos for. The folder comes from [`BasiliskConfig::cache_directory`], the
/// same routine `basilisk check` resolves entries through, so the editor and
/// the run can never disagree about where the cache lives.
pub(super) fn cache_configuration(
    config: &BasiliskConfig,
    root: &Path,
    tracked_files: usize,
) -> CacheConfigurationState {
    CacheConfigurationState {
        persistent: PersistentCacheState {
            enabled: config.cache_is_enabled(),
            folder: config
                .cache_directory(root)
                .to_string_lossy()
                .into_owned(),
            folder_configured: config.cache_dir.is_some(),
        },
        in_session: InSessionCacheState {
            tracked_files: super::snapshot::count_i64(tracked_files),
        },
    }
}

#[cfg(test)]
#[path = "snapshot_cache_tests.rs"]
mod tests;
