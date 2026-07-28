//! Caching portion of the generated configuration-editor wire model.
//!
//! Implements [LSPCFGED-CACHE]. Basilisk caches on two layers and the editor
//! describes BOTH, because a configuration surface that mentions only one of
//! them reads as "this is all the caching there is":
//!
//! - the **persistent** cross-session result cache ([CHKCACHE]) — two
//!   `[tool.basilisk]` keys, editable here;
//! - the **in-session** Salsa memo layer ([CHKARCH-INCREMENTAL-SALSA]) — always
//!   on, no key at all, reported read-only so its absence from the config file
//!   is a stated fact rather than an omission.

use serde::{Deserialize, Serialize};

use super::CacheSettingKey;

/// The persistent, cross-session result cache ([CHKCACHE]).
///
/// `folder` is the effective location the next run will use — the configured
/// `cache-dir` or the default — resolved by
/// [`BasiliskConfig::cache_directory`](basilisk_config::BasiliskConfig::cache_directory),
/// the same routine the CLI writes entries through, so the editor can never
/// display a folder the run does not use. `folder_configured` distinguishes
/// "the default, shown for information" from "a folder this project chose",
/// which is the only thing a reset control needs to know.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistentCacheState {
    pub enabled: bool,
    pub folder: String,
    pub folder_configured: bool,
}

/// The in-session incremental engine ([CHKARCH-INCREMENTAL-SALSA]).
///
/// There is nothing to configure and nothing to switch off: `parse → resolve →
/// check` is one memoized query per file and an edit re-executes only the
/// affected file's query. The one real number is how many files the live
/// database currently tracks; the copy that explains the layer is client
/// presentation, exactly as it is for Typeshed ([LSPCFGED-TYPESHED]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InSessionCacheState {
    pub tracked_files: i64,
}

/// Both caching layers in one projection ([LSPCFGED-CACHE]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheConfigurationState {
    pub persistent: PersistentCacheState,
    pub in_session: InSessionCacheState,
}

/// One exact persisted cache-setting change, in the preview's own vocabulary.
/// Values are rendered TOML text (`"true"` / `"false"` for `cache`, the path
/// for `cache-dir`); `None` on a side means the key is absent there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheSettingChange {
    pub key: CacheSettingKey,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}
