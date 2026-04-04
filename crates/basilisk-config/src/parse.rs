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

    /// Whether to emit BSK-W0010 (missing type stubs) warnings.
    ///
    /// When `true` (default), warns about installed packages lacking type stubs.
    /// Maps to `basilisk.uv.stubSuggestions` in the LSP config.
    pub uv_stub_suggestions: bool,

    /// Whether to emit dependency hygiene diagnostics (BSK-W0011, W0012, W0013).
    ///
    /// When `true`, warns about undeclared transitive dependencies, unused
    /// declared dependencies, and stale lock files. Disabled by default.
    /// Maps to `basilisk.uv.dependencyDiagnostics` in the LSP config.
    pub uv_dependency_diagnostics: bool,

    /// Auto-stub generation mode: `"runtime"`, `"ast"`, `"hybrid"`, or `"disabled"`.
    ///
    /// Controls how `basilisk stubs generate` creates `.pyi` files for
    /// untyped packages. Defaults to `"hybrid"`.
    pub auto_stub_mode: String,

    /// Directory for auto-generated stubs.
    ///
    /// Generated `.pyi` files are placed here and automatically included
    /// in the stub search path. Defaults to `".basilisk/stubs"`.
    pub auto_stub_path: PathBuf,
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
            uv_stub_suggestions: true,
            uv_dependency_diagnostics: false,
            auto_stub_mode: "hybrid".to_owned(),
            auto_stub_path: PathBuf::from(".basilisk/stubs"),
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

    // uv section
    if let Some(uv_obj) = obj.get("uv").and_then(|v| v.as_object()) {
        if let Some(val) = uv_obj
            .get("stubSuggestions")
            .or_else(|| uv_obj.get("stub-suggestions"))
            .and_then(serde_json::Value::as_bool)
        {
            cfg.uv_stub_suggestions = val;
        }
        if let Some(val) = uv_obj
            .get("dependencyDiagnostics")
            .or_else(|| uv_obj.get("dependency-diagnostics"))
            .and_then(serde_json::Value::as_bool)
        {
            cfg.uv_dependency_diagnostics = val;
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

    // auto-stub-mode / autoStubMode
    if let Some(val) = obj
        .get("autoStubMode")
        .or_else(|| obj.get("auto-stub-mode"))
        .and_then(|v| v.as_str())
    {
        val.clone_into(&mut cfg.auto_stub_mode);
    }

    // auto-stub-path / autoStubPath
    if let Some(val) = obj
        .get("autoStubPath")
        .or_else(|| obj.get("auto-stub-path"))
        .and_then(|v| v.as_str())
    {
        cfg.auto_stub_path = PathBuf::from(val);
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

    // uv section
    if let Some(uv_table) = basilisk.get("uv").and_then(|v| v.as_table()) {
        if let Some(val) = uv_table
            .get("stub-suggestions")
            .and_then(toml::Value::as_bool)
        {
            cfg.uv_stub_suggestions = val;
        }
        if let Some(val) = uv_table
            .get("dependency-diagnostics")
            .and_then(toml::Value::as_bool)
        {
            cfg.uv_dependency_diagnostics = val;
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
    if let Some(table) = basilisk.get("per-path-overrides").and_then(|v| v.as_table()) {
        parse_toml_path_overrides(table, &mut cfg.per_path_overrides);
    }

    // auto-stub-mode
    if let Some(val) = basilisk.get("auto-stub-mode").and_then(|v| v.as_str()) {
        val.clone_into(&mut cfg.auto_stub_mode);
    }

    // auto-stub-path
    if let Some(val) = basilisk.get("auto-stub-path").and_then(|v| v.as_str()) {
        cfg.auto_stub_path = PathBuf::from(val);
    }

    Some(cfg)
}

/// Parse `[tool.basilisk.per-path-overrides]` into the config map.
fn parse_toml_path_overrides(
    table: &toml::Table,
    overrides: &mut HashMap<String, PathOverride>,
) {
    for (pattern, override_val) in table {
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

            let _ = overrides.insert(
                pattern.clone(),
                PathOverride {
                    disabled_rules,
                    rule_overrides,
                },
            );
        }
    }
}
