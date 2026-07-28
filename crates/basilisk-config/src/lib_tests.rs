//! Discovery and merge tests for [CHKARCH-CONFIG-DISCOVERY] / [CHKARCH-CONFIG-MODEL]
//! in `lib.rs`. Split out to keep that file under the repository size ceiling;
//! `#[path]`-included, so it stays the same `tests` module it always was.

use super::*;
use std::fs;

fn with_temp_cfg_dir(unique: &str, files: &[(&str, &str)], check: impl FnOnce(BasiliskConfig)) {
    let dir = std::env::temp_dir().join(unique);
    fs::create_dir_all(&dir).unwrap();
    for (name, contents) in files {
        fs::write(dir.join(name), contents).unwrap();
    }
    check(load_basilisk_config(&dir));
    let _ = fs::remove_dir_all(&dir);
}

/// [CHKARCH-CONFIG-MODEL]: no config table anywhere means an empty rule
/// chain — the caller's scope default applies (PEP at error, nothing
/// else runs).
#[test]
fn load_default_when_no_config() {
    with_temp_cfg_dir("bsk_cfg_empty_xm", &[], |cfg| {
        assert!(!cfg.exclude.is_empty(), "defaults must include excludes");
        assert!(
            cfg.exclude.iter().any(|e| e == "site-packages"),
            "default excludes must contain site-packages"
        );
        assert!(cfg.stub_paths.is_empty());
        assert!(cfg.typeshed_path.is_none());
        assert!(!cfg.has_config_table(), "no table anywhere -> empty chain");
        assert_eq!(cfg.resolve_severity("anything", &["pep"]), None);
    });
}

/// [CHKARCH-CONFIG-FILE]: `[tool.basilisk.rules]` and
/// `[tool.basilisk.rule-tags]` parse into one folder table.
#[test]
fn load_from_pyproject_toml() {
    with_temp_cfg_dir(
        "bsk_cfg_pyproject_xm",
        &[(
            "pyproject.toml",
            r#"
[tool.basilisk]
stub-paths = ["stubs/", "typings/"]
typeshed-path = "typeshed-mp"

[tool.basilisk.rules]
"imports_unresolved" = "warning"
"BSK-0001" = "disabled"

[tool.basilisk.rule-tags]
"basilisk" = "error"
"#,
        )],
        |cfg| {
            assert_eq!(cfg.stub_paths.len(), 2);
            assert_eq!(
                cfg.typeshed_path,
                Some(std::path::PathBuf::from("typeshed-mp"))
            );
            let tables = cfg.nearest_tables().unwrap();
            assert_eq!(
                tables.rules.get("imports_unresolved").copied(),
                Some(RuleSeverity::Warning)
            );
            assert_eq!(
                tables.rules.get("BSK-0001").copied(),
                Some(RuleSeverity::Disabled)
            );
            assert_eq!(
                tables.rule_tags.get("basilisk").copied(),
                Some(RuleSeverity::Error)
            );
        },
    );
}

/// [STUBRES-TYPESHED-CONFIG]: the whole runtime typeshed surface is three
/// string keys, parsed verbatim.
#[test]
fn runtime_typeshed_settings_parse_as_one_source_policy() {
    with_temp_cfg_dir(
        "bsk_cfg_typeshed_runtime_xm",
        &[(
            "pyproject.toml",
            r#"
[tool.basilisk]
typeshed-path = "custom-typeshed"
typeshed-commit = "83c2518a9e6abbda0c44592c3483de459198f887"
typeshed-store-path = ".cache/typeshed-store"
"#,
        )],
        |cfg| {
            assert_eq!(
                cfg.typeshed_path,
                Some(std::path::PathBuf::from("custom-typeshed"))
            );
            assert_eq!(
                cfg.typeshed_commit.as_deref(),
                Some("83c2518a9e6abbda0c44592c3483de459198f887")
            );
            assert_eq!(
                cfg.typeshed_store_path,
                Some(std::path::PathBuf::from(".cache/typeshed-store"))
            );
            assert_eq!(cfg.python_version, None);
        },
    );
}

/// [STUBRES-TYPESHED-CONFIG]: unset keys stay `None`, so the runtime uses
/// the bundled commit (with `typeshed_source_unpinned`) and the OS store directory —
/// never a download.
#[test]
fn typeshed_acquisition_keys_default_to_none() {
    with_temp_cfg_dir(
        "bsk_cfg_typeshed_keys_unset_xm",
        &[("pyproject.toml", "[tool.basilisk]\n")],
        |cfg| {
            assert!(cfg.typeshed_commit.is_none());
            assert!(cfg.typeshed_path.is_none());
            assert!(cfg.typeshed_store_path.is_none());
        },
    );
}

/// [CHKARCH-CONFIG-MODEL] resolution: within a table a per-rule entry
/// beats tag entries.
#[test]
fn rule_entry_beats_tag_entry_within_a_table() {
    with_temp_cfg_dir(
        "bsk_cfg_rule_over_tag_xm",
        &[(
            "pyproject.toml",
            r#"
[tool.basilisk.rules]
"BSK-0050" = "warning"

[tool.basilisk.rule-tags]
"basilisk" = "error"
"#,
        )],
        |cfg| {
            assert_eq!(
                cfg.resolve_severity("BSK-0050", &["basilisk", "redundancy"]),
                Some(RuleSeverity::Warning),
                "the per-rule entry must beat the tag entry"
            );
            assert_eq!(
                cfg.resolve_severity("BSK-0001", &["basilisk"]),
                Some(RuleSeverity::Error),
                "rules without their own entry take the tag entry"
            );
            assert_eq!(
                cfg.resolve_severity("returns_compatibility", &["pep"]),
                None,
                "rules no table decides resolve to None"
            );
        },
    );
}

/// [CHKARCH-CONFIG-MODEL] resolution: among matching tag entries the
/// strictest severity wins (error > warning > info > disabled).
#[test]
fn strictest_matching_tag_entry_wins() {
    with_temp_cfg_dir(
        "bsk_cfg_strictest_tag_xm",
        &[(
            "pyproject.toml",
            r#"
[tool.basilisk.rule-tags]
"basilisk" = "info"
"suppressions" = "error"
"style" = "disabled"
"#,
        )],
        |cfg| {
            assert_eq!(
                cfg.resolve_severity("BSK-0061", &["basilisk", "suppressions"]),
                Some(RuleSeverity::Error),
                "error must beat info among overlapping tag entries"
            );
            assert_eq!(
                cfg.resolve_severity("BSK-0014", &["basilisk", "style"]),
                Some(RuleSeverity::Info),
                "info must beat disabled among overlapping tag entries"
            );
        },
    );
}

/// The legacy `basilisk.json` format is never read: a directory holding
/// only a (formerly valid) `basilisk.json` yields the default config.
#[test]
fn basilisk_json_is_ignored() {
    with_temp_cfg_dir(
        "bsk_cfg_json_xm",
        &[(
            "basilisk.json",
            r#"{
            "stubPaths": ["stubs/"],
            "rules": { "imports_unresolved": "info" }
        }"#,
        )],
        |cfg| {
            assert!(cfg.stub_paths.is_empty());
            assert!(!cfg.has_config_table());
        },
    );
}

#[test]
fn toml_exclude_overrides_defaults() {
    with_temp_cfg_dir(
        "bsk_cfg_toml_exclude_xm",
        &[(
            "pyproject.toml",
            r#"
[tool.basilisk]
exclude = ["legacy", "third_party"]
"#,
        )],
        |cfg| {
            assert_eq!(cfg.exclude, vec!["legacy", "third_party"]);
        },
    );
}

/// GitHub #311: rule config must be discovered by walking ancestor
/// directories, not just the exact directory passed in. See
/// [CHKARCH-CONFIG-DISCOVERY].
#[test]
fn discovers_config_from_ancestor_directory() {
    let root = std::env::temp_dir().join(format!("bsk_cfg_walk_up_{}", std::process::id()));
    let child = root.join("nested").join("pkg");
    fs::create_dir_all(&child).unwrap();
    fs::write(
        root.join("pyproject.toml"),
        "[tool.basilisk.rules]\n\"imports_unresolved\" = \"warning\"\n",
    )
    .unwrap();

    let cfg = load_basilisk_config(&child);
    let _ = fs::remove_dir_all(&root);

    assert_eq!(
        cfg.resolve_severity("imports_unresolved", &["pep", "imports"]),
        Some(RuleSeverity::Warning),
        "loading config from a child directory must discover the ancestor's \
         pyproject.toml [tool.basilisk] (GitHub #311)"
    );
}

/// [CHKARCH-CONFIG-MODEL]: the nearest table that decides a rule wins
/// outright — including a child tag entry beating an ancestor per-rule
/// entry, because proximity beats specificity across tables.
#[test]
fn nearest_deciding_table_wins_across_folders() {
    let root = std::env::temp_dir().join(format!("bsk_cfg_nearest_{}", std::process::id()));
    let child = root.join("child");
    fs::create_dir_all(&child).unwrap();
    fs::write(
        root.join("pyproject.toml"),
        "[tool.basilisk.rules]\n\
         \"imports_unresolved\" = \"warning\"\n\
         \"BSK-0001\" = \"error\"\n",
    )
    .unwrap();
    fs::write(
        child.join("pyproject.toml"),
        "[tool.basilisk.rule-tags]\n\"basilisk\" = \"info\"\n",
    )
    .unwrap();

    let cfg = load_basilisk_config(&child);
    let _ = fs::remove_dir_all(&root);

    assert_eq!(
        cfg.resolve_severity("BSK-0001", &["basilisk"]),
        Some(RuleSeverity::Info),
        "the child's tag entry must beat the ancestor's per-rule entry"
    );
    assert_eq!(
        cfg.resolve_severity("imports_unresolved", &["pep", "imports"]),
        Some(RuleSeverity::Warning),
        "rules the child does not decide fall through to the ancestor"
    );
}

/// [CHKARCH-CONFIG-MODEL]: an explicitly empty table decides nothing but
/// still counts as an existing table (the LSP seed distinction).
#[test]
fn empty_table_exists_but_decides_nothing() {
    with_temp_cfg_dir(
        "bsk_cfg_empty_table_xm",
        &[("pyproject.toml", "[tool.basilisk]\n")],
        |cfg| {
            assert!(cfg.has_config_table(), "an empty table still exists");
            assert_eq!(cfg.resolve_severity("BSK-0001", &["basilisk"]), None);
            assert_eq!(
                cfg.resolve_severity("returns_compatibility", &["pep"]),
                None
            );
        },
    );
}

/// [CHKARCH-CONFIG-MODEL]: `RuleTables::is_empty` distinguishes an empty
/// table (exists, decides nothing) from one carrying entries.
#[test]
fn rule_tables_report_emptiness() {
    assert!(RuleTables::default().is_empty());
    let tagged = RuleTables {
        rules: std::collections::HashMap::new(),
        rule_tags: std::collections::HashMap::from([("basilisk".to_owned(), RuleSeverity::Error)]),
    };
    assert!(!tagged.is_empty());
    let ruled = RuleTables {
        rules: std::collections::HashMap::from([("BSK-0001".to_owned(), RuleSeverity::Info)]),
        rule_tags: std::collections::HashMap::new(),
    };
    assert!(!ruled.is_empty());
}

/// [CHKARCH-CONFIG-MODEL]: a key the parser does not implement must not be
/// quietly absorbed. `auto-stub-mode`/`auto-stub-path` used to parse, merge
/// and carry a default while NO consumer ever read them, so setting one was
/// a silent no-op. They are gone; this pins that they stay gone rather than
/// returning as dead surface a reader would reasonably trust.
#[test]
fn retired_auto_stub_keys_are_not_resurrected_as_silent_no_ops() {
    with_temp_cfg_dir(
        "bsk_cfg_auto_stub_xm",
        &[(
            "pyproject.toml",
            "[tool.basilisk]\nauto-stub-mode = \"runtime\"\nauto-stub-path = \"stubs_gen\"\n",
        )],
        |cfg| {
            // The file still parses — an unknown key is not an error …
            assert!(
                cfg.rule_chain.iter().all(RuleTables::is_empty),
                "the retired keys must not manufacture rule entries"
            );
            // … and grants no behaviour: this table decides nothing, exactly
            // like the empty table it now is.
            assert_eq!(cfg.resolve_severity("BSK-0001", &[]), None);
            assert_eq!(
                cfg.resolve_severity("returns_compatibility", &["pep"]),
                None
            );
        },
    );
}
