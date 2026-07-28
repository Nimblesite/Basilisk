//! Persistent result-cache persistence tests ([CHKCACHE-CONFIG]).
//!
//! Cross-references [CHKCACHE-CONFIG](docs/specs/CHECKER-CACHE-SPEC.md) and
//! [CONFIGEDITOR-SOURCES](docs/specs/LSP-CONFIGURATION-EDITOR-SPEC.md); code
//! under test is `editor/patch.rs` (the writer) and `editor/mod.rs` (the
//! validator). Split from `editor/tests.rs` to keep both under the size ceiling.

use super::super::{
    build_configuration_patch, discover_config_document_with_content, CacheConfigMutation,
    CacheConfigUpdate, ConfigDocumentError, ConfigurationUpdate,
};
use super::{document_for, temp_root};

fn cache_update(mutations: Vec<CacheConfigMutation>) -> ConfigurationUpdate {
    ConfigurationUpdate {
        cache: CacheConfigUpdate { mutations },
        ..ConfigurationUpdate::default()
    }
}

/// [CHKCACHE-CONFIG]: the cache keys ride the same atomic,
/// structure-preserving transaction as rule/tag and Typeshed entries, and are
/// written with the TOML types the parser actually reads.
#[test]
fn cache_settings_patch_with_their_documented_toml_types() {
    let root = temp_root("cache_patch");
    let document = document_for(
        &root,
        "# keep\n[project]\nname = \"demo\"\n\n[tool.basilisk]\n",
    );
    let update = cache_update(vec![
        CacheConfigMutation::SetEnabled(true),
        CacheConfigMutation::SetDir("build/bsk-cache".to_owned()),
    ]);
    let patch = build_configuration_patch(&document, &update).unwrap();
    assert!(patch.content.contains("# keep"));
    assert!(patch.content.contains("name = \"demo\""));
    assert!(
        patch.content.contains("cache = true"),
        "`cache` must render as a TOML boolean, not a string: {}",
        patch.content
    );
    assert!(patch.content.contains("cache-dir = \"build/bsk-cache\""));
    assert_eq!(patch.config.cache_enabled, Some(true));
    assert_eq!(
        patch.config.cache_dir,
        Some(std::path::PathBuf::from("build/bsk-cache"))
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// [CHKCACHE-CONFIG]: turning the cache off writes an explicit `cache = false`
/// — the same "always write an explicit entry" rule the rule controls follow.
#[test]
fn disabling_the_cache_writes_an_explicit_false() {
    let root = temp_root("cache_off");
    let document = document_for(&root, "[tool.basilisk]\ncache = true\n");
    let patch = build_configuration_patch(
        &document,
        &cache_update(vec![CacheConfigMutation::SetEnabled(false)]),
    )
    .unwrap();
    assert!(patch.content.contains("cache = false"));
    assert_eq!(patch.config.cache_enabled, Some(false));
    let _ = std::fs::remove_dir_all(&root);
}

/// [CHKCACHE-CONFIG]: removing `cache-dir` restores the default location and
/// leaves every unrelated key and comment exactly where it was.
#[test]
fn cache_directory_removal_is_narrow() {
    let root = temp_root("cache_dir_remove");
    let document = document_for(
        &root,
        "[tool.basilisk]\n# keep me\ncache = true\ncache-dir = \"build/bsk-cache\"\n",
    );
    let patch = build_configuration_patch(
        &document,
        &cache_update(vec![CacheConfigMutation::RemoveDir]),
    )
    .unwrap();
    assert!(!patch.content.contains("cache-dir"));
    assert!(patch.content.contains("# keep me"));
    assert!(patch.content.contains("cache = true"));
    assert!(patch.config.cache_dir.is_none());
    assert_eq!(patch.config.cache_enabled, Some(true));
    let _ = std::fs::remove_dir_all(&root);
}

/// [CHKCACHE-CONFIG]: removing `cache` drops the key entirely, so the project
/// states no preference again rather than pinning the default as an entry.
#[test]
fn cache_enable_removal_drops_the_key() {
    let root = temp_root("cache_enable_remove");
    let document = document_for(&root, "[tool.basilisk]\ncache = true\n");
    let patch = build_configuration_patch(
        &document,
        &cache_update(vec![CacheConfigMutation::RemoveEnabled]),
    )
    .unwrap();
    assert!(!patch.content.contains("cache ="));
    assert!(patch.config.cache_enabled.is_none());
    let _ = std::fs::remove_dir_all(&root);
}

/// [CHKCACHE-CONFIG]: later writes win over earlier ones for the same key,
/// exactly as a repeated TOML assignment would.
#[test]
fn later_cache_mutations_win_over_earlier_ones() {
    let root = temp_root("cache_last_wins");
    let document = document_for(&root, "[tool.basilisk]\n");
    let patch = build_configuration_patch(
        &document,
        &cache_update(vec![
            CacheConfigMutation::SetEnabled(true),
            CacheConfigMutation::SetEnabled(false),
        ]),
    )
    .unwrap();
    assert_eq!(patch.config.cache_enabled, Some(false));
    let _ = std::fs::remove_dir_all(&root);
}

/// [CHKCACHE-CONFIG]: a hand-written `cache = "yes"` is refused outright. The
/// parser would silently drop it, so the editor must not present a cache
/// setting the checker will never honour.
#[test]
fn wrongly_typed_cache_keys_are_rejected_as_invalid_documents() {
    let root = temp_root("cache_bad_type");
    for (content, expected) in [
        (
            "[tool.basilisk]\ncache = \"yes\"\n",
            "`cache` must be a boolean",
        ),
        (
            "[tool.basilisk]\ncache-dir = 7\n",
            "`cache-dir` must be a string",
        ),
        (
            "[tool.basilisk]\ncache-dir = \"  \"\n",
            "`cache-dir` must name a directory",
        ),
    ] {
        // Read the rejection reason out rather than branching on it, so a
        // document that is wrongly ACCEPTED fails on the same assertion as one
        // rejected for the wrong reason.
        let reason = match discover_config_document_with_content(&root, content.to_owned()) {
            Err(ConfigDocumentError::Invalid { message, .. }) => message,
            Err(other) => format!("wrong error: {other}"),
            Ok(_) => "accepted".to_owned(),
        };
        assert_eq!(reason, expected, "for {content:?}");
    }
    let _ = std::fs::remove_dir_all(&root);
}
