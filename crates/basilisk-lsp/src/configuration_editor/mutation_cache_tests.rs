//! Cache-setting mutation validation and preview projection ([LSPCFGED-CACHE]).
//!
//! Code under test is `mutation.rs`; the keys it writes are specified in
//! [CHKCACHE-CONFIG](../../../../docs/specs/CHECKER-CACHE-SPEC.md). Split from
//! `mutation_tests.rs` to keep both files under the repository size ceiling.

use std::path::PathBuf;

use basilisk_config::{BasiliskConfig, CacheConfigMutation};

use super::super::{build_update, resolved_cache_changes};
use super::error_kind;
use crate::configuration_editor::catalog::descriptors;
use crate::configuration_editor::model::{CacheSettingKey, EditorMutation};

/// [LSPCFGED-CACHE]: the cache keys fold into the same validated update as
/// rule and Typeshed mutations, and `cache` becomes a real TOML boolean rather
/// than the string the wire carried it as.
#[test]
fn cache_setting_mutations_fold_into_the_update() {
    let catalog = descriptors();
    let update = build_update(
        &[
            EditorMutation::SetCacheSetting {
                key: CacheSettingKey::CacheEnabled,
                value: "true".to_owned(),
            },
            EditorMutation::SetCacheSetting {
                key: CacheSettingKey::CacheDir,
                value: "build/bsk".to_owned(),
            },
            EditorMutation::RemoveCacheSetting {
                key: CacheSettingKey::CacheDir,
            },
        ],
        &catalog,
    )
    .expect("valid cache mutations must build an update");
    assert_eq!(
        update.cache.mutations,
        vec![
            CacheConfigMutation::SetEnabled(true),
            CacheConfigMutation::SetDir("build/bsk".to_owned()),
            CacheConfigMutation::RemoveDir,
        ],
        "request order must be preserved so the last write wins"
    );
}

/// [LSPCFGED-CACHE]: `cache` is a boolean. A value the TOML parser would drop
/// is a request error, never a silently-ignored write.
#[test]
fn cache_setting_values_are_strictly_validated() {
    let catalog = descriptors();
    for mutation in [
        EditorMutation::SetCacheSetting {
            key: CacheSettingKey::CacheEnabled,
            value: "yes".to_owned(),
        },
        EditorMutation::SetCacheSetting {
            key: CacheSettingKey::CacheEnabled,
            value: "True".to_owned(),
        },
        EditorMutation::SetCacheSetting {
            key: CacheSettingKey::CacheDir,
            value: "   ".to_owned(),
        },
    ] {
        let result = build_update(&[mutation], &catalog);
        assert!(result.is_err(), "invalid cache value must fail");
        let Some(error) = result.err() else { continue };
        assert_eq!(
            error_kind(&error),
            Some(serde_json::json!("invalidCacheSetting"))
        );
    }
    for value in ["true", "false"] {
        let _accepted = build_update(
            &[EditorMutation::SetCacheSetting {
                key: CacheSettingKey::CacheEnabled,
                value: value.to_owned(),
            }],
            &catalog,
        )
        .expect("the two canonical boolean spellings are accepted");
    }
}

/// [LSPCFGED-CACHE]: the preview names exactly the cache keys that move, with
/// the persisted text on each side and nothing for keys that stay put.
#[test]
fn cache_changes_report_only_what_actually_moves() {
    let before = BasiliskConfig {
        cache_enabled: Some(false),
        cache_dir: Some(PathBuf::from("build/bsk")),
        ..BasiliskConfig::default()
    };
    let after = BasiliskConfig {
        cache_enabled: Some(true),
        cache_dir: Some(PathBuf::from("build/bsk")),
        ..BasiliskConfig::default()
    };
    let changes = resolved_cache_changes(&before, &after);
    assert_eq!(
        changes
            .iter()
            .map(|change| (
                change.key,
                change.before.as_deref(),
                change.after.as_deref()
            ))
            .collect::<Vec<_>>(),
        vec![(CacheSettingKey::CacheEnabled, Some("false"), Some("true"))],
        "only `cache` moved"
    );

    assert!(
        resolved_cache_changes(&before, &before).is_empty(),
        "an unchanged configuration reports no cache changes"
    );
}

/// [LSPCFGED-CACHE]: removing a key is reported as a change to "no value", so
/// a reset is visible in the preview rather than looking like a no-op.
#[test]
fn cache_key_removal_is_reported_as_a_change() {
    let before = BasiliskConfig {
        cache_dir: Some(PathBuf::from("build/bsk")),
        ..BasiliskConfig::default()
    };
    let changes = resolved_cache_changes(&before, &BasiliskConfig::default());
    assert_eq!(
        changes
            .iter()
            .map(|change| (
                change.key,
                change.before.as_deref(),
                change.after.as_deref()
            ))
            .collect::<Vec<_>>(),
        vec![(CacheSettingKey::CacheDir, Some("build/bsk"), None)],
        "a removed key must read as a change to no value"
    );
}
