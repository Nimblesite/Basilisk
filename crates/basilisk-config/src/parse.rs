//! Implements [CHKARCH-CONFIG-MODEL] and [CHKARCH-CONFIG-FILE]. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-MODEL
//!
//! Configuration file parsing — `pyproject.toml` `[tool.basilisk]`.
//! A configuration is two flat maps and nothing else: `[tool.basilisk.rules]`
//! (per-rule entries) and `[tool.basilisk.rule-tags]` (group entries).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::severity::RuleSeverity;

/// Path segments of the default persistent-cache directory, relative to the
/// project root ([CHKCACHE-CONFIG], [CHKCACHE-ENTRY]).
pub const DEFAULT_CACHE_DIR: &[&str] = &[".basilisk", "cache", "check"];

/// The `[tool.basilisk]` key that runs the persistent result cache.
///
/// Spelled once, so the parser, the editor's validator, and the editor's
/// writer can never disagree about what the key is called.
pub(crate) const CACHE_KEY: &str = "cache";

/// The `[tool.basilisk]` key that relocates the persistent result cache.
pub(crate) const CACHE_DIR_KEY: &str = "cache-dir";

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

    /// Exact `python/typeshed` commit pin ([STUBRES-TYPESHED-CONFIG]). A full
    /// SHA. Unset means the bundled commit with a `typeshed_source_unpinned` warning. A set
    /// pin fails closed — the checker never downloads and never substitutes
    /// another SHA; a pin not on this machine is `NO SOURCE`.
    pub typeshed_commit: Option<String>,

    /// A `PyPI` typeshed distribution pinned by wheel SHA-256, `name@sha256:<hex>`
    /// ([STUBRES-TYPESHED-PYPI], issue #312). Mutually exclusive with
    /// `typeshed-commit` and `typeshed-path`; a verified pin suppresses the
    /// source-status advisories.
    pub typeshed_package: Option<String>,

    /// The verified content-addressed typeshed store directory
    /// ([STUBRES-TYPESHED-STORE]). Unset uses the OS cache directory.
    pub typeshed_store_path: Option<PathBuf>,

    /// Whether the persistent, cross-session result cache runs
    /// ([CHKCACHE-CONFIG]). `None` means the project states no preference, so
    /// the caller's default applies — off for `basilisk check`, which is why
    /// an unwritten key and `cache = false` are behaviourally identical.
    /// The in-session Salsa memo layer is a different thing entirely: it is
    /// always on and has no key ([CHKARCH-INCREMENTAL-SALSA]).
    pub cache_enabled: Option<bool>,

    /// Directory holding the persistent result cache ([CHKCACHE-CONFIG]).
    /// Relative paths resolve against the project root. `None` uses
    /// [`Self::cache_directory`]'s default, `.basilisk/cache/check`.
    pub cache_dir: Option<PathBuf>,

    /// Nearest-first chain of `[tool.basilisk]` rule tables on the ancestor
    /// walk ([CHKARCH-CONFIG-DISCOVERY]). Rules are never merged: resolution
    /// walks this chain and the nearest table that decides a rule wins
    /// outright ([`Self::resolve_severity`]).
    pub rule_chain: Vec<RuleTables>,

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
            typeshed_commit: None,
            typeshed_package: None,
            typeshed_store_path: None,
            cache_enabled: None,
            cache_dir: None,
            rule_chain: Vec::new(),
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

    /// Whether the persistent result cache runs for this project
    /// ([CHKCACHE-CONFIG]). An unwritten `cache` key is off, so the default
    /// stays exactly what it was before the key existed.
    #[must_use]
    pub fn cache_is_enabled(&self) -> bool {
        self.cache_enabled.unwrap_or(false)
    }

    /// Where the persistent result cache lives for a project rooted at
    /// `project_root` ([CHKCACHE-CONFIG]).
    ///
    /// The configured `cache-dir` wins, resolved against the project root when
    /// relative so a checked-out project keeps one cache wherever it is
    /// invoked from; otherwise [`DEFAULT_CACHE_DIR`] under that root. Every
    /// surface — the CLI that writes entries and the configuration editor that
    /// displays the location — resolves through this one routine, so the
    /// folder the editor shows is the folder the run uses.
    #[must_use]
    pub fn cache_directory(&self, project_root: &Path) -> PathBuf {
        self.cache_dir.as_ref().map_or_else(
            || {
                DEFAULT_CACHE_DIR
                    .iter()
                    .fold(project_root.to_path_buf(), |dir, part| dir.join(part))
            },
            |dir| {
                if dir.is_absolute() {
                    dir.clone()
                } else {
                    project_root.join(dir)
                }
            },
        )
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
        // `typeshed-path`, `typeshed-commit`, and `typeshed-package` are one
        // mutually-exclusive source selection. A nearer directory that
        // chooses any one replaces the inherited choice as a unit; merging
        // the fields independently could manufacture an invalid combination
        // (e.g. path+package) that appeared in no source file.
        let child_selects_source = child.typeshed_path.is_some()
            || child.typeshed_commit.is_some()
            || child.typeshed_package.is_some();
        if child_selects_source {
            self.typeshed_path = child.typeshed_path;
            self.typeshed_commit = child.typeshed_commit;
            self.typeshed_package = child.typeshed_package;
        }
        self.typeshed_store_path = child.typeshed_store_path.or(self.typeshed_store_path);
        self.cache_enabled = child.cache_enabled.or(self.cache_enabled);
        self.cache_dir = child.cache_dir.or(self.cache_dir);
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

    // [STUBRES-TYPESHED-CONFIG]: `typeshed-commit` pins an exact SHA (unset
    // => bundled commit + typeshed_source_unpinned); `typeshed-store-path` relocates the
    // verified store. That is the whole runtime surface.
    if let Some(val) = basilisk.get("typeshed-commit").and_then(|v| v.as_str()) {
        cfg.typeshed_commit = Some(val.to_owned());
    }
    // [STUBRES-TYPESHED-PYPI] (issue #312): `typeshed-package` pins a PyPI
    // typeshed distribution by wheel SHA-256 (`name@sha256:<hex>`); mutually
    // exclusive with `typeshed-commit` and `typeshed-path`.
    if let Some(val) = basilisk.get("typeshed-package").and_then(|v| v.as_str()) {
        cfg.typeshed_package = Some(val.to_owned());
    }
    if let Some(val) = basilisk.get("typeshed-store-path").and_then(|v| v.as_str()) {
        cfg.typeshed_store_path = Some(PathBuf::from(val));
    }

    // [CHKCACHE-CONFIG]: the persistent result cache is two keys — run it, and
    // where it lives. A non-boolean `cache` or non-string `cache-dir` is left
    // unset here; the configuration editor rejects both outright, so the only
    // way to reach this parser with one is a hand-edited file.
    if let Some(val) = basilisk.get(CACHE_KEY).and_then(toml::Value::as_bool) {
        cfg.cache_enabled = Some(val);
    }
    if let Some(val) = basilisk.get(CACHE_DIR_KEY).and_then(|v| v.as_str()) {
        cfg.cache_dir = Some(PathBuf::from(val));
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

    // [STUBRES-TYPESHED-CONFIG]: surface (never drop) malformed acquisition
    // values so a bad pin fails closed downstream instead of silently
    // resolving to a different commit.
    warn_on_malformed_typeshed_values(&cfg);

    Some(cfg)
}

/// Parse a `"<key>" = "<severity>"` TOML table into `target`.
///
/// An entry whose value is not one of the four severity names is dropped — the
/// key keeps whatever the rest of the walk decides ([CHKARCH-CONFIG-MODEL]).
/// Dropping it *silently* is the trap: `BSK-0001 = "eror"` then reads as a rule
/// the author graded, while the checker never sees the entry at all. The
/// configuration editor already rejects such a value outright, so a run that
/// merely ignored it would disagree with the editor about the same file. Warn
/// with the key and the offending spelling so the mismatch is visible — both
/// are author-written config identifiers, never PII.
fn parse_severity_map(table: &toml::Table, target: &mut HashMap<String, RuleSeverity>) {
    for (key, severity_val) in table {
        if let Some(severity) = severity_val.as_str().and_then(RuleSeverity::parse) {
            let _ = target.insert(key.clone(), severity);
        } else {
            tracing::warn!(
                key = key.as_str(),
                value = severity_val.to_string(),
                "ignoring config entry: not one of `error`, `warning`, `info`, `disabled` \
                 (or the aliases `warn`/`information`/`off`/`none`); the entry has no effect"
            );
        }
    }
}

/// Whether `sha` is a full 40-character hex git commit SHA — the only accepted
/// `typeshed-commit` form ([STUBRES-TYPESHED-CONFIG]).
///
/// Abbreviated or non-hex values are rejected so an exact pin is unambiguous
/// and can *fail closed* (D1) instead of silently resolving to a different
/// commit. Case is accepted on either side; git's canonical lower-case and a
/// pasted upper-case SHA identify the same immutable commit.
#[must_use]
pub fn is_full_commit_sha(sha: &str) -> bool {
    sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Parse a `typeshed-package` pin spec of the form `"name@sha256:<hex>"`
/// ([STUBRES-TYPESHED-PYPI], issue #312). The distribution name precedes the
/// `@sha256:` separator; the hash must be 64 hex characters. This is the one
/// parser for the pin — the LSP and config editor call it rather than
/// carrying a divergent twin.
///
/// # Errors
///
/// Returns a redacted, user-facing reason for a malformed spec.
pub fn parse_typeshed_package(spec: &str) -> Result<(String, String), String> {
    let (name, hash) = spec
        .split_once("@sha256:")
        .ok_or_else(|| "typeshed-package must be of the form `name@sha256:<64-hex>`".to_owned())?;
    if name.is_empty() {
        return Err("typeshed-package distribution name is empty".to_owned());
    }
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("typeshed-package sha256 must be 64 hex characters".to_owned());
    }
    Ok((name.to_owned(), hash.to_ascii_lowercase()))
}

/// Emit a structured warning for a malformed `typeshed-commit` pin
/// ([STUBRES-TYPESHED-CONFIG]). The value is kept verbatim on the config so
/// the runtime fails closed on a bad pin rather than silently dropping it;
/// this only surfaces the problem. The raw value is never logged.
fn warn_on_malformed_typeshed_values(cfg: &BasiliskConfig) {
    if let Some(sha) = cfg.typeshed_commit.as_deref() {
        if !is_full_commit_sha(sha) {
            tracing::warn!(
                len = sha.len(),
                "typeshed-commit is not a full 40-char hex SHA; the exact pin will fail closed"
            );
        }
    }
    if let Some(spec) = cfg.typeshed_package.as_deref() {
        if parse_typeshed_package(spec).is_err() {
            tracing::warn!(
                "typeshed-package is not `name@sha256:<64-hex>`; the package pin will fail closed"
            );
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test-only: a fixture document that fails to parse must abort naming it"
)]
#[path = "parse_tests.rs"]
mod validation_tests;
