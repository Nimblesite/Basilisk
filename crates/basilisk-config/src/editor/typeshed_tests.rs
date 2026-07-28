//! Typeshed acquisition-setting persistence tests ([LSPCFGED-TYPESHED]).
//!
//! Cross-references [STUBRES-TYPESHED-CONFIG](docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md)
//! and [CONFIGEDITOR-SOURCES](docs/specs/LSP-CONFIGURATION-EDITOR-SPEC.md);
//! code under test is `editor/patch.rs` (the writer) and `editor/mod.rs` (the
//! validator). Split from `editor/tests.rs` to keep both under the size ceiling.

use std::collections::BTreeMap;

use super::super::{
    build_configuration_patch, discover_config_document_with_content, CacheConfigUpdate,
    ConfigDocumentError, ConfigurationUpdate, RuleConfigUpdate, TypeshedConfigKey,
    TypeshedConfigUpdate,
};
use super::{document_for, set_rule, temp_root};
use crate::RuleSeverity;

/// [LSPCFGED-TYPESHED]: malformed acquisition settings fail before a snapshot
/// can silently reinterpret them as defaults or a different source mode.
#[test]
fn malformed_typeshed_settings_are_invalid() {
    let root = temp_root("bad_typeshed_settings");
    for content in [
        "[tool.basilisk]\ntypeshed-commit = 42\n",
        "[tool.basilisk]\ntypeshed-commit = \"short\"\n",
        "[tool.basilisk]\ntypeshed-store-path = false\n",
        "[tool.basilisk]\ntypeshed-path = \"custom\"\ntypeshed-commit = \"83c2518a9e6abbda0c44592c3483de459198f887\"\n",
    ] {
        let result = discover_config_document_with_content(&root, content.to_owned());
        assert!(
            matches!(result, Err(ConfigDocumentError::Invalid { .. })),
            "must reject {content}"
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// [LSPCFGED-TYPESHED]: Typeshed keys participate in the same atomic,
/// structure-preserving transaction as rule/tag entries.
#[test]
fn typeshed_settings_patch_atomically_and_preserve_project_content() {
    let root = temp_root("typeshed_patch");
    let document = document_for(
        &root,
        "# keep\n[project]\nname = \"demo\"\n\n[tool.basilisk]\n",
    );
    let update = ConfigurationUpdate {
        rules: set_rule("BSK-0001", RuleSeverity::Warning),
        typeshed: TypeshedConfigUpdate {
            entries: BTreeMap::from([
                (
                    TypeshedConfigKey::TypeshedCommit,
                    Some("83c2518a9e6abbda0c44592c3483de459198f887".to_owned()),
                ),
                (
                    TypeshedConfigKey::TypeshedStorePath,
                    Some(".cache/typeshed-store".to_owned()),
                ),
            ]),
        },
        cache: CacheConfigUpdate::default(),
    };
    let patch = build_configuration_patch(&document, &update).unwrap();
    assert!(patch.content.contains("# keep"));
    assert!(patch.content.contains("name = \"demo\""));
    assert!(patch
        .content
        .contains("typeshed-commit = \"83c2518a9e6abbda0c44592c3483de459198f887\""));
    assert!(patch
        .content
        .contains("typeshed-store-path = \".cache/typeshed-store\""));
    assert_eq!(
        patch.config.typeshed_store_path,
        Some(std::path::PathBuf::from(".cache/typeshed-store"))
    );
    assert_eq!(
        patch.config.typeshed_commit.as_deref(),
        Some("83c2518a9e6abbda0c44592c3483de459198f887")
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// [LSPCFGED-TYPESHED]: removing an explicit setting leaves unrelated
/// acquisition settings and comments untouched.
#[test]
fn typeshed_setting_removal_is_allowlisted_and_narrow() {
    let root = temp_root("typeshed_remove");
    let document = document_for(
        &root,
        "[tool.basilisk]\n# keep store\ntypeshed-store-path = \".cache/store\"\ntypeshed-commit = \"83c2518a9e6abbda0c44592c3483de459198f887\"\n",
    );
    let update = ConfigurationUpdate {
        rules: RuleConfigUpdate::default(),
        typeshed: TypeshedConfigUpdate {
            entries: BTreeMap::from([(TypeshedConfigKey::TypeshedCommit, None)]),
        },
        cache: CacheConfigUpdate::default(),
    };
    let patch = build_configuration_patch(&document, &update).unwrap();
    assert!(!patch.content.contains("typeshed-commit"));
    assert!(patch.content.contains("# keep store"));
    assert!(patch.content.contains("typeshed-store-path"));
    assert!(patch.config.typeshed_commit.is_none());
    let _ = std::fs::remove_dir_all(&root);
}
