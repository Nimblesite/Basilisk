//! Implements [STUBRES-CONFIG]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CONFIG
//! Configuration parsing for Basilisk.
//!
//! Parses `pyproject.toml` `[tool.basilisk]` with support for:
//! - Global rule severity overrides (`rules."imports_unresolved" = "warning"`)
//! - Per-module overrides (`per-module-overrides."fastmcp".ignore-missing-stubs = true`)
//! - Per-path overrides (`per-path-overrides."vendor/**".rules.disabled = [...]`)
//! - Stub path directories (`stub-paths = ["stubs/"]`)

pub mod editor;
pub mod overrides;
mod parse;

pub use editor::{
    active_config_path, adoption_rule_overrides, apply_config_patch, build_rule_patch,
    discover_config_document, discover_config_document_with_content, ConfigDocument,
    ConfigDocumentError, ConfigFormat, ConfigPatch, RuleConfigScope, RuleConfigUpdate,
};
pub use overrides::{path_matches_pattern, ModuleOverride, PathOverride, RuleSeverity};
pub use parse::BasiliskConfig;

use std::path::Path;

/// Directories excluded from analysis by default.
///
/// Users can override this via the `exclude` key in
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

/// Load a `BasiliskConfig` for `start` by discovering config files up the
/// ancestor directory chain and merging them cumulatively.
///
/// Implements [CHKARCH-CONFIG-DISCOVERY] (GitHub #311): every surface —
/// `basilisk check`/`fix`/`adopt` and the LSP — resolves rule config through
/// this one routine, so the result is independent of argument order, path
/// spelling, and cwd. The walk visits `start` and every ancestor up to the
/// filesystem root; each directory contributes its config file (see
/// [`load_dir_config`] for the per-directory priority), and directories
/// nearer to `start` win per key over ancestors (see
/// [`BasiliskConfig::merged_with`]). A `pyproject.toml` without a
/// `[tool.basilisk]` table contributes nothing and does not stop the walk.
///
/// Returns `BasiliskConfig::default()` if no config file is found anywhere
/// on the chain (and likewise on malformed files — no configuration-error
/// exit is raised; see report).
/// See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-DISCOVERY
#[must_use]
pub fn load_basilisk_config(start: &Path) -> BasiliskConfig {
    let chain: Vec<BasiliskConfig> = absolute_start(start)
        .ancestors()
        .filter_map(load_dir_config)
        .collect();
    // `ancestors()` yields nearest-first; fold from the outermost ancestor so
    // configs nearer to `start` override it per key.
    let mut config = chain
        .into_iter()
        .rev()
        .fold(BasiliskConfig::default(), BasiliskConfig::merged_with);
    // The nearest config-holding directory anchors root-relative
    // interpretation (per-path overrides, adoption store); with no config
    // anywhere on the chain, `start` itself anchors, as before.
    if config.project_root.is_none() {
        config.project_root = Some(start.to_path_buf());
    }
    config
}

/// The nearest directory at or above `start` holding a recognized config file
/// (`pyproject.toml` with a `[tool.basilisk]` table).
///
/// This is the anchor directory for artifacts that live next to the config —
/// e.g. the adoption store — so `basilisk adopt` writes where `basilisk check`
/// discovers. Implements [CHKARCH-CONFIG-DISCOVERY].
#[must_use]
pub fn discover_config_dir(start: &Path) -> Option<std::path::PathBuf> {
    absolute_start(start)
        .ancestors()
        .find(|dir| load_dir_config(dir).is_some())
        .map(Path::to_path_buf)
}

/// Load the config from exactly one directory — no ancestor walk.
///
/// Implements [CHKARCH-CONFIG-FILE]: the only configuration source is the
/// `[tool.basilisk]` table of the directory's `pyproject.toml`. A
/// `pyproject.toml` without that table contributes nothing.
///
/// Returns `None` when the directory holds no parseable config.
fn load_dir_config(dir: &Path) -> Option<BasiliskConfig> {
    let pyproject = dir.join("pyproject.toml");
    if pyproject.is_file() {
        if let Some(mut cfg) = parse::load_from_pyproject(&pyproject) {
            cfg.project_root = Some(dir.to_path_buf());
            return Some(cfg);
        }
    }

    None
}

/// Absolutize `start` (against cwd) so `ancestors()` walks the full directory
/// chain even for relative paths like `.` or `child/` — WITHOUT
/// canonicalizing, so discovered directories keep the caller's path spelling
/// (a symlinked temp dir must not come back re-rooted, or callers that
/// `strip_prefix` against their own paths break).
fn absolute_start(start: &Path) -> std::path::PathBuf {
    if start.is_absolute() {
        start.to_path_buf()
    } else {
        std::env::current_dir().map_or_else(|_| start.to_path_buf(), |cwd| cwd.join(start))
    }
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
                assert!(
                    cfg.stub_paths.is_empty(),
                    "basilisk.json stubPaths must be ignored"
                );
                assert!(
                    cfg.typeshed_path.is_none(),
                    "basilisk.json typeshedPath must be ignored"
                );
                assert!(cfg.rules.is_empty(), "basilisk.json rules must be ignored");
                assert!(
                    cfg.per_module_overrides.is_empty(),
                    "basilisk.json perModuleOverrides must be ignored"
                );
            },
        );
    }

    #[test]
    fn pyproject_wins_over_stray_basilisk_json() {
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
                assert_eq!(cfg.stub_paths.first().unwrap().to_str(), Some("from_toml/"));
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
    fn json_exclude_does_not_override_defaults() {
        with_temp_cfg_dir(
            "bsk_cfg_json_exclude_xm",
            &[("basilisk.json", r#"{ "exclude": ["vendor", "generated"] }"#)],
            |cfg| {
                // The stray basilisk.json is never read: the defaults survive
                // and none of its entries load.
                assert!(
                    cfg.exclude.iter().any(|e| e == "__pycache__"),
                    "default excludes must survive a stray basilisk.json"
                );
                assert!(
                    !cfg.exclude.iter().any(|e| e == "vendor"),
                    "stray basilisk.json exclude entries must not load"
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
    fn json_kebab_case_stub_paths_are_ignored() {
        with_temp_cfg_dir(
            "bsk_cfg_json_kebab_stubs_xm",
            &[(
                "basilisk.json",
                r#"{
                "stub-paths": ["typings/", "custom-stubs/"]
            }"#,
            )],
            |cfg| {
                assert!(
                    cfg.stub_paths.is_empty(),
                    "stray basilisk.json stub-paths must be ignored"
                );
            },
        );
    }

    #[test]
    fn json_kebab_case_per_module_overrides_are_ignored() {
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
                    cfg.per_module_overrides.is_empty(),
                    "stray basilisk.json per-module-overrides must be ignored"
                );
            },
        );
    }

    #[test]
    fn malformed_stray_json_never_blocks_the_pyproject_config() {
        with_temp_cfg_dir(
            "bsk_cfg_invalid_json_xm",
            &[
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
                // basilisk.json is never read — malformed or not — so the
                // pyproject.toml config always loads.
                assert_eq!(
                    cfg.stub_paths.first().and_then(|p| p.to_str()),
                    Some("fallback-stubs/"),
                    "a stray basilisk.json must never block pyproject.toml"
                );
                assert_eq!(cfg.stub_paths.len(), 1);
            },
        );
    }

    /// GitHub #311: rule config must be discovered by walking ancestor
    /// directories, not just the exact directory passed in — otherwise
    /// `basilisk check path/to/file.py` silently ignores the project root
    /// config. See [CHKARCH-CONFIG-FILE].
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
            cfg.rules.get("imports_unresolved").copied(),
            Some(RuleSeverity::Warning),
            "loading config from a child directory must discover the ancestor's \
             pyproject.toml [tool.basilisk] (GitHub #311)"
        );
    }

    /// GitHub #311: config is cumulative/additive — a child directory's config
    /// appends to (and per-key overrides) ancestor config; it must never blow
    /// the ancestor config away.
    #[test]
    fn child_config_merges_cumulatively_over_ancestor() {
        let root = std::env::temp_dir().join(format!("bsk_cfg_cumulative_{}", std::process::id()));
        let child = root.join("child");
        fs::create_dir_all(&child).unwrap();
        fs::write(
            root.join("pyproject.toml"),
            "[tool.basilisk.rules]\n\
             \"imports_unresolved\" = \"warning\"\n\
             \"BSK-E0001\" = \"warning\"\n",
        )
        .unwrap();
        fs::write(
            child.join("pyproject.toml"),
            "[tool.basilisk.rules]\n\"BSK-E0001\" = \"disabled\"\n",
        )
        .unwrap();

        let cfg = load_basilisk_config(&child);
        let _ = fs::remove_dir_all(&root);

        assert_eq!(
            cfg.rules.get("imports_unresolved").copied(),
            Some(RuleSeverity::Warning),
            "ancestor rules not mentioned by the child config must survive the merge"
        );
        assert_eq!(
            cfg.rules.get("BSK-E0001").copied(),
            Some(RuleSeverity::Disabled),
            "the child config must win where it overlaps the ancestor config"
        );
    }
}
