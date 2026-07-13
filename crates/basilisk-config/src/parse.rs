//! Implements [STUBRES-CONFIG]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CONFIG
//! Configuration file parsing — `pyproject.toml` and `basilisk.json`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::overrides::{ModuleOverride, PathOverride, RuleSeverity};

/// Basilisk project configuration parsed from config files.
///
/// This is the rich configuration model with per-module and per-path overrides.
/// It supplements the `WorkspaceConfig` in `basilisk-lsp` which handles
/// analysis mode, python version, and other LSP-level settings.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BasiliskConfig {
    /// Runtime owning root used to interpret path overrides root-relatively.
    /// This is discovered by loaders and is never a persisted config key.
    #[serde(skip)]
    pub project_root: Option<PathBuf>,

    /// Directory names to exclude from file discovery.
    ///
    /// Defaults to [`DEFAULT_EXCLUDES`]. Setting this in config replaces
    /// the defaults — add them back explicitly if still needed.
    ///
    /// Hidden directories (starting with `.`) are always excluded
    /// regardless of this list.
    pub exclude: Vec<String>,

    /// Roots scanned when no paths are given on the CLI ([CHKARCH-CONFIG-INCLUDE]).
    ///
    /// Empty means "check the current directory". Explicit CLI paths always
    /// override this list; `exclude` applies within the include roots.
    pub include: Vec<String>,

    /// Additional directories to search for `.pyi` stubs.
    pub stub_paths: Vec<PathBuf>,

    /// Custom typeshed directory whose `stdlib/` subtree overrides the bundled
    /// standard-library stubs as the canonical source for stdlib types
    /// (typing-spec import-resolution step 3 —
    /// [STUBRES-CUSTOM-TYPESHED](../../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED)).
    /// `None` keeps the bundled typeshed.
    pub typeshed_path: Option<PathBuf>,

    /// Global rule severity overrides.
    ///
    /// Maps rule codes (e.g. `"imports_unresolved"`) to severity levels.
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

    /// Target Python version for version-aware rules, e.g. `"3.9"`.
    ///
    /// `None` means "use the checker's centralized default" (3.12, see
    /// `basilisk_checker::context::DEFAULT_TARGET_VERSION`). The LSP fills
    /// this from its `[LSPUV-PYTHON-VERSION-RESOLUTION-ORDER]` cascade.
    /// Implements [CHKARCH-VERSION-TARGET].
    pub python_version: Option<String>,

    /// Target platform for platform-aware rules: `"linux"`, `"darwin"`,
    /// or `"win32"`. `None` means platform-neutral analysis.
    /// Implements [CHKARCH-VERSION-TARGET].
    pub python_platform: Option<String>,
}

impl Default for BasiliskConfig {
    fn default() -> Self {
        Self {
            project_root: None,
            exclude: crate::DEFAULT_EXCLUDES
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
            include: Vec::new(),
            stub_paths: Vec::new(),
            typeshed_path: None,
            rules: HashMap::new(),
            per_module_overrides: HashMap::new(),
            per_path_overrides: HashMap::new(),
            auto_stub_mode: "hybrid".to_owned(),
            auto_stub_path: PathBuf::from(".basilisk/stubs"),
            python_version: None,
            python_platform: None,
        }
    }
}

impl BasiliskConfig {
    /// Check whether `imports_unresolved` should be suppressed for a given module.
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

    /// Merge `child` over `self` cumulatively — a child directory's config
    /// appends to an ancestor's, never replaces it wholesale.
    ///
    /// Implements [CHKARCH-CONFIG-DISCOVERY] (GitHub #311). Semantics:
    /// - Map fields (`rules`, `per_module_overrides`, `per_path_overrides`)
    ///   union per key, child wins on overlap.
    /// - `stub_paths` appends the child's entries (deduplicated).
    /// - Remaining scalar/list fields keep the ancestor's value unless the
    ///   child explicitly set one (detected as differing from the default).
    ///
    /// Merging a default (empty) config is the identity, in both directions.
    #[must_use]
    pub fn merged_with(mut self, child: Self) -> Self {
        let defaults = Self::default();
        if child.exclude != defaults.exclude {
            self.exclude = child.exclude;
        }
        if !child.include.is_empty() {
            self.include = child.include;
        }
        for stub_path in child.stub_paths {
            if !self.stub_paths.contains(&stub_path) {
                self.stub_paths.push(stub_path);
            }
        }
        self.rules.extend(child.rules);
        self.per_module_overrides.extend(child.per_module_overrides);
        self.per_path_overrides.extend(child.per_path_overrides);
        // The nearest config's directory anchors root-relative interpretation
        // (per-path overrides, adoption store).
        self.project_root = child.project_root.or(self.project_root);
        if child.auto_stub_mode != defaults.auto_stub_mode {
            self.auto_stub_mode = child.auto_stub_mode;
        }
        if child.auto_stub_path != defaults.auto_stub_path {
            self.auto_stub_path = child.auto_stub_path;
        }
        self.typeshed_path = child.typeshed_path.or(self.typeshed_path);
        self.python_version = child.python_version.or(self.python_version);
        self.python_platform = child.python_platform.or(self.python_platform);
        self
    }
}

/// Look up a JSON object field by its `camelCase` key, falling back to the
/// `kebab-case` alias. Config files accept both spellings interchangeably.
fn alias_get<'a>(
    obj: &'a serde_json::Map<String, serde_json::Value>,
    camel: &str,
    kebab: &str,
) -> Option<&'a serde_json::Value> {
    obj.get(camel).or_else(|| obj.get(kebab))
}

/// Collect the string elements of a JSON array field, if present.
fn json_string_array(
    obj: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<Vec<String>> {
    let arr = obj.get(key)?.as_array()?;
    Some(
        arr.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
    )
}

/// Collect the string elements of a TOML array field, if present.
fn toml_string_array(table: &toml::Table, key: &str) -> Option<Vec<String>> {
    let arr = table.get(key)?.as_array()?;
    Some(
        arr.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
    )
}

/// Load configuration from `basilisk.json`.
pub fn load_from_json(path: &Path) -> Option<BasiliskConfig> {
    let content = std::fs::read_to_string(path).ok()?;
    parse_json_content(&content)
}

/// Parse a `basilisk.json` document already held in memory.
pub(crate) fn parse_json_content(content: &str) -> Option<BasiliskConfig> {
    let json: serde_json::Value = serde_json::from_str(content).ok()?;
    let obj = json.as_object()?;

    let mut cfg = BasiliskConfig::default();

    if let Some(exclude) = json_string_array(obj, "exclude") {
        cfg.exclude = exclude;
    }
    // [CHKARCH-CONFIG-INCLUDE]
    if let Some(include) = json_string_array(obj, "include") {
        cfg.include = include;
    }

    // stub-paths / stubPaths
    if let Some(arr) = alias_get(obj, "stubPaths", "stub-paths").and_then(|v| v.as_array()) {
        cfg.stub_paths = arr
            .iter()
            .filter_map(|v| v.as_str().map(PathBuf::from))
            .collect();
    }

    // typeshed-path / typeshedPath
    if let Some(val) = alias_get(obj, "typeshedPath", "typeshed-path").and_then(|v| v.as_str()) {
        cfg.typeshed_path = Some(PathBuf::from(val));
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
    if let Some(overrides_obj) =
        alias_get(obj, "perModuleOverrides", "per-module-overrides").and_then(|v| v.as_object())
    {
        for (pattern, override_val) in overrides_obj {
            if let Some(override_obj) = override_val.as_object() {
                let ignore = alias_get(override_obj, "ignoreMissingStubs", "ignore-missing-stubs")
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

    parse_json_path_overrides(obj, &mut cfg.per_path_overrides);

    // auto-stub-mode / autoStubMode
    if let Some(val) = alias_get(obj, "autoStubMode", "auto-stub-mode").and_then(|v| v.as_str()) {
        val.clone_into(&mut cfg.auto_stub_mode);
    }

    // auto-stub-path / autoStubPath
    if let Some(val) = alias_get(obj, "autoStubPath", "auto-stub-path").and_then(|v| v.as_str()) {
        cfg.auto_stub_path = PathBuf::from(val);
    }

    // pythonVersion / python-version [CHKARCH-VERSION-TARGET]
    if let Some(val) = alias_get(obj, "pythonVersion", "python-version").and_then(|v| v.as_str()) {
        cfg.python_version = Some(val.to_owned());
    }

    // pythonPlatform / python-platform [CHKARCH-VERSION-TARGET]
    if let Some(val) = alias_get(obj, "pythonPlatform", "python-platform").and_then(|v| v.as_str())
    {
        cfg.python_platform = Some(val.to_owned());
    }

    Some(cfg)
}

fn parse_json_path_overrides(
    root: &serde_json::Map<String, serde_json::Value>,
    overrides: &mut HashMap<String, PathOverride>,
) {
    let Some(entries) = alias_get(root, "perPathOverrides", "per-path-overrides")
        .and_then(serde_json::Value::as_object)
    else {
        return;
    };
    for (pattern, value) in entries {
        let Some(entry) = value.as_object() else {
            continue;
        };
        let disabled_rules = entry
            .get("disabled")
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let rule_overrides = entry
            .get("rules")
            .and_then(serde_json::Value::as_object)
            .map(|rules| {
                rules
                    .iter()
                    .filter_map(|(code, value)| {
                        value
                            .as_str()
                            .and_then(RuleSeverity::parse)
                            .map(|severity| (code.clone(), severity))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let _ = overrides.insert(
            pattern.clone(),
            PathOverride {
                disabled_rules,
                rule_overrides,
            },
        );
    }
}

/// Load configuration from `pyproject.toml` `[tool.basilisk]` section.
///
/// Implements [CHKARCH-CONFIG-FILE]: parses the `[tool.basilisk]` table —
/// `python-version`/`python-platform`, `stub-paths`, `include`/`exclude`,
/// `rules`, `per-module-overrides`, and `per-path-overrides`. (The spec's
/// `[tool.basilisk.mojo-safety]` keys are not parsed — those rules are unshipped;
/// see report.) See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-FILE
pub fn load_from_pyproject(path: &Path) -> Option<BasiliskConfig> {
    let content = std::fs::read_to_string(path).ok()?;
    parse_pyproject_content(&content)
}

/// Parse a `pyproject.toml` document already held in memory.
pub(crate) fn parse_pyproject_content(content: &str) -> Option<BasiliskConfig> {
    let table: toml::Table = content.parse().ok()?;

    let tool = table.get("tool")?.as_table()?;
    let basilisk = tool.get("basilisk")?.as_table()?;

    let mut cfg = BasiliskConfig::default();

    if let Some(exclude) = toml_string_array(basilisk, "exclude") {
        cfg.exclude = exclude;
    }
    // [CHKARCH-CONFIG-INCLUDE]
    if let Some(include) = toml_string_array(basilisk, "include") {
        cfg.include = include;
    }

    // stub-paths
    if let Some(arr) = basilisk.get("stub-paths").and_then(|v| v.as_array()) {
        cfg.stub_paths = arr
            .iter()
            .filter_map(|v| v.as_str().map(PathBuf::from))
            .collect();
    }

    // typeshed-path
    if let Some(val) = basilisk.get("typeshed-path").and_then(|v| v.as_str()) {
        cfg.typeshed_path = Some(PathBuf::from(val));
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
    if let Some(table) = basilisk
        .get("per-path-overrides")
        .and_then(|v| v.as_table())
    {
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

    // python-version [CHKARCH-VERSION-TARGET]
    if let Some(val) = basilisk.get("python-version").and_then(|v| v.as_str()) {
        cfg.python_version = Some(val.to_owned());
    }

    // python-platform [CHKARCH-VERSION-TARGET]
    if let Some(val) = basilisk.get("python-platform").and_then(|v| v.as_str()) {
        cfg.python_platform = Some(val.to_owned());
    }

    Some(cfg)
}

/// Parse `[tool.basilisk.per-path-overrides]` into the config map.
fn parse_toml_path_overrides(table: &toml::Table, overrides: &mut HashMap<String, PathOverride>) {
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
