//! Tests for the server-described caching projection ([LSPCFGED-CACHE]).
//!
//! Code under test is `snapshot_cache.rs`; the configuration keys it reads are
//! specified in [CHKCACHE-CONFIG] (docs/specs/CHECKER-CACHE-SPEC.md) and the
//! in-session layer it reports in
//! [CHKARCH-INCREMENTAL-SALSA](../../../../docs/specs/CHECKER-ARCHITECTURE-SPEC.md).

use std::path::{Path, PathBuf};

use super::*;

fn root() -> PathBuf {
    Path::new("/projects").join("demo")
}

/// [LSPCFGED-CACHE]: with nothing configured the panel shows the cache off and
/// the default folder, so a reader learns where entries WOULD go without
/// having to enable it first.
#[test]
fn unconfigured_project_reports_the_default_folder_and_a_disabled_cache() {
    let state = cache_configuration(&BasiliskConfig::default(), &root(), 0);
    assert!(!state.persistent.enabled);
    assert!(
        !state.persistent.folder_configured,
        "the default folder is not a project choice"
    );
    assert_eq!(
        state.persistent.folder,
        root()
            .join(".basilisk")
            .join("cache")
            .join("check")
            .to_string_lossy()
    );
}

/// [LSPCFGED-CACHE]: the folder shown is the folder the run resolves, relative
/// paths included — the editor must never display a location `basilisk check`
/// would not use.
#[test]
fn configured_folder_is_reported_resolved_and_marked_as_a_project_choice() {
    let config = BasiliskConfig {
        cache_enabled: Some(true),
        cache_dir: Some(PathBuf::from("build/bsk")),
        ..BasiliskConfig::default()
    };
    let state = cache_configuration(&config, &root(), 0);
    assert!(state.persistent.enabled);
    assert!(state.persistent.folder_configured);
    assert_eq!(
        state.persistent.folder,
        root().join("build").join("bsk").to_string_lossy(),
        "the reported folder must match BasiliskConfig::cache_directory"
    );
}

/// [CHKCACHE-CONFIG]: `cache = false` and an unwritten key both report a
/// disabled cache — the difference is a config-file fact, not an effect.
#[test]
fn explicit_false_and_unset_both_report_disabled() {
    let explicit = BasiliskConfig {
        cache_enabled: Some(false),
        ..BasiliskConfig::default()
    };
    assert!(!cache_configuration(&explicit, &root(), 0).persistent.enabled);
    assert!(
        !cache_configuration(&BasiliskConfig::default(), &root(), 0)
            .persistent
            .enabled
    );
}

/// [LSPCFGED-CACHE]: the in-session layer reports the live memo count. It has
/// no key to show, so this number is the only evidence it is running.
#[test]
fn in_session_layer_reports_the_live_tracked_file_count() {
    let state = cache_configuration(&BasiliskConfig::default(), &root(), 42);
    assert_eq!(state.in_session.tracked_files, 42);
}
