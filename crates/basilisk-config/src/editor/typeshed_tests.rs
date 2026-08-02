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
use super::fixtures::{
    assert_project_content_preserved, document_for, set_rule, temp_root, PROJECT_DOCUMENT,
};
use crate::RuleSeverity;

/// [LSPCFGED-TYPESHED]: malformed acquisition settings fail before a snapshot
/// can silently reinterpret them as defaults or a different source mode.
#[test]
fn malformed_typeshed_settings_are_invalid() {
    let root = temp_root("bad_typeshed_settings");
    let commit = "83c2518a9e6abbda0c44592c3483de459198f887";
    let package =
        "micropython-stdlib-stubs@sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    for content in [
        "[tool.basilisk]\ntypeshed-commit = 42\n".to_owned(),
        "[tool.basilisk]\ntypeshed-commit = \"short\"\n".to_owned(),
        "[tool.basilisk]\ntypeshed-store-path = false\n".to_owned(),
        // A package pin is a string like every other source key, and its shape
        // is checked by the one shared parser ([STUBRES-TYPESHED-PYPI]).
        "[tool.basilisk]\ntypeshed-package = 42\n".to_owned(),
        "[tool.basilisk]\ntypeshed-package = \"no-digest\"\n".to_owned(),
        "[tool.basilisk]\ntypeshed-package = \"name@sha256:abc\"\n".to_owned(),
        // Every pairing of the three mutually-exclusive step-3 sources.
        format!("[tool.basilisk]\ntypeshed-path = \"custom\"\ntypeshed-commit = \"{commit}\"\n"),
        format!("[tool.basilisk]\ntypeshed-path = \"custom\"\ntypeshed-package = \"{package}\"\n"),
        format!("[tool.basilisk]\ntypeshed-commit = \"{commit}\"\ntypeshed-package = \"{package}\"\n"),
        format!(
            "[tool.basilisk]\ntypeshed-path = \"custom\"\ntypeshed-commit = \"{commit}\"\ntypeshed-package = \"{package}\"\n"
        ),
    ] {
        let result = discover_config_document_with_content(&root, content.clone());
        assert!(
            matches!(result, Err(ConfigDocumentError::Invalid { .. })),
            "must reject {content}"
        );
    }
    // A lone, well-formed package pin is the valid single-source case — the
    // exclusion check must not reject it.
    assert!(
        discover_config_document_with_content(
            &root,
            format!("[tool.basilisk]\ntypeshed-package = \"{package}\"\n"),
        )
        .is_ok(),
        "a single well-formed package pin is valid configuration"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// [LSPCFGED-TYPESHED]: Typeshed keys participate in the same atomic,
/// structure-preserving transaction as rule/tag entries.
#[test]
fn typeshed_settings_patch_atomically_and_preserve_project_content() {
    let root = temp_root("typeshed_patch");
    let document = document_for(&root, PROJECT_DOCUMENT);
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
    assert_project_content_preserved(&patch.content);
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
