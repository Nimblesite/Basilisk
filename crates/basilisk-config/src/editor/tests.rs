use std::collections::BTreeMap;
use std::path::PathBuf;

use super::{
    adoption_rule_overrides, apply_config_patch, build_rule_patch, content_revision,
    discover_config_document, discover_config_document_with_content, validate_content,
    ConfigDocument, ConfigDocumentError, ConfigFormat, RuleConfigScope, RuleConfigUpdate,
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
