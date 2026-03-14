//! Configuration parsing for Basilisk.
//!
//! Parses `pyproject.toml` `[tool.basilisk]` and `basilisk.json` with support for:
//! - Global rule severity overrides (`rules."BSK-E0010" = "warning"`)
//! - Per-module overrides (`per-module-overrides."fastmcp".ignore-missing-stubs = true`)
//! - Per-path overrides (`per-path-overrides."vendor/**".rules.disabled = [...]`)
//! - Stub path directories (`stub-paths = ["stubs/"]`)

mod overrides;
mod parse;

pub use overrides::{ModuleOverride, PathOverride, RuleSeverity};
pub use parse::BasiliskConfig;

use std::path::Path;

/// Load a `BasiliskConfig` from the first config file found in `root`.
///
/// Search order (highest priority wins):
/// 1. `basilisk.json`
/// 2. `pyproject.toml` `[tool.basilisk]`
///
/// Returns `BasiliskConfig::default()` if no config file is found.
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

    #[test]
    fn load_default_when_no_config() {
        let dir = std::env::temp_dir().join("bsk_cfg_empty_xm");
        fs::create_dir_all(&dir).unwrap();
        let cfg = load_basilisk_config(&dir);
        assert!(cfg.stub_paths.is_empty());
        assert!(cfg.rules.is_empty());
        assert!(cfg.per_module_overrides.is_empty());
        assert!(cfg.per_path_overrides.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_from_pyproject_toml() {
        let dir = std::env::temp_dir().join("bsk_cfg_pyproject_xm");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("pyproject.toml"),
            r#"
[tool.basilisk]
stub-paths = ["stubs/", "typings/"]

[tool.basilisk.rules]
"BSK-E0010" = "warning"
"BSK-E0001" = "disabled"

[tool.basilisk.per-module-overrides.fastmcp]
ignore-missing-stubs = true

[tool.basilisk.per-module-overrides."django.*"]
ignore-missing-stubs = true

[tool.basilisk.per-path-overrides."vendor/**"]
disabled = ["BSK-E0010", "BSK-E0001"]
"#,
        )
        .unwrap();

        let cfg = load_basilisk_config(&dir);
        assert_eq!(cfg.stub_paths.len(), 2);
        assert_eq!(cfg.rules.len(), 2);
        assert_eq!(
            cfg.rules.get("BSK-E0010").copied(),
            Some(RuleSeverity::Warning)
        );
        assert_eq!(
            cfg.rules.get("BSK-E0001").copied(),
            Some(RuleSeverity::Disabled)
        );
        assert!(cfg.per_module_overrides.contains_key("fastmcp"));
        assert!(cfg.per_module_overrides.contains_key("django.*"));
        assert!(cfg.per_path_overrides.contains_key("vendor/**"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_from_basilisk_json() {
        let dir = std::env::temp_dir().join("bsk_cfg_json_xm");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("basilisk.json"),
            r#"{
                "stubPaths": ["stubs/"],
                "rules": {
                    "BSK-E0010": "info"
                },
                "perModuleOverrides": {
                    "requests": { "ignoreMissingStubs": true }
                }
            }"#,
        )
        .unwrap();

        let cfg = load_basilisk_config(&dir);
        assert_eq!(cfg.stub_paths.len(), 1);
        assert_eq!(
            cfg.rules.get("BSK-E0010").copied(),
            Some(RuleSeverity::Info)
        );
        assert!(cfg.per_module_overrides.contains_key("requests"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn basilisk_json_takes_priority() {
        let dir = std::env::temp_dir().join("bsk_cfg_priority_xm");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("basilisk.json"),
            r#"{ "stubPaths": ["from_json/"] }"#,
        )
        .unwrap();
        fs::write(
            dir.join("pyproject.toml"),
            "[tool.basilisk]\nstub-paths = [\"from_toml/\"]\n",
        )
        .unwrap();

        let cfg = load_basilisk_config(&dir);
        assert_eq!(cfg.stub_paths.len(), 1);
        assert_eq!(cfg.stub_paths.first().unwrap().to_str(), Some("from_json/"));

        let _ = fs::remove_dir_all(&dir);
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
}
