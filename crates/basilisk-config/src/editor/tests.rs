//! Editor persistence tests.
//!
//! Cross-references [CONFIGEDITOR-SOURCES] (docs/specs/LSP-CONFIGURATION-EDITOR-SPEC.md)
//! and [CHKARCH-CONFIG-MODEL] (docs/specs/CHECKER-ARCHITECTURE-SPEC.md); code
//! under test is `editor/mod.rs`, `editor/patch.rs`, `editor/write.rs`.

use std::collections::BTreeMap;
use std::path::Path;

use super::{
    active_config_path, apply_config_patch, build_rule_patch, content_revision,
    discover_config_document, discover_config_document_with_content, ConfigDocument,
    ConfigDocumentError, ConfigPatch, RuleConfigUpdate,
};
use crate::{BasiliskConfig, RuleSeverity};

fn temp_root(unique: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("bsk_editor_{unique}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn document_for(root: &Path, content: &str) -> ConfigDocument {
    discover_config_document_with_content(root, content.to_owned()).unwrap()
}

fn set_rule(code: &str, severity: RuleSeverity) -> RuleConfigUpdate {
    RuleConfigUpdate {
        rules: BTreeMap::from([(code.to_owned(), Some(severity))]),
        rule_tags: BTreeMap::new(),
    }
}

fn remove_rule(code: &str) -> RuleConfigUpdate {
    RuleConfigUpdate {
        rules: BTreeMap::from([(code.to_owned(), None)]),
        rule_tags: BTreeMap::new(),
    }
}

fn set_tag(tag: &str, severity: RuleSeverity) -> RuleConfigUpdate {
    RuleConfigUpdate {
        rules: BTreeMap::new(),
        rule_tags: BTreeMap::from([(tag.to_owned(), Some(severity))]),
    }
}

/// [CONFIGEDITOR-SOURCES]: the active source is always the root's
/// `pyproject.toml`, existing or as creation target.
#[test]
fn active_source_is_root_pyproject() {
    let root = temp_root("active_source");
    assert_eq!(active_config_path(&root), root.join("pyproject.toml"));
    let document = discover_config_document(&root).unwrap();
    assert!(!document.exists);
    assert!(!document.config.has_config_table());
    let _ = std::fs::remove_dir_all(&root);
}

/// [CONFIGEDITOR-SOURCES]: a legacy `basilisk.json` is never read.
#[test]
fn basilisk_json_is_never_read() {
    let root = temp_root("json_ignored");
    std::fs::write(
        root.join("basilisk.json"),
        r#"{ "rules": { "BSK-0001": "error" } }"#,
    )
    .unwrap();
    let document = discover_config_document(&root).unwrap();
    assert!(!document.config.has_config_table());
    let _ = std::fs::remove_dir_all(&root);
}

/// [CONFIGEDITOR-OPERATIONS]: malformed configuration is an error, never a
/// silent fallback to defaults.
#[test]
fn malformed_severity_is_invalid() {
    let root = temp_root("bad_severity");
    let result = discover_config_document_with_content(
        &root,
        "[tool.basilisk.rules]\n\"BSK-0001\" = \"loud\"\n".to_owned(),
    );
    assert!(matches!(result, Err(ConfigDocumentError::Invalid { .. })));
    let result = discover_config_document_with_content(
        &root,
        "[tool.basilisk.rule-tags]\n\"basilisk\" = 3\n".to_owned(),
    );
    assert!(matches!(result, Err(ConfigDocumentError::Invalid { .. })));
    let _ = std::fs::remove_dir_all(&root);
}

/// [CHKARCH-CONFIG-MODEL]: `SetRule` writes one explicit per-rule entry.
#[test]
fn set_rule_writes_entry() {
    let root = temp_root("set_rule");
    let document = document_for(&root, "");
    let patch = build_rule_patch(&document, &set_rule("BSK-0050", RuleSeverity::Error)).unwrap();
    assert!(patch.content.contains("[tool.basilisk.rules]"));
    assert!(patch.content.contains("BSK-0050 = \"error\""));
    assert_eq!(
        patch.config.resolve_severity("BSK-0050", &["basilisk"]),
        Some(RuleSeverity::Error)
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// [CHKARCH-CONFIG-MODEL]: `SetTag` writes one explicit tag entry — e.g. the
/// two-line strict-by-default seed ([LSPARCH-CONFIG-SEEDING]).
#[test]
fn set_tag_writes_entry() {
    let root = temp_root("set_tag");
    let document = document_for(&root, "");
    let patch = build_rule_patch(&document, &set_tag("basilisk", RuleSeverity::Error)).unwrap();
    assert!(patch.content.contains("[tool.basilisk.rule-tags]"));
    assert!(patch.content.contains("basilisk = \"error\""));
    assert_eq!(
        patch
            .config
            .nearest_tables()
            .unwrap()
            .rule_tags
            .get("basilisk")
            .copied(),
        Some(RuleSeverity::Error)
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// [CONFIGEDITOR-SOURCES]: removing every entry leaves an explicitly empty
/// table — never pruned, because a missing table would re-arm the seed.
#[test]
fn removing_last_entry_keeps_empty_table() {
    let root = temp_root("keep_empty");
    let document = document_for(&root, "[tool.basilisk.rules]\n\"BSK-0050\" = \"error\"\n");
    let patch = build_rule_patch(&document, &remove_rule("BSK-0050")).unwrap();
    assert!(
        patch.content.contains("[tool.basilisk.rules]"),
        "an emptied table must survive: {}",
        patch.content
    );
    assert!(!patch.content.contains("BSK-0050"));
    assert!(
        patch.config.has_config_table(),
        "the empty table still exists and still blocks re-seeding"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// [CONFIGEDITOR-SOURCES]: TOML edits preserve unrelated content, comments,
/// ordering, and newline style.
#[test]
fn patch_preserves_unrelated_content_and_comments() {
    let root = temp_root("preserve");
    let content = "[project]\nname = \"demo\" # important\n\n\
                   [tool.basilisk.rules]\n# reviewed 2026-01\n\"imports_unresolved\" = \"warning\"\n";
    let document = document_for(&root, content);
    let patch = build_rule_patch(&document, &set_rule("BSK-0001", RuleSeverity::Info)).unwrap();
    assert!(patch.content.contains("name = \"demo\" # important"));
    assert!(patch.content.contains("# reviewed 2026-01"));
    assert!(patch
        .content
        .contains("\"imports_unresolved\" = \"warning\""));
    assert!(patch.content.contains("BSK-0001 = \"info\""));
    let _ = std::fs::remove_dir_all(&root);
}

/// [CONFIGEDITOR-SOURCES]: CRLF documents stay CRLF after a patch.
#[test]
fn patch_preserves_crlf_newlines() {
    let root = temp_root("crlf");
    let document = document_for(
        &root,
        "[tool.basilisk.rules]\r\n\"BSK-0001\" = \"warning\"\r\n",
    );
    let patch = build_rule_patch(&document, &set_rule("BSK-0002", RuleSeverity::Error)).unwrap();
    assert!(patch.content.contains("\r\n"));
    assert!(!patch.content.replace("\r\n", "").contains('\n'));
    let _ = std::fs::remove_dir_all(&root);
}

/// [CONFIGEDITOR-SOURCES]: apply verifies the base revision so a stale patch
/// never overwrites an external edit.
#[test]
fn apply_rejects_stale_revision() {
    let root = temp_root("stale");
    let config_file = root.join("pyproject.toml");
    std::fs::write(
        &config_file,
        "[tool.basilisk.rules]\n\"BSK-0001\" = \"error\"\n",
    )
    .unwrap();
    let document = discover_config_document(&root).unwrap();
    let patch = build_rule_patch(&document, &set_rule("BSK-0002", RuleSeverity::Error)).unwrap();
    // External edit lands between preview and apply.
    std::fs::write(
        &config_file,
        "[tool.basilisk.rules]\n\"BSK-0001\" = \"info\"\n",
    )
    .unwrap();
    let result = apply_config_patch(&patch);
    assert!(matches!(
        result,
        Err(ConfigDocumentError::RevisionConflict { .. })
    ));
    let _ = std::fs::remove_dir_all(&root);
}

/// [CONFIGEDITOR-SOURCES]: a valid patch persists atomically and reloads.
#[test]
fn apply_persists_patch() {
    let root = temp_root("persist");
    let document = discover_config_document(&root).unwrap();
    let patch = build_rule_patch(&document, &set_tag("basilisk", RuleSeverity::Error)).unwrap();
    apply_config_patch(&patch).unwrap();
    let reloaded = discover_config_document(&root).unwrap();
    assert!(reloaded.exists);
    assert_eq!(reloaded.revision, patch.revision);
    assert_eq!(
        reloaded.config.resolve_severity("BSK-0001", &["basilisk"]),
        Some(RuleSeverity::Error)
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// A wrong-shaped mutation target is invalid, not silently replaced.
#[test]
fn wrong_shaped_rules_target_is_invalid() {
    let root = temp_root("wrong_shape");
    let result = discover_config_document_with_content(
        &root,
        "[tool.basilisk]\nrules = \"nope\"\n".to_owned(),
    );
    assert!(matches!(result, Err(ConfigDocumentError::Invalid { .. })));
    let _ = std::fs::remove_dir_all(&root);
}

/// A document the editor holds in a state discovery would have rejected
/// (stale buffer, external corruption) — the patch layer re-validates rather
/// than trusting the caller.
fn manual_document(root: &Path, content: &str, read_only: bool) -> ConfigDocument {
    ConfigDocument {
        root: root.to_path_buf(),
        path: root.join("pyproject.toml"),
        exists: true,
        read_only,
        content: content.to_owned(),
        revision: content_revision(content),
        config: BasiliskConfig::default(),
    }
}

fn manual_patch(path: std::path::PathBuf, base: &str, content: &str) -> ConfigPatch {
    ConfigPatch {
        path,
        base_revision: content_revision(base),
        revision: content_revision(content),
        content: content.to_owned(),
        config: BasiliskConfig::default(),
    }
}

/// [CONFIGEDITOR-OPERATIONS]: every error variant renders an actionable
/// message naming the source or the conflicting revisions.
#[test]
fn errors_display_their_cause() {
    let path = std::path::PathBuf::from("/ws/pyproject.toml");
    let read = ConfigDocumentError::Read {
        path: path.clone(),
        message: "denied".to_owned(),
    };
    assert!(read.to_string().contains("failed to read"), "{read}");
    assert!(read.to_string().contains("denied"), "{read}");
    let invalid = ConfigDocumentError::Invalid {
        path: path.clone(),
        message: "bad shape".to_owned(),
    };
    assert!(
        invalid.to_string().contains("invalid configuration"),
        "{invalid}"
    );
    let conflict = ConfigDocumentError::RevisionConflict {
        expected: "fnv1a64:aa".to_owned(),
        actual: "fnv1a64:bb".to_owned(),
    };
    assert!(
        conflict.to_string().contains("revision changed"),
        "{conflict}"
    );
    assert!(conflict.to_string().contains("fnv1a64:aa"), "{conflict}");
    let read_only = ConfigDocumentError::ReadOnly { path };
    assert!(read_only.to_string().contains("read-only"), "{read_only}");
}

/// [CONFIGEDITOR-SOURCES]: an unreadable active source is a `Read` error,
/// never a silent fallback to defaults.
#[test]
fn unreadable_pyproject_is_a_read_error() {
    let root = temp_root("unreadable");
    std::fs::write(root.join("pyproject.toml"), [0xFF, 0xFE, 0x00, 0x9F]).unwrap();
    let result = discover_config_document(&root);
    assert!(matches!(result, Err(ConfigDocumentError::Read { .. })));
    let _ = std::fs::remove_dir_all(&root);
}

/// [CHKARCH-CONFIG-FILE]: `tool` and `tool.basilisk` must be tables; a file
/// configuring only other tools contributes no table.
#[test]
fn wrong_shaped_tool_roots_are_invalid() {
    let root = temp_root("tool_shape");
    let result = discover_config_document_with_content(&root, "tool = 3\n".to_owned());
    assert!(matches!(result, Err(ConfigDocumentError::Invalid { .. })));
    let result = discover_config_document_with_content(&root, "[tool]\nbasilisk = 3\n".to_owned());
    assert!(matches!(result, Err(ConfigDocumentError::Invalid { .. })));
    let document = document_for(&root, "[tool.poetry]\nname = \"demo\"\n");
    assert!(!document.config.has_config_table());
    let _ = std::fs::remove_dir_all(&root);
}

/// [CONFIGEDITOR-OPERATIONS]: a read-only source refuses mutations.
#[test]
fn read_only_document_refuses_patches() {
    let root = temp_root("read_only");
    let document = manual_document(&root, "", true);
    let result = build_rule_patch(&document, &set_rule("BSK-0001", RuleSeverity::Error));
    assert!(matches!(result, Err(ConfigDocumentError::ReadOnly { .. })));
    let _ = std::fs::remove_dir_all(&root);
}

/// [CONFIGEDITOR-OPERATIONS]: held content that no longer parses fails to
/// patch instead of guessing at a rewrite.
#[test]
fn unparseable_document_content_fails_to_patch() {
    let root = temp_root("unparseable");
    let document = manual_document(&root, "not = [valid", false);
    let result = build_rule_patch(&document, &set_rule("BSK-0001", RuleSeverity::Error));
    assert!(matches!(result, Err(ConfigDocumentError::Invalid { .. })));
    let _ = std::fs::remove_dir_all(&root);
}

/// [CHKARCH-CONFIG-MODEL]: wrong-shaped mutation targets are invalid for
/// `rules` and `rule-tags` alike — never silently replaced.
#[test]
fn wrong_shaped_mutation_targets_fail_to_patch() {
    let root = temp_root("wrong_target");
    let rules_document = manual_document(&root, "[tool.basilisk]\nrules = \"nope\"\n", false);
    let result = build_rule_patch(&rules_document, &set_rule("BSK-0001", RuleSeverity::Error));
    assert!(matches!(result, Err(ConfigDocumentError::Invalid { .. })));
    let tags_document = manual_document(&root, "[tool.basilisk]\nrule-tags = 3\n", false);
    let result = build_rule_patch(&tags_document, &set_tag("basilisk", RuleSeverity::Error));
    assert!(matches!(result, Err(ConfigDocumentError::Invalid { .. })));
    let _ = std::fs::remove_dir_all(&root);
}

/// [CONFIGEDITOR-SOURCES]: apply surfaces an unreadable target as a `Read`
/// error before touching anything.
#[test]
fn apply_surfaces_unreadable_target() {
    let root = temp_root("apply_unreadable");
    let config_file = root.join("pyproject.toml");
    std::fs::write(&config_file, [0xFF, 0xFE, 0x00, 0x9F]).unwrap();
    let patch = manual_patch(config_file, "", "[tool.basilisk.rules]\n");
    assert!(matches!(
        apply_config_patch(&patch),
        Err(ConfigDocumentError::Read { .. })
    ));
    let _ = std::fs::remove_dir_all(&root);
}

/// [CONFIGEDITOR-SOURCES]: a target with no parent directory cannot be
/// written atomically and is rejected.
#[test]
fn apply_rejects_rootless_target() {
    let patch = manual_patch(std::path::PathBuf::from("/"), "", "x = 1\n");
    assert!(matches!(
        apply_config_patch(&patch),
        Err(ConfigDocumentError::Read { .. })
    ));
}

/// [CONFIGEDITOR-SOURCES]: a regular file shadowing the parent directory
/// fails `apply` cleanly.
#[test]
fn apply_rejects_file_shadowing_parent_directory() {
    let root = temp_root("apply_parent_file");
    let blocker = root.join("blocker");
    std::fs::write(&blocker, "file").unwrap();
    let patch = manual_patch(blocker.join("pyproject.toml"), "", "x = 1\n");
    assert!(matches!(
        apply_config_patch(&patch),
        Err(ConfigDocumentError::Read { .. })
    ));
    let _ = std::fs::remove_dir_all(&root);
}

/// [CONFIGEDITOR-SOURCES]: a directory squatting on the target path fails the
/// atomic rename AND leaves no temp file behind.
#[test]
fn apply_rejects_directory_target_and_cleans_temp() {
    let root = temp_root("apply_dir_target");
    let target = root.join("pyproject.toml");
    std::fs::create_dir_all(&target).unwrap();
    let patch = manual_patch(target, "", "x = 1\n");
    assert!(matches!(
        apply_config_patch(&patch),
        Err(ConfigDocumentError::Read { .. })
    ));
    let leftovers: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "temp files must be cleaned up on failure"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// [CONFIGEDITOR-SOURCES]: an unwritable parent directory surfaces as a
/// `Read` error instead of a partial write.
#[cfg(unix)]
#[test]
fn apply_surfaces_unwritable_parent() {
    use std::os::unix::fs::PermissionsExt as _;
    let root = temp_root("apply_ro_dir");
    let locked = root.join("locked");
    std::fs::create_dir_all(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
    let patch = manual_patch(locked.join("pyproject.toml"), "", "x = 1\n");
    let result = apply_config_patch(&patch);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(result, Err(ConfigDocumentError::Read { .. })));
    let _ = std::fs::remove_dir_all(&root);
}
