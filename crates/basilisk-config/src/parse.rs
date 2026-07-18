//! Implements [CHKARCH-CONFIG-MODEL] and [CHKARCH-CONFIG-FILE]. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL
//!
//! Configuration file parsing — `pyproject.toml` `[tool.basilisk]`.
//! A configuration is two flat maps and nothing else: `[tool.basilisk.rules]`
//! (per-rule entries) and `[tool.basilisk.rule-tags]` (group entries).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::severity::RuleSeverity;

/// One folder's `[tool.basilisk]` rule tables ([CHKARCH-CONFIG-MODEL]).
///
/// The design source is `models/configuration.td` (`RulesConfig`): explicit
/// per-rule entries plus explicit tag entries. A missing table and an empty
/// table behave identically — the table decides nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct RuleTables {
    /// `[tool.basilisk.rules]` — `"<code>" = "<severity>"`.
    pub rules: HashMap<String, RuleSeverity>,
    /// `[tool.basilisk.rule-tags]` — `"<tag>" = "<severity>"`. One written
    /// line that grades every rule carrying the tag; never an implicit switch.
    pub rule_tags: HashMap<String, RuleSeverity>,
}

impl RuleTables {
    /// Whether this table decides `code`, and with what severity.
    ///
    /// Within one table a per-rule entry beats tag entries; among matching
    /// tag entries the strictest severity wins ([CHKARCH-CONFIG-MODEL]).
    #[must_use]
    pub fn decide(&self, code: &str, tags: &[&str]) -> Option<RuleSeverity> {
        if let Some(severity) = self.rules.get(code) {
            return Some(*severity);
        }
        tags.iter()
            .filter_map(|tag| self.rule_tags.get(*tag))
            .copied()
            .max_by_key(|severity| severity.strictness())
    }

    /// Whether the table carries no entries at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty() && self.rule_tags.is_empty()
    }
}

/// Basilisk project configuration parsed from `pyproject.toml`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BasiliskConfig {
    /// Runtime owning root discovered by loaders; never a persisted key.
    #[serde(skip)]
    pub project_root: Option<PathBuf>,

    /// Directory names to exclude from file discovery.
    ///
    /// Defaults to [`crate::DEFAULT_EXCLUDES`]. Setting this in config
    /// replaces the defaults — add them back explicitly if still needed.
    /// Hidden directories (starting with `.`) are always excluded.
    pub exclude: Vec<String>,

    /// Roots scanned when no paths are given on the CLI ([CHKARCH-CONFIG-INCLUDE]).
    pub include: Vec<String>,

    /// Additional directories to search for `.pyi` stubs.
    pub stub_paths: Vec<PathBuf>,

    /// Custom typeshed directory whose `stdlib/` subtree overrides the bundled
    /// standard-library stubs ([STUBRES-CUSTOM-TYPESHED]).
    pub typeshed_path: Option<PathBuf>,

    /// Nearest-first chain of `[tool.basilisk]` rule tables on the ancestor
    /// walk ([CHKARCH-CONFIG-DISCOVERY]). Rules are never merged: resolution
    /// walks this chain and the nearest table that decides a rule wins
    /// outright ([`Self::resolve_severity`]).
    pub rule_chain: Vec<RuleTables>,

    /// Auto-stub generation mode: `"runtime"`, `"ast"`, `"hybrid"`, or `"disabled"`.
    pub auto_stub_mode: String,

    /// Directory for auto-generated stubs.
    pub auto_stub_path: PathBuf,

    /// Target Python version for version-aware rules ([CHKARCH-VERSION-TARGET]).
    pub python_version: Option<String>,

    /// Target platform for platform-aware rules ([CHKARCH-VERSION-TARGET]).
    pub python_platform: Option<String>,

    /// Whether attribute narrowing (`x.attr` guards) survives intervening
    /// calls — the explicit soundness tradeoff of
    /// [TYPEINF-NARROWING-ATTR-CALLS]. `None` means the default `true`: the
    /// USABLE behavior (a call *could* invalidate the attribute, but
    /// treating every call as an invalidation makes attribute narrowing
    /// useless in practice — Pyrefly's documented lesson). Projects that
    /// prefer the sound-but-strict behavior set
    /// `narrow-attributes-across-calls = false` in `[tool.basilisk]`.
    pub narrow_attributes_across_calls: Option<bool>,
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
            rule_chain: Vec::new(),
            auto_stub_mode: "hybrid".to_owned(),
            auto_stub_path: PathBuf::from(".basilisk/stubs"),
            python_version: None,
            python_platform: None,
            narrow_attributes_across_calls: None,
        }
    }
}

impl BasiliskConfig {
    /// A config whose single nearest table holds these per-rule entries.
    ///
    /// Convenience for callers and tests that need one folder's
    /// `[tool.basilisk.rules]` table without parsing TOML.
    #[must_use]
    pub fn with_rule_entries(rules: HashMap<String, RuleSeverity>) -> Self {
        Self {
            rule_chain: vec![RuleTables {
                rules,
                rule_tags: HashMap::new(),
            }],
            ..Default::default()
        }
    }

    /// Resolve the configured severity for `code` carrying `tags`.
    ///
    /// Implements [CHKARCH-CONFIG-MODEL] resolution: one walk, first decision
    /// wins. The nearest table that decides the rule — per-rule entry first,
    /// then strictest matching tag entry — wins outright. `None` means no
    /// table decides the rule: `pep` rules then run at `error` and every
    /// other rule is disabled (the caller owns that scope default, because
    /// provenance lives in the checker's rule registry).
    #[must_use]
    pub fn resolve_severity(&self, code: &str, tags: &[&str]) -> Option<RuleSeverity> {
        self.rule_chain
            .iter()
            .find_map(|table| table.decide(code, tags))
    }

    /// Whether any `[tool.basilisk]` table exists on the discovered chain.
    ///
    /// A missing table and an empty table behave identically for checking;
    /// the only consumer of this distinction is the LSP's one-time seed
    /// ([LSPARCH-CONFIG-SEEDING]).
    #[must_use]
    pub fn has_config_table(&self) -> bool {
        !self.rule_chain.is_empty()
    }

    /// The nearest folder's rule tables, when any table exists on the chain.
    #[must_use]
    pub fn nearest_tables(&self) -> Option<&RuleTables> {
        self.rule_chain.first()
    }

    /// Merge `child` over `self`, where `child` is nearer to the checked file.
    ///
    /// Implements [CHKARCH-CONFIG-DISCOVERY]. Rules are never merged: the
    /// child's tables go in front of the ancestor's on [`Self::rule_chain`]
    /// so the nearest deciding table wins. Non-rule fields merge additively,
    /// nearest directory winning per key.
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
        let mut chain = child.rule_chain;
        chain.append(&mut self.rule_chain);
        self.rule_chain = chain;
        // The nearest config's directory anchors root-relative interpretation
        // (`include`/`exclude` globs).
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
        self.narrow_attributes_across_calls = child
            .narrow_attributes_across_calls
            .or(self.narrow_attributes_across_calls);
        self
    }
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

/// Load configuration from `pyproject.toml` `[tool.basilisk]`.
///
/// Implements [CHKARCH-CONFIG-FILE]: parses `python-version`/`python-platform`,
/// `stub-paths`, `include`/`exclude`, `rules`, and `rule-tags`. Returns `None`
/// when the file has no `[tool.basilisk]` table — such a file contributes
/// nothing to the walk.
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

    if let Some(arr) = basilisk.get("stub-paths").and_then(|v| v.as_array()) {
        cfg.stub_paths = arr
            .iter()
            .filter_map(|v| v.as_str().map(PathBuf::from))
            .collect();
    }

    if let Some(val) = basilisk.get("typeshed-path").and_then(|v| v.as_str()) {
        cfg.typeshed_path = Some(PathBuf::from(val));
    }

    // [CHKARCH-CONFIG-MODEL]: this file's one rule table. An empty or absent
    // pair of maps still contributes a (deciding-nothing) table to the chain —
    // the table's existence is what the LSP seed checks.
    let mut tables = RuleTables::default();
    if let Some(rules_table) = basilisk.get("rules").and_then(|v| v.as_table()) {
        parse_severity_map(rules_table, &mut tables.rules);
    }
    if let Some(tags_table) = basilisk.get("rule-tags").and_then(|v| v.as_table()) {
        parse_severity_map(tags_table, &mut tables.rule_tags);
    }
    cfg.rule_chain = vec![tables];

    if let Some(val) = basilisk.get("auto-stub-mode").and_then(|v| v.as_str()) {
        val.clone_into(&mut cfg.auto_stub_mode);
    }

    if let Some(val) = basilisk.get("auto-stub-path").and_then(|v| v.as_str()) {
        cfg.auto_stub_path = PathBuf::from(val);
    }

    // python-version / python-platform [CHKARCH-VERSION-TARGET]
    if let Some(val) = basilisk.get("python-version").and_then(|v| v.as_str()) {
        cfg.python_version = Some(val.to_owned());
    }
    if let Some(val) = basilisk.get("python-platform").and_then(|v| v.as_str()) {
        cfg.python_platform = Some(val.to_owned());
    }

    // [TYPEINF-NARROWING-ATTR-CALLS]: the attribute-narrowing soundness knob.
    if let Some(val) = basilisk
        .get("narrow-attributes-across-calls")
        .and_then(toml::Value::as_bool)
    {
        cfg.narrow_attributes_across_calls = Some(val);
    }

    Some(cfg)
}

/// Parse a `"<key>" = "<severity>"` TOML table into `target`.
fn parse_severity_map(table: &toml::Table, target: &mut HashMap<String, RuleSeverity>) {
    for (key, severity_val) in table {
        if let Some(severity) = severity_val.as_str().and_then(RuleSeverity::parse) {
            let _ = target.insert(key.clone(), severity);
        }
    }
}
