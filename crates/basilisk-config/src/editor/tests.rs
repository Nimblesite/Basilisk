use std::collections::BTreeMap;
use std::path::PathBuf;

use super::{
    active_config_path, adoption_rule_overrides, apply_config_patch, build_rule_patch,
    content_revision, discover_config_document, discover_config_document_with_content,
    validate_content, ConfigDocument, ConfigDocumentError, ConfigFormat, RuleConfigScope,
    RuleConfigUpdate,
};
use crate::RuleSeverity;

// ---------------------------------------------------------------------------
// Fixtures.
// ---------------------------------------------------------------------------

fn document_with_config(content: &str, config: crate::BasiliskConfig) -> ConfigDocument {
    ConfigDocument {
        root: PathBuf::from("/workspace"),
        path: PathBuf::from("/workspace/pyproject.toml"),
        format: ConfigFormat::PyprojectToml,
        exists: true,
        read_only: false,
        shadowed_sources: Vec::new(),
        content: content.to_owned(),
        revision: content_revision(content),
        config,
    }
}

/// A validated in-memory `pyproject.toml` document.
fn document(content: &str) -> ConfigDocument {
    let config = validate_content(
        &PathBuf::from("/workspace/pyproject.toml"),
        ConfigFormat::PyprojectToml,
        content,
    )
    .unwrap();
    document_with_config(content, config)
}

/// An unvalidated document for exercising patch-time shape errors on content
/// that document validation would reject up front.
fn raw_document(content: &str) -> ConfigDocument {
    document_with_config(content, crate::BasiliskConfig::default())
}

fn update(scope: RuleConfigScope, code: &str, severity: Option<RuleSeverity>) -> RuleConfigUpdate {
    RuleConfigUpdate {
        scope,
        rules: BTreeMap::from([(code.to_owned(), severity)]),
    }
}

fn temp_root(tag: &str) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "basilisk-config-{tag}-{}-{seq}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

/// A stray legacy `basilisk.json` must never influence discovery, whatever
/// its contents: the `pyproject.toml` config wins verbatim and the JSON file
/// is only reported as shadowed — never read, never an error.
fn assert_stray_json_is_ignored(tag: &str, json_content: &str) {
    let root = temp_root(tag);
    std::fs::write(
        root.join("pyproject.toml"),
        "[tool.basilisk.rules]\nBSK-E0001 = \"warning\"\n",
    )
    .unwrap();
    std::fs::write(root.join("basilisk.json"), json_content).unwrap();
    let document = discover_config_document(&root).unwrap();
    assert_eq!(document.format, ConfigFormat::PyprojectToml);
    assert_eq!(document.path, root.join("pyproject.toml"));
    assert_eq!(document.shadowed_sources, vec![root.join("basilisk.json")]);
    assert_eq!(
        document.config.rules.get("BSK-E0001"),
        Some(&RuleSeverity::Warning)
    );
    assert_eq!(document.config.rules.len(), 1);
    assert!(document.config.per_path_overrides.is_empty());
    assert!(document.config.stub_paths.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

// ---------------------------------------------------------------------------
// Discovery with editor-held content.
// ---------------------------------------------------------------------------

#[test]
fn editor_content_can_repair_a_malformed_active_disk_source() {
    let root = std::env::temp_dir().join(format!(
        "basilisk-config-open-repair-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("pyproject.toml"), "[tool.basilisk.rules\n").unwrap();
    assert!(discover_config_document(&root).is_err());

    let repaired = discover_config_document_with_content(
        &root,
        "[tool.basilisk.rules]\nBSK-E0001 = \"warning\"\n".to_owned(),
    )
    .unwrap();

    assert_eq!(
        repaired.config.rules.get("BSK-E0001"),
        Some(&RuleSeverity::Warning)
    );
    assert_eq!(repaired.path, root.join("pyproject.toml"));
    let _ = std::fs::remove_dir_all(root);
}

// ---------------------------------------------------------------------------
// Stray legacy basilisk.json — never read, never an error, always shadowed.
// Each fixture is content the removed JSON parser used to accept or reject;
// today none of it may have any effect on the loaded config.
// ---------------------------------------------------------------------------

#[test]
fn stray_json_kebab_per_path_overrides_have_no_effect() {
    assert_stray_json_is_ignored(
        "stray-kebab",
        r#"{
  "per-path-overrides": {
    "src/app.py": {
      "adoption": true,
      "rules": { "BSK-E0001": "warning" }
    }
  },
  "custom": { "keep": true }
}
"#,
    );
}

#[test]
fn stray_json_with_both_per_path_spellings_has_no_effect() {
    assert_stray_json_is_ignored(
        "stray-both-spellings",
        r#"{"perPathOverrides": {}, "per-path-overrides": {}}"#,
    );
}

#[test]
fn stray_json_with_invalid_syntax_has_no_effect() {
    assert_stray_json_is_ignored("stray-invalid-syntax", "{not json");
}

#[test]
fn stray_json_with_non_object_root_has_no_effect() {
    assert_stray_json_is_ignored("stray-non-object-root", "[1, 2, 3]");
}

#[test]
fn stray_json_with_non_object_rules_has_no_effect() {
    assert_stray_json_is_ignored("stray-rules-shape", r#"{"rules": "nope"}"#);
}

#[test]
fn stray_json_with_invalid_severity_has_no_effect() {
    assert_stray_json_is_ignored("stray-severity", r#"{"rules": {"BSK-E0001": "louder"}}"#);
}

#[test]
fn stray_json_with_non_object_per_path_overrides_has_no_effect() {
    assert_stray_json_is_ignored("stray-ppo-shape", r#"{"perPathOverrides": 5}"#);
}

#[test]
fn stray_json_with_non_object_path_entry_has_no_effect() {
    assert_stray_json_is_ignored(
        "stray-path-entry",
        r#"{"perPathOverrides": {"src/app.py": 5}}"#,
    );
}

#[test]
fn stray_json_with_non_array_disabled_has_no_effect() {
    assert_stray_json_is_ignored(
        "stray-disabled-shape",
        r#"{"perPathOverrides": {"src/app.py": {"disabled": "x"}}}"#,
    );
}

#[test]
fn stray_json_with_non_string_disabled_entries_has_no_effect() {
    assert_stray_json_is_ignored(
        "stray-disabled-entries",
        r#"{"perPathOverrides": {"src/app.py": {"disabled": [1]}}}"#,
    );
}

#[test]
fn stray_json_with_non_boolean_adoption_has_no_effect() {
    assert_stray_json_is_ignored(
        "stray-adoption-shape",
        r#"{"perPathOverrides": {"src/app.py": {"adoption": 1}}}"#,
    );
}

#[test]
fn stray_json_with_camel_case_per_path_overrides_has_no_effect() {
    assert_stray_json_is_ignored(
        "stray-camel",
        r#"{"perPathOverrides": {"src/app.py": {"adoption": true, "rules": {"BSK-E0001": "info"}}}}"#,
    );
}

// ---------------------------------------------------------------------------
// Rule patching — structure preservation and adoption projection.
// ---------------------------------------------------------------------------

#[test]
fn toml_reset_removes_empty_adoption_path_and_rules() {
    let source = r#"[tool.basilisk.per-path-overrides."src/app.py"]
adoption = true

[tool.basilisk.per-path-overrides."src/app.py".rules]
BSK-E0001 = "warning"
"#;
    let patch = build_rule_patch(
        &document(source),
        &[update(
            RuleConfigScope::Path {
                pattern: "src/app.py".to_owned(),
                adoption: false,
            },
            "BSK-E0001",
            None,
        )],
    )
    .unwrap();
    assert!(patch.content.trim().is_empty());
    assert!(patch.config.per_path_overrides.is_empty());
}

#[test]
fn malformed_existing_pyproject_tables_are_not_defaults() {
    for source in [
        "tool = 1\n",
        "[tool]\nbasilisk = 1\n",
        "[tool.basilisk]\nrules = []\n",
        "[tool.basilisk]\nper-path-overrides = []\n",
    ] {
        assert!(
            validate_content(
                &PathBuf::from("pyproject.toml"),
                ConfigFormat::PyprojectToml,
                source,
            )
            .is_err(),
            "source should be invalid: {source}"
        );
    }
}

#[test]
fn toml_patch_preserves_comments_and_projects_replacement_config() {
    let source = "# project comment\n[project]\nname = \"demo\"\n\n[tool.basilisk.rules]\n# rule comment\nBSK-E0001 = \"warning\"\n";
    let patch = build_rule_patch(
        &document(source),
        &[update(
            RuleConfigScope::Project,
            "BSK-E0001",
            Some(RuleSeverity::Error),
        )],
    )
    .unwrap();
    assert!(patch.content.contains("# project comment"));
    assert!(patch.content.contains("# rule comment"));
    assert_eq!(
        patch.config.rules.get("BSK-E0001"),
        Some(&RuleSeverity::Error)
    );
    assert_ne!(patch.revision, patch.base_revision);
}

#[test]
fn adoption_projection_reads_only_marked_toml_entries() {
    let toml = document(
        r#"[tool.basilisk.per-path-overrides."src/app.py"]
adoption = true

[tool.basilisk.per-path-overrides."src/app.py".rules]
BSK-E0001 = "warning"
BSK-W0050 = "info"

[tool.basilisk.per-path-overrides."vendor/**".rules]
BSK-E0001 = "disabled"
"#,
    );
    let expected = BTreeMap::from([(
        "src/app.py".to_owned(),
        BTreeMap::from([
            ("BSK-E0001".to_owned(), RuleSeverity::Warning),
            ("BSK-W0050".to_owned(), RuleSeverity::Info),
        ]),
    )]);
    assert_eq!(adoption_rule_overrides(&toml), expected);
}

#[test]
fn path_severity_canonicalizes_legacy_disabled_entries() {
    let source = r#"[tool.basilisk.per-path-overrides."src/app.py"]
disabled = ["BSK-E0001", "BSK-E0002"]
"#;
    for severity in [RuleSeverity::Warning, RuleSeverity::Disabled] {
        let patch = build_rule_patch(
            &document(source),
            &[update(
                RuleConfigScope::Path {
                    pattern: "src/app.py".to_owned(),
                    adoption: false,
                },
                "BSK-E0001",
                Some(severity),
            )],
        )
        .unwrap();
        let entry = patch.config.per_path_overrides.get("src/app.py").unwrap();
        assert!(!entry.disabled_rules.contains(&"BSK-E0001".to_owned()));
        assert!(entry.disabled_rules.contains(&"BSK-E0002".to_owned()));
        assert_eq!(entry.rule_overrides.get("BSK-E0001"), Some(&severity));
    }
}

#[test]
fn atomic_apply_rejects_a_stale_revision_without_overwriting_external_edits() {
    let root = std::env::temp_dir().join(format!("basilisk-config-stale-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let source_path = root.join("pyproject.toml");
    std::fs::write(&source_path, "[project]\nname = \"before\"\n").unwrap();
    let document = discover_config_document(&root).unwrap();
    let patch = build_rule_patch(
        &document,
        &[update(
            RuleConfigScope::Project,
            "BSK-E0001",
            Some(RuleSeverity::Warning),
        )],
    )
    .unwrap();
    let external = "[project]\nname = \"external\"\n";
    std::fs::write(&source_path, external).unwrap();

    let error = apply_config_patch(&patch).unwrap_err();
    assert!(matches!(
        error,
        ConfigDocumentError::RevisionConflict { .. }
    ));
    assert_eq!(std::fs::read_to_string(&source_path).unwrap(), external);
    let _ = std::fs::remove_dir_all(root);
}

// ---------------------------------------------------------------------------
// ConfigDocumentError Display — all four variants.
// ---------------------------------------------------------------------------

#[test]
fn display_read_error_names_path_and_message() {
    let error = ConfigDocumentError::Read {
        path: PathBuf::from("/workspace/pyproject.toml"),
        message: "permission denied".to_owned(),
    };
    assert_eq!(
        error.to_string(),
        "failed to read /workspace/pyproject.toml: permission denied"
    );
}

#[test]
fn display_invalid_error_names_path_and_message() {
    let error = ConfigDocumentError::Invalid {
        path: PathBuf::from("/workspace/pyproject.toml"),
        message: "rules must be a table".to_owned(),
    };
    assert_eq!(
        error.to_string(),
        "invalid configuration /workspace/pyproject.toml: rules must be a table"
    );
}

#[test]
fn display_revision_conflict_shows_both_revisions() {
    let error = ConfigDocumentError::RevisionConflict {
        expected: "fnv1a64:aaaa".to_owned(),
        actual: "fnv1a64:bbbb".to_owned(),
    };
    assert_eq!(
        error.to_string(),
        "configuration revision changed (fnv1a64:aaaa != fnv1a64:bbbb)"
    );
}

#[test]
fn display_read_only_error_names_path() {
    let error = ConfigDocumentError::ReadOnly {
        path: PathBuf::from("/workspace/pyproject.toml"),
    };
    assert_eq!(
        error.to_string(),
        "configuration is read-only: /workspace/pyproject.toml"
    );
}

// ---------------------------------------------------------------------------
// validate_content — TOML structure errors.
// ---------------------------------------------------------------------------

fn validate_toml(source: &str) -> Result<(), ConfigDocumentError> {
    validate_content(
        &PathBuf::from("pyproject.toml"),
        ConfigFormat::PyprojectToml,
        source,
    )
    .map(|_| ())
}

#[test]
fn empty_content_is_a_default_config() {
    let config = validate_content(
        &PathBuf::from("pyproject.toml"),
        ConfigFormat::PyprojectToml,
        "",
    )
    .unwrap();
    assert!(config.rules.is_empty());
}

#[test]
fn toml_invalid_syntax_is_rejected() {
    let error = validate_toml("[tool.basilisk.rules\n").unwrap_err();
    assert!(matches!(error, ConfigDocumentError::Invalid { .. }));
}

#[test]
fn toml_without_tool_basilisk_is_default() {
    let config = validate_content(
        &PathBuf::from("pyproject.toml"),
        ConfigFormat::PyprojectToml,
        "[project]\nname = \"demo\"\n",
    )
    .unwrap();
    assert!(config.rules.is_empty());
}

#[test]
fn toml_tool_not_a_table_is_rejected() {
    let error = validate_toml("tool = 1\n").unwrap_err();
    assert!(error.to_string().contains("`tool` must be a table"));
}

#[test]
fn toml_tool_without_basilisk_is_default() {
    let config = validate_content(
        &PathBuf::from("pyproject.toml"),
        ConfigFormat::PyprojectToml,
        "[tool.other]\nkey = 1\n",
    )
    .unwrap();
    assert!(config.rules.is_empty());
}

#[test]
fn toml_basilisk_not_a_table_is_rejected() {
    let error = validate_toml("[tool]\nbasilisk = 1\n").unwrap_err();
    assert!(error
        .to_string()
        .contains("`tool.basilisk` must be a table"));
}

#[test]
fn toml_rules_not_a_table_is_rejected() {
    let error = validate_toml("[tool.basilisk]\nrules = []\n").unwrap_err();
    assert!(error.to_string().contains("rules must be a table"));
}

#[test]
fn toml_invalid_severity_names_the_rule() {
    let error = validate_toml("[tool.basilisk.rules]\nBSK-E0001 = \"loud\"\n").unwrap_err();
    assert!(error
        .to_string()
        .contains("rule `BSK-E0001` has an invalid severity"));
}

#[test]
fn toml_per_path_overrides_not_a_table_is_rejected() {
    let error = validate_toml("[tool.basilisk]\nper-path-overrides = []\n").unwrap_err();
    assert!(error
        .to_string()
        .contains("`tool.basilisk.per-path-overrides` must be a table"));
}

#[test]
fn toml_path_override_not_a_table_is_rejected() {
    let error =
        validate_toml("[tool.basilisk.per-path-overrides]\n\"src/app.py\" = 1\n").unwrap_err();
    assert!(error
        .to_string()
        .contains("path override `src/app.py` must be a table"));
}

#[test]
fn toml_disabled_not_an_array_is_rejected() {
    let error =
        validate_toml("[tool.basilisk.per-path-overrides.\"src/app.py\"]\ndisabled = \"x\"\n")
            .unwrap_err();
    assert!(error
        .to_string()
        .contains("path override `src/app.py` disabled must be an array"));
}

#[test]
fn toml_disabled_entries_must_be_strings() {
    let error =
        validate_toml("[tool.basilisk.per-path-overrides.\"src/app.py\"]\ndisabled = [1]\n")
            .unwrap_err();
    assert!(error
        .to_string()
        .contains("path override `src/app.py` disabled entries must be strings"));
}

#[test]
fn toml_adoption_must_be_a_boolean() {
    let error = validate_toml("[tool.basilisk.per-path-overrides.\"src/app.py\"]\nadoption = 1\n")
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("path override `src/app.py` adoption must be a boolean"));
}

// ---------------------------------------------------------------------------
// content_revision determinism.
// ---------------------------------------------------------------------------

#[test]
fn content_revision_is_deterministic_and_distinct() {
    let a = content_revision("hello world");
    assert_eq!(a, content_revision("hello world"));
    assert_ne!(a, content_revision("hello worle"));
    assert!(a.starts_with("fnv1a64:"));
}

// ---------------------------------------------------------------------------
// discover_config_document — selection, shadowing, and read-only.
// ---------------------------------------------------------------------------

#[test]
fn discover_treats_directory_named_pyproject_as_missing() {
    let root = temp_root("discover-dir");
    // A directory named pyproject.toml is not a file: discovery treats the
    // active source as absent and returns a default config rather than a
    // read error.
    std::fs::create_dir_all(root.join("pyproject.toml")).unwrap();
    let document = discover_config_document(&root).unwrap();
    assert!(!document.exists);
    assert!(document.config.rules.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn discover_defaults_when_no_source_exists() {
    let root = temp_root("discover-empty");
    let document = discover_config_document(&root).unwrap();
    assert_eq!(document.format, ConfigFormat::PyprojectToml);
    assert_eq!(document.path, root.join("pyproject.toml"));
    assert!(!document.exists);
    assert!(!document.read_only);
    assert!(document.shadowed_sources.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn active_config_path_is_pyproject_even_when_json_present() {
    let root = temp_root("active-precedence");
    std::fs::write(root.join("pyproject.toml"), "[project]\nname = \"x\"\n").unwrap();
    std::fs::write(root.join("basilisk.json"), "{}\n").unwrap();
    assert_eq!(active_config_path(&root), root.join("pyproject.toml"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn discover_selects_toml_and_shadows_stray_json() {
    // The two sources conflict on BSK-E0001; the TOML severity must win and
    // the stray JSON must only be reported as shadowed.
    assert_stray_json_is_ignored("discover-shadow", r#"{"rules": {"BSK-E0001": "disabled"}}"#);
}

#[test]
fn discover_detects_read_only_source() {
    let root = temp_root("discover-readonly");
    let path = root.join("pyproject.toml");
    std::fs::write(&path, "[tool.basilisk.rules]\nBSK-E0001 = \"warning\"\n").unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&path, perms).unwrap();
    let document = discover_config_document(&root).unwrap();
    assert!(document.read_only);
    // A read-only file inside a writable directory is still removable, so
    // recursive cleanup succeeds without clearing the read-only bit.
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn discover_ignores_malformed_stray_json_without_pyproject() {
    let root = temp_root("discover-stray-invalid");
    std::fs::write(root.join("basilisk.json"), "{not json").unwrap();
    let document = discover_config_document(&root).unwrap();
    assert_eq!(document.format, ConfigFormat::PyprojectToml);
    assert_eq!(document.path, root.join("pyproject.toml"));
    assert!(!document.exists);
    assert!(document.config.rules.is_empty());
    assert_eq!(document.shadowed_sources, vec![root.join("basilisk.json")]);
    let _ = std::fs::remove_dir_all(root);
}

// ---------------------------------------------------------------------------
// build_rule_patch — read-only and wrong-shaped targets.
// ---------------------------------------------------------------------------

#[test]
fn build_rule_patch_rejects_read_only_document() {
    let mut document = document("");
    document.read_only = true;
    let error = build_rule_patch(
        &document,
        &[update(
            RuleConfigScope::Project,
            "BSK-E0001",
            Some(RuleSeverity::Warning),
        )],
    )
    .unwrap_err();
    assert!(matches!(error, ConfigDocumentError::ReadOnly { .. }));
}

#[test]
fn build_rule_patch_toml_rejects_non_table_rules_target() {
    // rules-as-string TOML fails document validation, so build the document
    // without validation; patch_toml must reject the target before
    // re-validating.
    let error = build_rule_patch(
        &raw_document("[tool.basilisk]\nrules = \"x\"\n"),
        &[update(
            RuleConfigScope::Project,
            "BSK-E0001",
            Some(RuleSeverity::Warning),
        )],
    )
    .unwrap_err();
    assert!(error.to_string().contains("`rules` must be a table"));
}

#[test]
fn build_rule_patch_toml_rejects_non_table_tool() {
    let error = build_rule_patch(
        &raw_document("tool = 1\n"),
        &[update(
            RuleConfigScope::Project,
            "BSK-E0001",
            Some(RuleSeverity::Warning),
        )],
    )
    .unwrap_err();
    assert!(error.to_string().contains("`tool` must be a table"));
}

#[test]
fn build_rule_patch_toml_rejects_non_table_path_entry() {
    let error = build_rule_patch(
        &raw_document("[tool.basilisk.per-path-overrides]\n\"src/app.py\" = 1\n"),
        &[update(
            RuleConfigScope::Path {
                pattern: "src/app.py".to_owned(),
                adoption: false,
            },
            "BSK-E0001",
            Some(RuleSeverity::Warning),
        )],
    )
    .unwrap_err();
    assert!(error.to_string().contains("`src/app.py` must be a table"));
}

// ---------------------------------------------------------------------------
// patch rendering — parse and shape errors on manually-built documents.
// ---------------------------------------------------------------------------

#[test]
fn patch_toml_reports_unparseable_source() {
    let error = build_rule_patch(
        &raw_document("[tool.basilisk.rules\n"),
        &[update(
            RuleConfigScope::Project,
            "BSK-E0001",
            Some(RuleSeverity::Warning),
        )],
    )
    .unwrap_err();
    assert!(matches!(error, ConfigDocumentError::Invalid { .. }));
}

#[test]
fn patch_toml_rejects_non_table_basilisk() {
    let error = build_rule_patch(
        &raw_document("[tool]\nbasilisk = 1\n"),
        &[update(
            RuleConfigScope::Project,
            "BSK-E0001",
            Some(RuleSeverity::Warning),
        )],
    )
    .unwrap_err();
    assert!(error.to_string().contains("`basilisk` must be a table"));
}

#[test]
fn patch_toml_rejects_non_table_per_path_intermediate() {
    let error = build_rule_patch(
        &raw_document("[tool.basilisk]\nper-path-overrides = \"x\"\n"),
        &[update(
            RuleConfigScope::Path {
                pattern: "src/app.py".to_owned(),
                adoption: false,
            },
            "BSK-E0001",
            Some(RuleSeverity::Warning),
        )],
    )
    .unwrap_err();
    assert!(error
        .to_string()
        .contains("`per-path-overrides` must be a table"));
}

#[test]
fn patch_toml_rejects_non_table_path_rules_target() {
    let error = build_rule_patch(
        &raw_document("[tool.basilisk.per-path-overrides.\"src/app.py\"]\nrules = \"x\"\n"),
        &[update(
            RuleConfigScope::Path {
                pattern: "src/app.py".to_owned(),
                adoption: false,
            },
            "BSK-E0001",
            Some(RuleSeverity::Warning),
        )],
    )
    .unwrap_err();
    assert!(error.to_string().contains("`rules` must be a table"));
}

// ---------------------------------------------------------------------------
// patch_toml — mutation edge cases.
// ---------------------------------------------------------------------------

#[test]
fn toml_removing_last_project_rule_drops_the_rules_table() {
    let source = "[tool.basilisk.rules]\nBSK-E0001 = \"warning\"\n";
    let patch = build_rule_patch(
        &document(source),
        &[update(RuleConfigScope::Project, "BSK-E0001", None)],
    )
    .unwrap();
    assert!(!patch.content.contains("rules"));
    assert!(patch.config.rules.is_empty());
}

#[test]
fn toml_removing_last_rule_gc_keeps_unrelated_content() {
    let source = "[project]\nname = \"demo\"\n\n[tool.basilisk.rules]\nBSK-E0001 = \"warning\"\n";
    let patch = build_rule_patch(
        &document(source),
        &[update(RuleConfigScope::Project, "BSK-E0001", None)],
    )
    .unwrap();
    let table: toml::Table = patch.content.parse().unwrap();
    assert!(table.get("tool").is_none());
    assert_eq!(table["project"]["name"].as_str(), Some("demo"));
    assert!(patch.config.rules.is_empty());
}

#[test]
fn toml_patch_preserves_crlf_newlines() {
    let source = "[tool.basilisk.rules]\r\nBSK-E0001 = \"warning\"\r\n";
    let patch = build_rule_patch(
        &document(source),
        &[update(
            RuleConfigScope::Project,
            "BSK-E0002",
            Some(RuleSeverity::Error),
        )],
    )
    .unwrap();
    assert!(patch.content.contains("\r\n"));
    assert!(!patch.content.contains("\n\n\r"));
    // Every newline is a CRLF: no bare LF remains.
    assert!(!patch.content.replace("\r\n", "").contains('\n'));
}

#[test]
fn toml_adoption_true_writes_the_adoption_key() {
    let source = "";
    let patch = build_rule_patch(
        &document(source),
        &[update(
            RuleConfigScope::Path {
                pattern: "src/app.py".to_owned(),
                adoption: true,
            },
            "BSK-E0001",
            Some(RuleSeverity::Disabled),
        )],
    )
    .unwrap();
    assert!(patch.content.contains("adoption = true"));
    assert!(patch.content.contains("BSK-E0001"));
}

#[test]
fn toml_adoption_patch_round_trips_through_projection() {
    let patch = build_rule_patch(
        &document(""),
        &[update(
            RuleConfigScope::Path {
                pattern: "src/app.py".to_owned(),
                adoption: true,
            },
            "BSK-E0001",
            Some(RuleSeverity::Disabled),
        )],
    )
    .unwrap();
    let expected = BTreeMap::from([(
        "src/app.py".to_owned(),
        BTreeMap::from([("BSK-E0001".to_owned(), RuleSeverity::Disabled)]),
    )]);
    assert_eq!(adoption_rule_overrides(&document(&patch.content)), expected);
}

#[test]
fn toml_canonicalize_drops_empty_disabled_array() {
    let source = "[tool.basilisk.per-path-overrides.\"src/app.py\"]\ndisabled = [\"BSK-E0001\"]\n";
    let patch = build_rule_patch(
        &document(source),
        &[update(
            RuleConfigScope::Path {
                pattern: "src/app.py".to_owned(),
                adoption: false,
            },
            "BSK-E0001",
            Some(RuleSeverity::Warning),
        )],
    )
    .unwrap();
    assert!(!patch.content.contains("disabled"));
    let entry = patch.config.per_path_overrides.get("src/app.py").unwrap();
    assert!(entry.disabled_rules.is_empty());
}

#[test]
fn toml_path_entry_becomes_empty_is_removed() {
    let source = "[tool.basilisk.per-path-overrides.\"src/app.py\"]\ndisabled = [\"BSK-E0001\"]\n";
    let patch = build_rule_patch(
        &document(source),
        &[update(
            RuleConfigScope::Path {
                pattern: "src/app.py".to_owned(),
                adoption: false,
            },
            "BSK-E0001",
            None,
        )],
    )
    .unwrap();
    assert!(!patch.content.contains("src/app.py"));
    assert!(patch.config.per_path_overrides.is_empty());
}

#[test]
fn toml_reset_keeps_sibling_path_entries() {
    let source = r#"[tool.basilisk.per-path-overrides."src/app.py"]
disabled = ["BSK-E0001"]

[tool.basilisk.per-path-overrides."src/lib.py".rules]
BSK-E0002 = "info"
"#;
    let patch = build_rule_patch(
        &document(source),
        &[update(
            RuleConfigScope::Path {
                pattern: "src/app.py".to_owned(),
                adoption: false,
            },
            "BSK-E0001",
            None,
        )],
    )
    .unwrap();
    assert!(!patch.content.contains("src/app.py"));
    assert_eq!(patch.config.per_path_overrides.len(), 1);
    let sibling = patch.config.per_path_overrides.get("src/lib.py").unwrap();
    assert_eq!(
        sibling.rule_overrides.get("BSK-E0002"),
        Some(&RuleSeverity::Info)
    );
}

// ---------------------------------------------------------------------------
// apply_config_patch — happy path and nested-dir creation.
// ---------------------------------------------------------------------------

#[test]
fn apply_config_patch_writes_exact_content_atomically() {
    let root = temp_root("apply-happy");
    std::fs::write(root.join("pyproject.toml"), "[project]\nname = \"demo\"\n").unwrap();
    let document = discover_config_document(&root).unwrap();
    let patch = build_rule_patch(
        &document,
        &[update(
            RuleConfigScope::Project,
            "BSK-E0001",
            Some(RuleSeverity::Warning),
        )],
    )
    .unwrap();
    apply_config_patch(&patch).unwrap();
    let written = std::fs::read_to_string(root.join("pyproject.toml")).unwrap();
    assert_eq!(written, patch.content);
    assert!(written.contains("BSK-E0001"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn apply_config_patch_creates_missing_nested_directory() {
    let root = temp_root("apply-nested");
    let nested = root.join("a").join("b");
    let document = discover_config_document_with_content(&nested, String::new()).unwrap();
    assert_eq!(document.path, nested.join("pyproject.toml"));
    let patch = build_rule_patch(
        &document,
        &[update(
            RuleConfigScope::Project,
            "BSK-E0001",
            Some(RuleSeverity::Info),
        )],
    )
    .unwrap();
    apply_config_patch(&patch).unwrap();
    let written = std::fs::read_to_string(nested.join("pyproject.toml")).unwrap();
    assert_eq!(written, patch.content);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn apply_config_patch_creates_a_brand_new_pyproject_file() {
    let root = temp_root("apply-new-toml");
    // No source exists yet: discovery targets the (absent) pyproject.toml and
    // applying a patch must create it from empty.
    let document = discover_config_document(&root).unwrap();
    assert!(!document.exists);
    let patch = build_rule_patch(
        &document,
        &[update(
            RuleConfigScope::Project,
            "BSK-E0001",
            Some(RuleSeverity::Warning),
        )],
    )
    .unwrap();
    apply_config_patch(&patch).unwrap();
    let written = std::fs::read_to_string(root.join("pyproject.toml")).unwrap();
    let table: toml::Table = written.parse().unwrap();
    assert_eq!(
        table["tool"]["basilisk"]["rules"]["BSK-E0001"].as_str(),
        Some("warning")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn apply_config_patch_reports_rename_failure_and_cleans_up_temp() {
    let root = temp_root("apply-rename-fail");
    // Point the patch target at an existing non-empty directory: the base
    // revision matches empty content (path is not a file), create_dir_all on
    // the parent succeeds, the temp file writes, but the final rename onto a
    // populated directory fails, exercising the error + temp-cleanup branch.
    let target = root.join("target-dir");
    std::fs::create_dir_all(target.join("occupied")).unwrap();
    let patch = super::ConfigPatch {
        path: target.clone(),
        base_revision: content_revision(""),
        content: "x = 1\n".to_owned(),
        revision: content_revision("x = 1\n"),
        config: crate::BasiliskConfig::default(),
    };
    let error = apply_config_patch(&patch).unwrap_err();
    assert!(matches!(error, ConfigDocumentError::Read { .. }));
    // No stray temp file was left behind in the parent directory.
    let leftover: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".basilisk-config-")
        })
        .collect();
    assert!(leftover.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn apply_config_patch_reports_unreadable_existing_source() {
    use std::os::unix::fs::PermissionsExt as _;
    let root = temp_root("apply-unreadable");
    let source_path = root.join("pyproject.toml");
    std::fs::write(&source_path, "x = 1\n").unwrap();
    // Strip all permissions: the file still reports as a file but cannot be
    // read, exercising the read-error branch of the revision check.
    std::fs::set_permissions(&source_path, std::fs::Permissions::from_mode(0o000)).unwrap();
    let patch = super::ConfigPatch {
        path: source_path.clone(),
        base_revision: content_revision("x = 1\n"),
        content: "x = 1\n".to_owned(),
        revision: content_revision("x = 1\n"),
        config: crate::BasiliskConfig::default(),
    };
    let outcome = apply_config_patch(&patch);
    // Restore permissions before asserting so cleanup always succeeds.
    std::fs::set_permissions(&source_path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(matches!(outcome, Err(ConfigDocumentError::Read { .. })));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn apply_config_patch_reports_create_dir_all_failure() {
    let root = temp_root("apply-mkdir-fail");
    // The parent component is a regular file, so create_dir_all cannot make the
    // directory the temp write needs.
    let blocking_file = root.join("not-a-dir");
    std::fs::write(&blocking_file, "i am a file").unwrap();
    let target = blocking_file.join("pyproject.toml");
    let patch = super::ConfigPatch {
        path: target,
        base_revision: content_revision(""),
        content: "x = 1\n".to_owned(),
        revision: content_revision("x = 1\n"),
        config: crate::BasiliskConfig::default(),
    };
    let error = apply_config_patch(&patch).unwrap_err();
    assert!(matches!(error, ConfigDocumentError::Read { .. }));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn apply_config_patch_conflict_when_file_appears_under_empty_base() {
    let root = temp_root("apply-conflict-new");
    let source_path = root.join("pyproject.toml");
    // Patch was built against a non-existent source (empty base revision), but
    // a file now exists on disk with different content.
    std::fs::write(&source_path, "[tool.basilisk]\n").unwrap();
    let patch = super::ConfigPatch {
        path: source_path.clone(),
        base_revision: content_revision(""),
        content: "x = 1\n".to_owned(),
        revision: content_revision("x = 1\n"),
        config: crate::BasiliskConfig::default(),
    };
    let error = apply_config_patch(&patch).unwrap_err();
    assert!(matches!(
        error,
        ConfigDocumentError::RevisionConflict { .. }
    ));
    let _ = std::fs::remove_dir_all(root);
}
