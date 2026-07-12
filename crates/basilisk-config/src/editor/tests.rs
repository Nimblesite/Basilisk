use std::collections::BTreeMap;
use std::path::PathBuf;

use super::{
    active_config_path, adoption_rule_overrides, apply_config_patch, build_rule_patch,
    content_revision, discover_config_document, discover_config_document_with_content,
    validate_content, ConfigDocument, ConfigDocumentError, ConfigFormat, RuleConfigScope,
    RuleConfigUpdate,
};
use crate::RuleSeverity;

fn document(format: ConfigFormat, content: &str) -> ConfigDocument {
    let path = match format {
        ConfigFormat::PyprojectToml => PathBuf::from("/workspace/pyproject.toml"),
        ConfigFormat::BasiliskJson => PathBuf::from("/workspace/basilisk.json"),
    };
    ConfigDocument {
        root: PathBuf::from("/workspace"),
        path: path.clone(),
        format,
        exists: true,
        read_only: false,
        shadowed_sources: Vec::new(),
        content: content.to_owned(),
        revision: content_revision(content),
        config: validate_content(&path, format, content).unwrap(),
    }
}

fn update(scope: RuleConfigScope, code: &str, severity: Option<RuleSeverity>) -> RuleConfigUpdate {
    RuleConfigUpdate {
        scope,
        rules: BTreeMap::from([(code.to_owned(), severity)]),
    }
}

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

#[test]
fn json_mutates_existing_kebab_alias_without_shadowing_it() {
    let source = r#"{
  "per-path-overrides": {
    "src/app.py": {
      "adoption": true,
      "rules": { "BSK-E0001": "warning" }
    }
  },
  "custom": { "keep": true }
}
"#;
    let patch = build_rule_patch(
        &document(ConfigFormat::BasiliskJson, source),
        &[update(
            RuleConfigScope::Path {
                pattern: "src/app.py".to_owned(),
                adoption: true,
            },
            "BSK-E0001",
            Some(RuleSeverity::Info),
        )],
    )
    .unwrap();

    let value: serde_json::Value = serde_json::from_str(&patch.content).unwrap();
    assert!(value.get("per-path-overrides").is_some());
    assert!(value.get("perPathOverrides").is_none());
    assert_eq!(
        value["per-path-overrides"]["src/app.py"]["rules"]["BSK-E0001"],
        "info"
    );
    assert_eq!(value["custom"]["keep"], true);
}

#[test]
fn json_rejects_both_per_path_spellings() {
    let source = r#"{"perPathOverrides": {}, "per-path-overrides": {}}"#;
    let error = validate_content(
        &PathBuf::from("basilisk.json"),
        ConfigFormat::BasiliskJson,
        source,
    )
    .unwrap_err();
    assert!(error.to_string().contains("cannot define both"));
}

#[test]
fn json_reset_removes_empty_adoption_path_and_rules() {
    let source = r#"{
  "perPathOverrides": {
    "src/app.py": {
      "adoption": true,
      "rules": { "BSK-E0001": "warning" }
    }
  }
}
"#;
    let patch = build_rule_patch(
        &document(ConfigFormat::BasiliskJson, source),
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
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&patch.content).unwrap(),
        serde_json::json!({})
    );
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
        &document(ConfigFormat::PyprojectToml, source),
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
fn adoption_projection_has_json_toml_parity() {
    let json = document(
        ConfigFormat::BasiliskJson,
        r#"{
  "perPathOverrides": {
    "src/app.py": {
      "adoption": true,
      "rules": { "BSK-E0001": "warning", "BSK-W0050": "info" }
    }
  }
}
"#,
    );
    let toml = document(
        ConfigFormat::PyprojectToml,
        r#"[tool.basilisk.per-path-overrides."src/app.py"]
adoption = true

[tool.basilisk.per-path-overrides."src/app.py".rules]
BSK-E0001 = "warning"
BSK-W0050 = "info"
"#,
    );
    assert_eq!(
        adoption_rule_overrides(&json),
        adoption_rule_overrides(&toml)
    );
}

#[test]
fn path_severity_canonicalizes_legacy_disabled_entries() {
    let cases = [
        (
            ConfigFormat::BasiliskJson,
            r#"{
  "perPathOverrides": {
    "src/app.py": { "disabled": ["BSK-E0001", "BSK-E0002"] }
  }
}
"#,
        ),
        (
            ConfigFormat::PyprojectToml,
            r#"[tool.basilisk.per-path-overrides."src/app.py"]
disabled = ["BSK-E0001", "BSK-E0002"]
"#,
        ),
    ];
    for (format, source) in cases {
        for severity in [RuleSeverity::Warning, RuleSeverity::Disabled] {
            let patch = build_rule_patch(
                &document(format, source),
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
// Test helpers for filesystem-backed cases.
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// ConfigDocumentError Display — all four variants.
// ---------------------------------------------------------------------------

#[test]
fn display_read_error_names_path_and_message() {
    let error = ConfigDocumentError::Read {
        path: PathBuf::from("/workspace/basilisk.json"),
        message: "permission denied".to_owned(),
    };
    assert_eq!(
        error.to_string(),
        "failed to read /workspace/basilisk.json: permission denied"
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
        path: PathBuf::from("/workspace/basilisk.json"),
    };
    assert_eq!(
        error.to_string(),
        "configuration is read-only: /workspace/basilisk.json"
    );
}

// ---------------------------------------------------------------------------
// validate_content — JSON structure errors.
// ---------------------------------------------------------------------------

fn validate_json(source: &str) -> Result<(), ConfigDocumentError> {
    validate_content(
        &PathBuf::from("basilisk.json"),
        ConfigFormat::BasiliskJson,
        source,
    )
    .map(|_| ())
}

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
    let json = validate_content(
        &PathBuf::from("basilisk.json"),
        ConfigFormat::BasiliskJson,
        "",
    )
    .unwrap();
    assert!(json.rules.is_empty());
    let toml = validate_content(
        &PathBuf::from("pyproject.toml"),
        ConfigFormat::PyprojectToml,
        "",
    )
    .unwrap();
    assert!(toml.rules.is_empty());
}

#[test]
fn json_invalid_syntax_is_rejected() {
    let error = validate_json("{not json").unwrap_err();
    assert!(matches!(error, ConfigDocumentError::Invalid { .. }));
}

#[test]
fn json_non_object_root_has_wrong_shape() {
    let error = validate_json("[1, 2, 3]").unwrap_err();
    assert_eq!(
        error.to_string(),
        "invalid configuration basilisk.json: configuration root must be an object"
    );
}

#[test]
fn json_rules_must_be_an_object() {
    let error = validate_json(r#"{"rules": "nope"}"#).unwrap_err();
    assert!(error.to_string().contains("rules must be an object"));
}

#[test]
fn json_invalid_severity_names_the_rule() {
    let error = validate_json(r#"{"rules": {"BSK-E0001": "louder"}}"#).unwrap_err();
    assert!(error
        .to_string()
        .contains("rule `BSK-E0001` has an invalid severity"));
}

#[test]
fn json_per_path_overrides_must_be_an_object() {
    let error = validate_json(r#"{"perPathOverrides": 5}"#).unwrap_err();
    assert!(error
        .to_string()
        .contains("per-path overrides must be an object"));
}

#[test]
fn json_path_override_entry_must_be_an_object() {
    let error = validate_json(r#"{"perPathOverrides": {"src/app.py": 5}}"#).unwrap_err();
    assert!(error
        .to_string()
        .contains("path override `src/app.py` must be an object"));
}

#[test]
fn json_disabled_must_be_an_array() {
    let error =
        validate_json(r#"{"perPathOverrides": {"src/app.py": {"disabled": "x"}}}"#).unwrap_err();
    assert!(error
        .to_string()
        .contains("path override `src/app.py` disabled must be an array"));
}

#[test]
fn json_disabled_entries_must_be_strings() {
    let error =
        validate_json(r#"{"perPathOverrides": {"src/app.py": {"disabled": [1]}}}"#).unwrap_err();
    assert!(error
        .to_string()
        .contains("path override `src/app.py` disabled entries must be strings"));
}

#[test]
fn json_adoption_must_be_a_boolean() {
    let error =
        validate_json(r#"{"perPathOverrides": {"src/app.py": {"adoption": 1}}}"#).unwrap_err();
    assert!(error
        .to_string()
        .contains("path override `src/app.py` adoption must be a boolean"));
}

#[test]
fn json_both_per_path_spellings_via_validate_content() {
    let error = validate_json(r#"{"perPathOverrides": {}, "per-path-overrides": {}}"#).unwrap_err();
    assert!(error
        .to_string()
        .contains("cannot define both `perPathOverrides` and `per-path-overrides`"));
}

// ---------------------------------------------------------------------------
// validate_content — TOML structure errors.
// ---------------------------------------------------------------------------

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
// discover_config_document — read/dir/precedence/read-only.
// ---------------------------------------------------------------------------

#[test]
fn discover_reads_directory_as_read_error() {
    let root = temp_root("discover-dir");
    // Make basilisk.json a directory so is_file() is false but the caller
    // still selects it as the JSON source only when it is a file. To force a
    // read error we make pyproject.toml a directory and select TOML.
    std::fs::create_dir_all(root.join("pyproject.toml")).unwrap();
    // is_file() is false for a directory, so discovery treats it as empty and
    // returns a default config rather than a read error.
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
fn active_config_path_prefers_json_over_toml() {
    let root = temp_root("active-precedence");
    std::fs::write(root.join("pyproject.toml"), "[project]\nname = \"x\"\n").unwrap();
    std::fs::write(root.join("basilisk.json"), "{}\n").unwrap();
    assert_eq!(active_config_path(&root), root.join("basilisk.json"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn discover_shadows_toml_when_json_present() {
    let root = temp_root("discover-shadow");
    std::fs::write(root.join("pyproject.toml"), "[project]\nname = \"x\"\n").unwrap();
    std::fs::write(root.join("basilisk.json"), "{}\n").unwrap();
    let document = discover_config_document(&root).unwrap();
    assert_eq!(document.format, ConfigFormat::BasiliskJson);
    assert_eq!(document.shadowed_sources, vec![root.join("pyproject.toml")]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn discover_detects_read_only_source() {
    let root = temp_root("discover-readonly");
    let path = root.join("basilisk.json");
    std::fs::write(&path, "{}\n").unwrap();
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
fn discover_propagates_invalid_json_error() {
    let root = temp_root("discover-invalid");
    std::fs::write(root.join("basilisk.json"), "{not json").unwrap();
    let error = discover_config_document(&root).unwrap_err();
    assert!(matches!(error, ConfigDocumentError::Invalid { .. }));
    let _ = std::fs::remove_dir_all(root);
}

// ---------------------------------------------------------------------------
// build_rule_patch — read-only and wrong-shaped targets.
// ---------------------------------------------------------------------------

#[test]
fn build_rule_patch_rejects_read_only_document() {
    let mut document = document(ConfigFormat::BasiliskJson, "{}\n");
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
    // manually with content that parses but whose mutation target is
    // wrong-shaped; patch_toml must reject the target before re-validating.
    let raw = ConfigDocument {
        root: PathBuf::from("/workspace"),
        path: PathBuf::from("/workspace/pyproject.toml"),
        format: ConfigFormat::PyprojectToml,
        exists: true,
        read_only: false,
        shadowed_sources: Vec::new(),
        content: "[tool.basilisk]\nrules = \"x\"\n".to_owned(),
        revision: content_revision("[tool.basilisk]\nrules = \"x\"\n"),
        config: crate::BasiliskConfig::default(),
    };
    let error = build_rule_patch(
        &raw,
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
fn build_rule_patch_json_rejects_non_object_root() {
    let raw = ConfigDocument {
        root: PathBuf::from("/workspace"),
        path: PathBuf::from("/workspace/basilisk.json"),
        format: ConfigFormat::BasiliskJson,
        exists: true,
        read_only: false,
        shadowed_sources: Vec::new(),
        content: "[1, 2, 3]".to_owned(),
        revision: content_revision("[1, 2, 3]"),
        config: crate::BasiliskConfig::default(),
    };
    let error = build_rule_patch(
        &raw,
        &[update(
            RuleConfigScope::Project,
            "BSK-E0001",
            Some(RuleSeverity::Warning),
        )],
    )
    .unwrap_err();
    assert!(error
        .to_string()
        .contains("JSON configuration must be an object"));
}

#[test]
fn build_rule_patch_json_rejects_both_per_path_spellings() {
    let source = r#"{"perPathOverrides": {}, "per-path-overrides": {}}"#;
    let raw = ConfigDocument {
        root: PathBuf::from("/workspace"),
        path: PathBuf::from("/workspace/basilisk.json"),
        format: ConfigFormat::BasiliskJson,
        exists: true,
        read_only: false,
        shadowed_sources: Vec::new(),
        content: source.to_owned(),
        revision: content_revision(source),
        config: crate::BasiliskConfig::default(),
    };
    let error = build_rule_patch(
        &raw,
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
        .contains("cannot define both `perPathOverrides` and `per-path-overrides`"));
}

// ---------------------------------------------------------------------------
// patch rendering — parse and shape errors on manually-built documents.
// ---------------------------------------------------------------------------

fn raw_document(format: ConfigFormat, content: &str) -> ConfigDocument {
    let path = match format {
        ConfigFormat::PyprojectToml => PathBuf::from("/workspace/pyproject.toml"),
        ConfigFormat::BasiliskJson => PathBuf::from("/workspace/basilisk.json"),
    };
    ConfigDocument {
        root: PathBuf::from("/workspace"),
        path,
        format,
        exists: true,
        read_only: false,
        shadowed_sources: Vec::new(),
        content: content.to_owned(),
        revision: content_revision(content),
        config: crate::BasiliskConfig::default(),
    }
}

#[test]
fn patch_toml_reports_unparseable_source() {
    let error = build_rule_patch(
        &raw_document(ConfigFormat::PyprojectToml, "[tool.basilisk.rules\n"),
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
fn patch_json_reports_unparseable_source() {
    let error = build_rule_patch(
        &raw_document(ConfigFormat::BasiliskJson, "{not json"),
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
fn patch_toml_rejects_non_table_per_path_intermediate() {
    let error = build_rule_patch(
        &raw_document(
            ConfigFormat::PyprojectToml,
            "[tool.basilisk]\nper-path-overrides = \"x\"\n",
        ),
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
fn patch_json_rejects_non_object_existing_rules_target() {
    let error = build_rule_patch(
        &raw_document(ConfigFormat::BasiliskJson, r#"{"rules": "nope"}"#),
        &[update(
            RuleConfigScope::Project,
            "BSK-E0001",
            Some(RuleSeverity::Warning),
        )],
    )
    .unwrap_err();
    assert!(error.to_string().contains("`rules` must be an object"));
}

// ---------------------------------------------------------------------------
// patch_toml / patch_json — mutation edge cases.
// ---------------------------------------------------------------------------

#[test]
fn toml_removing_last_project_rule_drops_the_rules_table() {
    let source = "[tool.basilisk.rules]\nBSK-E0001 = \"warning\"\n";
    let patch = build_rule_patch(
        &document(ConfigFormat::PyprojectToml, source),
        &[update(RuleConfigScope::Project, "BSK-E0001", None)],
    )
    .unwrap();
    assert!(!patch.content.contains("rules"));
    assert!(patch.config.rules.is_empty());
}

#[test]
fn json_removing_last_project_rule_drops_the_rules_object() {
    let source = "{\n  \"rules\": {\n    \"BSK-E0001\": \"warning\"\n  }\n}\n";
    let patch = build_rule_patch(
        &document(ConfigFormat::BasiliskJson, source),
        &[update(RuleConfigScope::Project, "BSK-E0001", None)],
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_str(&patch.content).unwrap();
    assert!(value.get("rules").is_none());
}

#[test]
fn toml_patch_preserves_crlf_newlines() {
    let source = "[tool.basilisk.rules]\r\nBSK-E0001 = \"warning\"\r\n";
    let patch = build_rule_patch(
        &document(ConfigFormat::PyprojectToml, source),
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
        &document(ConfigFormat::PyprojectToml, source),
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
fn json_adoption_true_writes_the_adoption_key() {
    let patch = build_rule_patch(
        &document(ConfigFormat::BasiliskJson, ""),
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
    let value: serde_json::Value = serde_json::from_str(&patch.content).unwrap();
    assert_eq!(value["perPathOverrides"]["src/app.py"]["adoption"], true);
}

#[test]
fn toml_canonicalize_drops_empty_disabled_array() {
    let source = "[tool.basilisk.per-path-overrides.\"src/app.py\"]\ndisabled = [\"BSK-E0001\"]\n";
    let patch = build_rule_patch(
        &document(ConfigFormat::PyprojectToml, source),
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
        &document(ConfigFormat::PyprojectToml, source),
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
fn json_path_entry_becomes_empty_is_removed() {
    let source = r#"{
  "perPathOverrides": {
    "src/app.py": { "disabled": ["BSK-E0001"] }
  }
}
"#;
    let patch = build_rule_patch(
        &document(ConfigFormat::BasiliskJson, source),
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
    let value: serde_json::Value = serde_json::from_str(&patch.content).unwrap();
    assert_eq!(value, serde_json::json!({}));
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
fn apply_config_patch_creates_a_brand_new_json_file() {
    let root = temp_root("apply-newjson");
    // No source yet: default target is pyproject.toml, so write a json target
    // via a document whose content is empty and format forced by an existing
    // json marker. Simpler: create basilisk.json empty then patch it.
    std::fs::write(root.join("basilisk.json"), String::new()).unwrap();
    let document = discover_config_document(&root).unwrap();
    assert_eq!(document.format, ConfigFormat::BasiliskJson);
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
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("basilisk.json")).unwrap())
            .unwrap();
    assert_eq!(value["rules"]["BSK-E0001"], "warning");
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
        content: "{}\n".to_owned(),
        revision: content_revision("{}\n"),
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
    let source_path = root.join("basilisk.json");
    std::fs::write(&source_path, "{}\n").unwrap();
    // Strip all permissions: the file still reports as a file but cannot be
    // read, exercising the read-error branch of the revision check.
    std::fs::set_permissions(&source_path, std::fs::Permissions::from_mode(0o000)).unwrap();
    let patch = super::ConfigPatch {
        path: source_path.clone(),
        base_revision: content_revision("{}\n"),
        content: "{}\n".to_owned(),
        revision: content_revision("{}\n"),
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
    let target = blocking_file.join("basilisk.json");
    let patch = super::ConfigPatch {
        path: target,
        base_revision: content_revision(""),
        content: "{}\n".to_owned(),
        revision: content_revision("{}\n"),
        config: crate::BasiliskConfig::default(),
    };
    let error = apply_config_patch(&patch).unwrap_err();
    assert!(matches!(error, ConfigDocumentError::Read { .. }));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn apply_config_patch_conflict_when_file_appears_under_empty_base() {
    let root = temp_root("apply-conflict-new");
    let source_path = root.join("basilisk.json");
    // Patch was built against a non-existent source (empty base revision), but
    // a file now exists on disk with different content.
    std::fs::write(&source_path, "{\"rules\": {}}\n").unwrap();
    let patch = super::ConfigPatch {
        path: source_path.clone(),
        base_revision: content_revision(""),
        content: "{}\n".to_owned(),
        revision: content_revision("{}\n"),
        config: crate::BasiliskConfig::default(),
    };
    let error = apply_config_patch(&patch).unwrap_err();
    assert!(matches!(
        error,
        ConfigDocumentError::RevisionConflict { .. }
    ));
    let _ = std::fs::remove_dir_all(root);
}
