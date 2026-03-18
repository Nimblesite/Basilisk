//! Configuration file parsing — `pyproject.toml` and `basilisk.json`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::overrides::{ModuleOverride, PathOverride, RuleSeverity};

/// Basilisk project configuration parsed from config files.
///
/// This is the rich configuration model with per-module and per-path overrides.
/// It supplements the `WorkspaceConfig` in `basilisk-lsp` which handles
/// analysis mode, python version, and other LSP-level settings.
#[derive(Debug, Clone)]
pub struct BasiliskConfig {
    /// Directory names to exclude from file discovery.
    ///
    /// Defaults to [`DEFAULT_EXCLUDES`]. Setting this in config replaces
    /// the defaults — add them back explicitly if still needed.
    ///
    /// Hidden directories (starting with `.`) are always excluded
    /// regardless of this list.
    pub exclude: Vec<String>,

    /// Additional directories to search for `.pyi` stubs.
    pub stub_paths: Vec<PathBuf>,

    /// Global rule severity overrides.
    ///
    /// Maps rule codes (e.g. `"BSK-E0010"`) to severity levels.
    pub rules: HashMap<String, RuleSeverity>,

    /// Per-module overrides keyed by module name or pattern.
    ///
    /// Patterns support `.*` suffix for wildcard matching
    /// (e.g. `"django.*"` matches `django.db.models`).
    pub per_module_overrides: HashMap<String, ModuleOverride>,

    /// Per-path overrides keyed by path glob pattern.
    ///
    /// Patterns support `**` for recursive matching
    /// (e.g. `"vendor/**"` matches `vendor/lib/foo.py`).
    pub per_path_overrides: HashMap<String, PathOverride>,
}

impl Default for BasiliskConfig {
    fn default() -> Self {
        Self {
            exclude: crate::DEFAULT_EXCLUDES
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
            stub_paths: Vec::new(),
            rules: HashMap::new(),
            per_module_overrides: HashMap::new(),
            per_path_overrides: HashMap::new(),
        }
    }
}

impl BasiliskConfig {
    /// Check whether BSK-E0010 should be suppressed for a given module.
    #[must_use]
    pub fn should_ignore_missing_stubs(&self, module_name: &str) -> bool {
        crate::overrides::find_module_override(module_name, &self.per_module_overrides)
            .is_some_and(|o| o.ignore_missing_stubs)
    }

    /// Get the configured severity for a rule, if overridden.
    #[must_use]
    pub fn rule_severity(&self, code: &str) -> Option<RuleSeverity> {
        self.rules.get(code).copied()
    }

    /// Check whether a rule is disabled for a given file path.
    #[must_use]
    pub fn is_rule_disabled_for_path(&self, rule_code: &str, file_path: &Path) -> bool {
        crate::overrides::is_rule_disabled_for_path(rule_code, file_path, &self.per_path_overrides)
    }
}

/// Load configuration from `basilisk.json`.
pub fn load_from_json(path: &Path) -> Option<BasiliskConfig> {
    let content = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let obj = json.as_object()?;

    let mut cfg = BasiliskConfig::default();

    // exclude
    if let Some(arr) = obj.get("exclude").and_then(|v| v.as_array()) {
        cfg.exclude = arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
    }

    // stub-paths / stubPaths
    if let Some(arr) = obj
        .get("stubPaths")
        .or_else(|| obj.get("stub-paths"))
        .and_then(|v| v.as_array())
    {
        cfg.stub_paths = arr
            .iter()
            .filter_map(|v| v.as_str().map(PathBuf::from))
            .collect();
    }

    // rules
    if let Some(rules_obj) = obj.get("rules").and_then(|v| v.as_object()) {
        for (code, severity_val) in rules_obj {
            if let Some(severity_str) = severity_val.as_str() {
                if let Some(severity) = RuleSeverity::parse(severity_str) {
                    let _ = cfg.rules.insert(code.clone(), severity);
                }
            }
        }
    }

    // perModuleOverrides
    if let Some(overrides_obj) = obj
        .get("perModuleOverrides")
        .or_else(|| obj.get("per-module-overrides"))
        .and_then(|v| v.as_object())
    {
        for (pattern, override_val) in overrides_obj {
            if let Some(override_obj) = override_val.as_object() {
                let ignore = override_obj
                    .get("ignoreMissingStubs")
                    .or_else(|| override_obj.get("ignore-missing-stubs"))
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                let _ = cfg.per_module_overrides.insert(
                    pattern.clone(),
                    ModuleOverride {
                        ignore_missing_stubs: ignore,
                    },
                );
            }
        }
    }

    Some(cfg)
}

/// Load configuration from `pyproject.toml` `[tool.basilisk]` section.
pub fn load_from_pyproject(path: &Path) -> Option<BasiliskConfig> {
    let content = std::fs::read_to_string(path).ok()?;
    let table: toml::Table = content.parse().ok()?;

    let tool = table.get("tool")?.as_table()?;
    let basilisk = tool.get("basilisk")?.as_table()?;

    let mut cfg = BasiliskConfig::default();

    // exclude
    if let Some(arr) = basilisk.get("exclude").and_then(|v| v.as_array()) {
        cfg.exclude = arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
    }

    // stub-paths
    if let Some(arr) = basilisk.get("stub-paths").and_then(|v| v.as_array()) {
        cfg.stub_paths = arr
            .iter()
            .filter_map(|v| v.as_str().map(PathBuf::from))
            .collect();
    }

    // rules
    if let Some(rules_table) = basilisk.get("rules").and_then(|v| v.as_table()) {
        for (code, severity_val) in rules_table {
            if let Some(severity_str) = severity_val.as_str() {
                if let Some(severity) = RuleSeverity::parse(severity_str) {
                    let _ = cfg.rules.insert(code.clone(), severity);
                }
            }
        }
    }

    // per-module-overrides
    if let Some(overrides_table) = basilisk
        .get("per-module-overrides")
        .and_then(|v| v.as_table())
    {
        for (pattern, override_val) in overrides_table {
            if let Some(override_table) = override_val.as_table() {
                let ignore = override_table
                    .get("ignore-missing-stubs")
                    .and_then(toml::Value::as_bool)
                    .unwrap_or(false);
                let _ = cfg.per_module_overrides.insert(
                    pattern.clone(),
                    ModuleOverride {
                        ignore_missing_stubs: ignore,
                    },
                );
            }
        }
    }

    // per-path-overrides
    if let Some(path_overrides_table) = basilisk
        .get("per-path-overrides")
        .and_then(|v| v.as_table())
    {
        for (pattern, override_val) in path_overrides_table {
            if let Some(override_table) = override_val.as_table() {
                let disabled_rules = override_table
                    .get("disabled")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                let mut rule_overrides = HashMap::new();
                if let Some(rules_table) = override_table.get("rules").and_then(|v| v.as_table()) {
                    for (code, severity_val) in rules_table {
                        if let Some(severity_str) = severity_val.as_str() {
                            if let Some(severity) = RuleSeverity::parse(severity_str) {
                                let _ = rule_overrides.insert(code.clone(), severity);
                            }
                        }
                    }
                }

                let _ = cfg.per_path_overrides.insert(
                    pattern.clone(),
                    PathOverride {
                        disabled_rules,
                        rule_overrides,
                    },
                );
            }
        }
    }

    Some(cfg)
}
