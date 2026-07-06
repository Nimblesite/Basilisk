//! Implements [STUBRES-CONFIG]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CONFIG
//! Configuration parsing for Basilisk.
//!
//! Parses `pyproject.toml` `[tool.basilisk]` and `basilisk.json` with support for:
//! - Global rule severity overrides (`rules."imports_unresolved" = "warning"`)
//! - Per-module overrides (`per-module-overrides."fastmcp".ignore-missing-stubs = true`)
//! - Per-path overrides (`per-path-overrides."vendor/**".rules.disabled = [...]`)
//! - Stub path directories (`stub-paths = ["stubs/"]`)

pub mod adoption;
pub mod overrides;
mod parse;

pub use adoption::AdoptionStore;
pub use overrides::{path_matches_pattern, ModuleOverride, PathOverride, RuleSeverity};
pub use parse::BasiliskConfig;

use std::path::Path;

/// Directories excluded from analysis by default.
///
/// Users can override this via the `exclude` key in `basilisk.json` or
/// `pyproject.toml [tool.basilisk]`. Setting `exclude` in config replaces
/// these defaults entirely — add them back explicitly if still needed.
pub const DEFAULT_EXCLUDES: &[&str] = &[
    "__pycache__",
    "node_modules",
    "venv",
    ".venv",
    "env",
    ".env",
    ".tox",
    ".mypy_cache",
    ".ruff_cache",
    ".pytest_cache",
    "site-packages",
    "__pypackages__",
    "build",
    "dist",
    ".eggs",
    // Vendored / bundled third-party code shipped verbatim (e.g. the extension's
    // `bundled/debugpy` tree and its nested `_vendored/`). Never our code to
    // type-check; scanning it floods thousands of irrelevant diagnostics (#80).
    "bundled",
    "_vendored",
];

/// Load a `BasiliskConfig` from the first config file found in `root`.
///
/// Implements [CHKARCH-CONFIG-FILE]. Search order (highest priority wins):
/// 1. `basilisk.json`
/// 2. `pyproject.toml` `[tool.basilisk]`
///
/// Returns `BasiliskConfig::default()` if no config file is found (and likewise
/// on a malformed file — no configuration-error exit is raised; see report).
/// See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-FILE
#[must_use]
pub fn load_basilisk_config(root: &Path) -> BasiliskConfig {
    // 1. basilisk.json
    let basilisk_json = root.join("basilisk.json");
    if basilisk_json.is_file() {
        if let Some(cfg) = parse::load_from_json(&basilisk_json) {
            return cfg;
        }
    }

    // 2. pyproject.toml [tool.basilisk]
    let pyproject = root.join("pyproject.toml");
    if pyproject.is_file() {
        if let Some(cfg) = parse::load_from_pyproject(&pyproject) {
            return cfg;
        }
    }

    BasiliskConfig::default()
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only code: unwrap acceptable in unit tests"
)]
mod tests {
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

    #[test]
    fn load_default_when_no_config() {
        with_temp_cfg_dir("bsk_cfg_empty_xm", &[], |cfg| {
            assert!(!cfg.exclude.is_empty(), "defaults must include excludes");
            assert!(
                cfg.exclude.iter().any(|e| e == "site-packages"),
                "default excludes must contain site-packages"
            );
            assert!(
                cfg.exclude.iter().any(|e| e == "__pycache__"),
                "default excludes must contain __pycache__"
            );
            assert!(cfg.stub_paths.is_empty());
            assert!(cfg.typeshed_path.is_none());
            assert!(cfg.rules.is_empty());
            assert!(cfg.per_module_overrides.is_empty());
            assert!(cfg.per_path_overrides.is_empty());
        });
    }

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
"BSK-E0001" = "disabled"

[tool.basilisk.per-module-overrides.fastmcp]
ignore-missing-stubs = true

[tool.basilisk.per-module-overrides."django.*"]
ignore-missing-stubs = true

[tool.basilisk.per-path-overrides."vendor/**"]
disabled = ["imports_unresolved", "BSK-E0001"]
"#,
            )],
            |cfg| {
                assert_eq!(cfg.stub_paths.len(), 2);
                assert_eq!(
                    cfg.typeshed_path,
                    Some(std::path::PathBuf::from("typeshed-mp"))
                );
                assert_eq!(cfg.rules.len(), 2);
                assert_eq!(
                    cfg.rules.get("imports_unresolved").copied(),
                    Some(RuleSeverity::Warning)
                );
                assert_eq!(
                    cfg.rules.get("BSK-E0001").copied(),
                    Some(RuleSeverity::Disabled)
                );
                assert!(cfg.per_module_overrides.contains_key("fastmcp"));
                assert!(cfg.per_module_overrides.contains_key("django.*"));
                assert!(cfg.per_path_overrides.contains_key("vendor/**"));
            },
        );
    }

    #[test]
    fn load_from_basilisk_json() {
        with_temp_cfg_dir(
            "bsk_cfg_json_xm",
            &[(
                "basilisk.json",
                r#"{
                "stubPaths": ["stubs/"],
                "typeshedPath": "ts-json",
                "rules": {
                    "imports_unresolved": "info"
                },
                "perModuleOverrides": {
                    "requests": { "ignoreMissingStubs": true }
                }
            }"#,
            )],
            |cfg| {
                assert_eq!(cfg.stub_paths.len(), 1);
                assert_eq!(cfg.typeshed_path, Some(std::path::PathBuf::from("ts-json")));
                assert_eq!(
                    cfg.rules.get("imports_unresolved").copied(),
                    Some(RuleSeverity::Info)
                );
                assert!(cfg.per_module_overrides.contains_key("requests"));
            },
        );
    }

    #[test]
    fn basilisk_json_takes_priority() {
        with_temp_cfg_dir(
            "bsk_cfg_priority_xm",
            &[
                ("basilisk.json", r#"{ "stubPaths": ["from_json/"] }"#),
                (
                    "pyproject.toml",
                    "[tool.basilisk]\nstub-paths = [\"from_toml/\"]\n",
                ),
            ],
            |cfg| {
                assert_eq!(cfg.stub_paths.len(), 1);
                assert_eq!(cfg.stub_paths.first().unwrap().to_str(), Some("from_json/"));
            },
        );
    }

    #[test]
    fn module_override_wildcard_matching() {
        let override_entry = ModuleOverride {
            ignore_missing_stubs: true,
        };
        let cfg = BasiliskConfig {
            per_module_overrides: [("django.*".to_owned(), override_entry)]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        assert!(cfg.should_ignore_missing_stubs("django.db"));
        assert!(cfg.should_ignore_missing_stubs("django.db.models"));
        assert!(!cfg.should_ignore_missing_stubs("flask"));
    }

    #[test]
    fn exact_module_override() {
        let override_entry = ModuleOverride {
            ignore_missing_stubs: true,
        };
        let cfg = BasiliskConfig {
            per_module_overrides: [("fastmcp".to_owned(), override_entry)]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        assert!(cfg.should_ignore_missing_stubs("fastmcp"));
        assert!(cfg.should_ignore_missing_stubs("fastmcp.server"));
        assert!(!cfg.should_ignore_missing_stubs("flask"));
    }

    #[test]
    fn json_exclude_overrides_defaults() {
        with_temp_cfg_dir(
            "bsk_cfg_json_exclude_xm",
            &[("basilisk.json", r#"{ "exclude": ["vendor", "generated"] }"#)],
            |cfg| {
                assert_eq!(cfg.exclude, vec!["vendor", "generated"]);
                // Defaults are replaced, not merged.
                assert!(
                    !cfg.exclude.iter().any(|e| e == "__pycache__"),
                    "custom exclude must replace defaults entirely"
                );
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

    #[test]
    fn json_uv_stub_suggestions_and_dependency_diagnostics() {
        with_temp_cfg_dir(
            "bsk_cfg_json_uv_xm",
            &[(
                "basilisk.json",
                r#"{
                "uv": {
                    "stubSuggestions": false,
                    "dependencyDiagnostics": true
                }
            }"#,
            )],
            |cfg| {
                assert!(
                    !cfg.uv_stub_suggestions,
                    "stubSuggestions should be parsed as false"
                );
                assert!(
                    cfg.uv_dependency_diagnostics,
                    "dependencyDiagnostics should be parsed as true"
                );
            },
        );
    }

    #[test]
    fn json_uv_kebab_case_alternatives() {
        with_temp_cfg_dir(
            "bsk_cfg_json_uv_kebab_xm",
            &[(
                "basilisk.json",
                r#"{
                "uv": {
                    "stub-suggestions": false,
                    "dependency-diagnostics": true
                }
            }"#,
            )],
            |cfg| {
                assert!(
                    !cfg.uv_stub_suggestions,
                    "stub-suggestions kebab key should be accepted"
                );
                assert!(
                    cfg.uv_dependency_diagnostics,
                    "dependency-diagnostics kebab key should be accepted"
                );
            },
        );
    }

    #[test]
    fn toml_uv_stub_suggestions_and_dependency_diagnostics() {
        with_temp_cfg_dir(
            "bsk_cfg_toml_uv_xm",
            &[(
                "pyproject.toml",
                r"
[tool.basilisk.uv]
stub-suggestions = false
dependency-diagnostics = true
",
            )],
            |cfg| {
                assert!(
                    !cfg.uv_stub_suggestions,
                    "stub-suggestions should be parsed as false"
                );
                assert!(
                    cfg.uv_dependency_diagnostics,
                    "dependency-diagnostics should be parsed as true"
                );
            },
        );
    }

    #[test]
    fn toml_per_path_overrides_with_rules() {
        with_temp_cfg_dir(
            "bsk_cfg_toml_path_rules_xm",
            &[(
                "pyproject.toml",
                r#"
[tool.basilisk.per-path-overrides."tests/**"]
disabled = ["BSK-E0001"]

[tool.basilisk.per-path-overrides."tests/**".rules]
"imports_unresolved" = "warning"
"BSK-E0005" = "info"
"#,
            )],
            |cfg| {
                assert!(
                    cfg.per_path_overrides.contains_key("tests/**"),
                    "per-path-overrides should contain tests/** key"
                );
                let tests_override = cfg.per_path_overrides.get("tests/**").unwrap();
                assert_eq!(
                    tests_override.disabled_rules,
                    vec!["BSK-E0001"],
                    "disabled rules should be parsed"
                );
                assert_eq!(
                    tests_override
                        .rule_overrides
                        .get("imports_unresolved")
                        .copied(),
                    Some(RuleSeverity::Warning),
                    "rule overrides should contain imports_unresolved as warning"
                );
                assert_eq!(
                    tests_override.rule_overrides.get("BSK-E0005").copied(),
                    Some(RuleSeverity::Info),
                    "rule overrides should contain BSK-E0005 as info"
                );
            },
        );
    }

    #[test]
    fn rule_severity_returns_configured_override() {
        let cfg = BasiliskConfig {
            rules: [
                ("imports_unresolved".to_owned(), RuleSeverity::Warning),
                ("BSK-E0001".to_owned(), RuleSeverity::Disabled),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };

        assert_eq!(
            cfg.rule_severity("imports_unresolved"),
            Some(RuleSeverity::Warning)
        );
        assert_eq!(cfg.rule_severity("BSK-E0001"), Some(RuleSeverity::Disabled));
        assert_eq!(
            cfg.rule_severity("BSK-E9999"),
            None,
            "unconfigured rule should return None"
        );
    }

    #[test]
    fn is_rule_disabled_for_path_uses_overrides() {
        let cfg = BasiliskConfig {
            per_path_overrides: [(
                "vendor/**".to_owned(),
                PathOverride {
                    disabled_rules: vec!["imports_unresolved".to_owned(), "BSK-E0001".to_owned()],
                    rule_overrides: std::collections::HashMap::new(),
                },
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        };

        assert!(
            cfg.is_rule_disabled_for_path(
                "imports_unresolved",
                std::path::Path::new("vendor/lib/foo.py")
            ),
            "imports_unresolved should be disabled for vendor paths"
        );
        assert!(
            cfg.is_rule_disabled_for_path("BSK-E0001", std::path::Path::new("vendor/bar.py")),
            "BSK-E0001 should be disabled for vendor paths"
        );
        assert!(
            !cfg.is_rule_disabled_for_path(
                "imports_unresolved",
                std::path::Path::new("src/app.py")
            ),
            "imports_unresolved should NOT be disabled for non-vendor paths"
        );
        assert!(
            !cfg.is_rule_disabled_for_path("BSK-E9999", std::path::Path::new("vendor/foo.py")),
            "non-listed rule should NOT be disabled even for vendor paths"
        );
    }

    #[test]
    fn json_kebab_case_stub_paths() {
        with_temp_cfg_dir(
            "bsk_cfg_json_kebab_stubs_xm",
            &[(
                "basilisk.json",
                r#"{
                "stub-paths": ["typings/", "custom-stubs/"]
            }"#,
            )],
            |cfg| {
                assert_eq!(
                    cfg.stub_paths.len(),
                    2,
                    "stub-paths kebab key should be accepted"
                );
                assert_eq!(
                    cfg.stub_paths.first().and_then(|p| p.to_str()),
                    Some("typings/")
                );
                assert_eq!(
                    cfg.stub_paths.get(1).and_then(|p| p.to_str()),
                    Some("custom-stubs/")
                );
            },
        );
    }

    #[test]
    fn json_kebab_case_per_module_overrides() {
        with_temp_cfg_dir(
            "bsk_cfg_json_kebab_pmo_xm",
            &[(
                "basilisk.json",
                r#"{
                "per-module-overrides": {
                    "numpy": { "ignore-missing-stubs": true }
                }
            }"#,
            )],
            |cfg| {
                assert!(
                    cfg.per_module_overrides.contains_key("numpy"),
                    "per-module-overrides kebab key should be accepted"
                );
                assert!(
                    cfg.per_module_overrides
                        .get("numpy")
                        .unwrap()
                        .ignore_missing_stubs,
                    "ignore-missing-stubs kebab key should be accepted"
                );
            },
        );
    }

    #[test]
    fn fallback_to_pyproject_when_json_is_invalid() {
        with_temp_cfg_dir(
            "bsk_cfg_invalid_json_xm",
            &[
                // Write invalid JSON so load_from_json returns None.
                ("basilisk.json", "{ not valid json !!!"),
                (
                    "pyproject.toml",
                    r#"
[tool.basilisk]
stub-paths = ["fallback-stubs/"]
"#,
                ),
            ],
            |cfg| {
                assert_eq!(
                    cfg.stub_paths.len(),
                    1,
                    "should fall back to pyproject.toml when JSON is invalid"
                );
                assert_eq!(
                    cfg.stub_paths.first().unwrap().to_str(),
                    Some("fallback-stubs/")
                );
            },
        );
    }

    #[test]
    fn uv_defaults_when_not_configured() {
        with_temp_cfg_dir(
            "bsk_cfg_uv_defaults_xm",
            &[("basilisk.json", r#"{ "stubPaths": [] }"#)],
            |cfg| {
                assert!(
                    !cfg.uv_stub_suggestions,
                    "uv_stub_suggestions defaults to false so BSK-E0152 stays off by default (default config = the PEP set only)"
                );
                assert!(
                    !cfg.uv_dependency_diagnostics,
                    "uv_dependency_diagnostics should default to false"
                );
            },
        );
    }
}
