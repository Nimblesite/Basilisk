//! Implements [LSPARCH-CONFIG]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG
//!
//! Workspace configuration reader.
//!
//! Parses `pyrightconfig.json` and `pyproject.toml` `[tool.basilisk]` /
//! `[tool.pyright]` sections to configure include/exclude paths, import
//! resolution, and Python version. Rule strictness lives in rule configuration.

use std::path::{Path, PathBuf};

/// Controls which files the LSP server analyses.
///
/// See `docs/LSP-ANALYSIS-MODES-SPEC.md` for the full specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnalysisMode {
    /// Analyse only files currently open in the editor.
    OpenFilesOnly,
    /// Analyse all workspace Python files (default, strict-by-default).
    #[default]
    WholeModule,
    /// Cross-file import graph analysis (reserved for future use).
    CrossModule,
}

impl AnalysisMode {
    /// Parse from the string values used in config files and VS Code settings.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s {
            "openFilesOnly" => Self::OpenFilesOnly,
            "crossModule" => Self::CrossModule,
            _ => Self::WholeModule,
        }
    }
}

/// Formatter engine selection ([LSPFMT-CONFIG]).
///
/// `"ruff"` (default) is the Ruff formatter embedded in the binary
/// ([LSPFMT-ENGINE]); `"none"` disables formatting entirely — the server
/// does not advertise formatting capabilities. `"basilisk"` is reserved for
/// a future native formatter and currently behaves like the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FormatterEngine {
    /// The embedded Ruff formatter (default).
    #[default]
    Ruff,
    /// Formatting disabled; no formatting capabilities are advertised.
    Disabled,
}

impl FormatterEngine {
    /// Parse from the `basilisk.formatter` setting values.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s {
            "none" => Self::Disabled,
            _ => Self::Ruff,
        }
    }
}

/// Style options for the embedded Ruff formatter, read from the project's
/// `[tool.ruff]` / `[tool.ruff.format]` sections in `pyproject.toml`
/// ([LSPFMT-ENGINE]). `None`/`false` fields keep Ruff's own defaults.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FormatStyle {
    /// `[tool.ruff] line-length` (Ruff default: 88).
    pub line_length: Option<u16>,
    /// `[tool.ruff.format] quote-style`: `"double"`, `"single"`, `"preserve"`.
    pub quote_style: Option<String>,
    /// `[tool.ruff.format] indent-style`: `"space"` or `"tab"`.
    pub indent_style: Option<String>,
    /// `[tool.ruff.format] skip-magic-trailing-comma`.
    pub skip_magic_trailing_comma: bool,
}

/// Workspace configuration derived from config files.
#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    /// Python version target (e.g. "3.12").
    pub python_version: Option<String>,
    /// Python platform target (e.g. "Linux", "Darwin", "Windows").
    pub python_platform: Option<String>,
    /// Explicit Python interpreter binary (VS Code's `basilisk.python` via
    /// `initializationOptions`, or `--python`). When set, import resolution
    /// uses **this** interpreter's `site-packages`/`dist-packages` — the
    /// cross-version target-environment override
    /// ([TYPESHEDRT-ACCEPTANCE-TARGET]). `None` uses ambient discovery.
    pub python_interpreter: Option<PathBuf>,
    /// Paths to include in analysis (relative to workspace root).
    pub include: Vec<PathBuf>,
    /// Gitignore-style glob patterns to exclude from analysis, matched relative
    /// to the workspace root (e.g. `**/bundled/**`, `vendor/**`, `*.pb.py`).
    /// Implements [CHKARCH-CONFIG-EXCLUDE].
    pub exclude: Vec<PathBuf>,
    /// Extra paths for module resolution (e.g. `src/`).
    pub extra_paths: Vec<PathBuf>,
    /// Venv path for resolving third-party packages.
    pub venv_path: Option<PathBuf>,
    /// The venv name within `venv_path`.
    pub venv: Option<String>,
    /// Which files to analyse. Defaults to `WholeModule`.
    pub analysis_mode: AnalysisMode,
    /// Additional directories to search for `.pyi` stub files.
    pub stub_paths: Vec<PathBuf>,
    /// Custom typeshed directory (`typeshed-path`) whose `stdlib/` subtree is
    /// the canonical source for standard-library types, overriding the bundled
    /// typeshed (typing-spec import-resolution step 3 —
    /// [STUBRES-CUSTOM-TYPESHED](../../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED)).
    /// `None` uses the bundled typeshed.
    pub typeshed_path: Option<PathBuf>,
    /// Exact full `python/typeshed` commit pin; unset resolves to the bundled
    /// commit with an `UNPINNED` warning ([STUBRES-TYPESHED-CONFIG]).
    pub typeshed_commit: Option<String>,
    /// Optional verified content-addressed store directory
    /// ([STUBRES-TYPESHED-STORE]); unset uses the per-user OS default.
    pub typeshed_store_path: Option<PathBuf>,
    /// Redacted parse error for an explicitly malformed Typeshed key.
    /// Acquisition fails closed instead of silently applying a default.
    pub typeshed_configuration_error: Option<String>,
    /// Formatter engine ([LSPFMT-CONFIG]). Editor settings (VS Code's
    /// `basilisk.formatter` via initializationOptions) override this.
    pub formatter: FormatterEngine,
    /// Style options for the embedded Ruff formatter ([LSPFMT-ENGINE]).
    pub format_style: FormatStyle,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            python_version: None,
            python_platform: None,
            python_interpreter: None,
            include: Vec::new(),
            // The effective exclude list, resolved the same way `basilisk
            // check` resolves it ([CHKARCH-CONFIG-EXCLUDE]): the defaults
            // stand until a config supplies `exclude`, and supplying it
            // REPLACES them. Callers therefore never need a second hardcoded
            // default set, which no configuration could switch off.
            exclude: basilisk_config::DEFAULT_EXCLUDES
                .iter()
                .map(PathBuf::from)
                .collect(),
            extra_paths: Vec::new(),
            venv_path: None,
            venv: None,
            analysis_mode: AnalysisMode::WholeModule,
            stub_paths: Vec::new(),
            typeshed_path: None,
            typeshed_commit: None,
            typeshed_store_path: None,
            typeshed_configuration_error: None,
            formatter: FormatterEngine::Ruff,
            format_style: FormatStyle::default(),
        }
    }
}

/// Build the config-free resolution request shared by CLI, LSP, and MCP.
/// Invalid or mutually exclusive raw settings fail closed here, including when
/// a file bypassed the configuration editor's stronger source validation.
///
/// There are exactly two sources ([STUBRES-TYPESHED]): a pinned commit or a
/// custom folder. An unset `typeshed-commit` resolves to the bundled commit as
/// an implicit pin (still `UNPINNED`); resolution never downloads.
///
/// # Errors
///
/// Returns a redacted, user-facing reason for an invalid source or pin.
pub fn typeshed_request(
    config: &WorkspaceConfig,
) -> Result<basilisk_stubs::typeshed::source::TypeshedRequest, String> {
    use basilisk_stubs::typeshed::bundle::bundled_commit_sha;
    use basilisk_stubs::typeshed::gittree::Oid;
    use basilisk_stubs::typeshed::source::{SourceSelection, TypeshedRequest};

    if let Some(error) = config.typeshed_configuration_error.as_ref() {
        return Err(error.clone());
    }

    if config.typeshed_path.is_some() && config.typeshed_commit.is_some() {
        return Err("typeshed-path and typeshed-commit are mutually exclusive".to_owned());
    }
    let selection = if let Some(path) = config.typeshed_path.as_deref() {
        SourceSelection::Custom {
            path: path
                .to_str()
                .ok_or_else(|| "typeshed-path is not valid UTF-8".to_owned())?
                .to_owned(),
        }
    } else if let Some(commit) = config.typeshed_commit.as_deref() {
        SourceSelection::Pinned {
            commit: Oid::from_hex(commit).map_err(|_invalid_oid| {
                "typeshed-commit must be a full 40-character hex SHA".to_owned()
            })?,
            explicit: true,
        }
    } else {
        SourceSelection::Pinned {
            commit: Oid::from_hex(bundled_commit_sha())
                .map_err(|_invalid_oid| "bundled typeshed identity is unreadable".to_owned())?,
            explicit: false,
        }
    };
    Ok(TypeshedRequest {
        selection,
        store_path: config.typeshed_store_path.clone(),
    })
}

/// Load workspace configuration from the given root directory.
///
/// Searches for (in priority order):
/// 1. `pyrightconfig.json` (pyright compatibility)
/// 2. `pyproject.toml` `[tool.basilisk]` or `[tool.pyright]`
///
/// Returns `Default` if no config file is found.
///
/// Relative `stub-paths` and `typeshed-path` are resolved against `root` so
/// that a bare `stub-paths = ["stubs"]` points at `<root>/stubs` regardless of
/// the process's current working directory (issue #173).
#[must_use]
pub fn load_config(root: &Path) -> WorkspaceConfig {
    let mut cfg = load_analysis_config(root);
    // Formatter style always comes from `[tool.ruff]` in pyproject.toml,
    // independent of which file supplied the checker config ([LSPFMT-ENGINE]).
    cfg.format_style = load_format_style(root);
    cfg
}

/// Load only configuration used by analysis and import resolution.
///
/// CLI checks do not format source, so reparsing `pyproject.toml` solely for
/// Ruff style is wasted startup work. The LSP's full [`load_config`] adds that
/// editor-only configuration while this path preserves every analysis field.
#[must_use]
pub fn load_analysis_config(root: &Path) -> WorkspaceConfig {
    let mut cfg = load_config_raw(root);
    cfg.stub_paths = cfg
        .stub_paths
        .into_iter()
        .map(|p| if p.is_absolute() { p } else { root.join(p) })
        .collect();
    cfg.typeshed_path = cfg
        .typeshed_path
        .map(|p| if p.is_absolute() { p } else { root.join(p) });
    cfg.typeshed_store_path =
        cfg.typeshed_store_path
            .map(|p| if p.is_absolute() { p } else { root.join(p) });
    cfg
}

/// Read `[tool.ruff]` / `[tool.ruff.format]` style options from
/// `pyproject.toml` so the embedded formatter's output matches what the
/// user's own `ruff format` would produce ([LSPFMT-ENGINE]).
#[must_use]
pub fn load_format_style(root: &Path) -> FormatStyle {
    let Ok(content) = std::fs::read_to_string(root.join("pyproject.toml")) else {
        return FormatStyle::default();
    };
    let Ok(table) = content.parse::<toml::Table>() else {
        return FormatStyle::default();
    };
    let Some(ruff) = table.get("tool").and_then(|t| t.get("ruff")) else {
        return FormatStyle::default();
    };

    let mut style = FormatStyle {
        line_length: ruff
            .get("line-length")
            .and_then(toml::Value::as_integer)
            .and_then(|v| u16::try_from(v).ok()),
        ..FormatStyle::default()
    };
    if let Some(format) = ruff.get("format") {
        style.quote_style = format
            .get("quote-style")
            .and_then(toml::Value::as_str)
            .map(str::to_owned);
        style.indent_style = format
            .get("indent-style")
            .and_then(toml::Value::as_str)
            .map(str::to_owned);
        style.skip_magic_trailing_comma = format
            .get("skip-magic-trailing-comma")
            .and_then(toml::Value::as_bool)
            .unwrap_or(false);
    }
    style
}

/// Parse the config file without post-processing the resulting paths.
fn load_config_raw(root: &Path) -> WorkspaceConfig {
    // 1. pyrightconfig.json (pyright compatibility)
    let pyright_json = root.join("pyrightconfig.json");
    if pyright_json.is_file() {
        if let Some(cfg) = load_json_config(&pyright_json) {
            return cfg;
        }
    }

    // 2. pyproject.toml — [tool.basilisk] or, failing that, [tool.pyright]
    let pyproject = root.join("pyproject.toml");
    if pyproject.is_file() {
        if let Some(cfg) = load_pyproject_config(&pyproject) {
            return cfg;
        }
    }

    WorkspaceConfig::default()
}

/// Collect string entries of a JSON array as `PathBuf`s, dropping non-strings.
fn json_path_list(arr: &[serde_json::Value]) -> Vec<PathBuf> {
    arr.iter()
        .filter_map(|v| v.as_str().map(PathBuf::from))
        .collect()
}

/// Stub directories from a `pyrightconfig.json`-shaped object.
///
/// Pyright's own key is the **singular** `stubPath`, holding a single path
/// string (its default is `./typings`) — not the plural array Basilisk also
/// accepts. Reading only the plural meant a `pyrightconfig.json` copied
/// verbatim silently lost its stub directory, which is precisely the config
/// this loader exists to understand. `stub-paths` is the step-1 "manual path
/// head" config key ([STUBRES-PEP561-MAPPING]).
///
/// An explicit list wins when both spellings are present, since it is the
/// strictly more expressive of the two.
fn json_stub_paths(obj: &serde_json::Map<String, serde_json::Value>) -> Vec<PathBuf> {
    obj.get("stubPaths")
        .or_else(|| obj.get("stub-paths"))
        .and_then(|v| v.as_array())
        .map(|arr| json_path_list(arr))
        .or_else(|| {
            obj.get("stubPath")
                .or_else(|| obj.get("stub-path"))
                .and_then(serde_json::Value::as_str)
                .map(|path| vec![PathBuf::from(path)])
        })
        .unwrap_or_default()
}

/// Parse a `pyrightconfig.json` compatibility config file.
fn load_json_config(path: &Path) -> Option<WorkspaceConfig> {
    let content = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let obj = json.as_object()?;

    let mut cfg = WorkspaceConfig {
        typeshed_configuration_error: json_typeshed_type_error(obj),
        ..WorkspaceConfig::default()
    };

    if let Some(v) = obj.get("pythonVersion").and_then(|v| v.as_str()) {
        cfg.python_version = Some(v.to_owned());
    }
    if let Some(v) = obj.get("pythonPlatform").and_then(|v| v.as_str()) {
        cfg.python_platform = Some(v.to_owned());
    }
    if let Some(arr) = obj.get("include").and_then(|v| v.as_array()) {
        cfg.include = json_path_list(arr);
    }
    if let Some(arr) = obj.get("exclude").and_then(|v| v.as_array()) {
        cfg.exclude = json_path_list(arr);
    }
    if let Some(arr) = obj.get("extraPaths").and_then(|v| v.as_array()) {
        cfg.extra_paths = json_path_list(arr);
    }
    if let Some(v) = obj.get("venvPath").and_then(|v| v.as_str()) {
        cfg.venv_path = Some(PathBuf::from(v));
    }
    if let Some(v) = obj.get("venv").and_then(|v| v.as_str()) {
        cfg.venv = Some(v.to_owned());
    }
    if let Some(v) = obj.get("analysisMode").and_then(|v| v.as_str()) {
        cfg.analysis_mode = AnalysisMode::parse(v);
    }
    if let Some(v) = obj.get("formatter").and_then(|v| v.as_str()) {
        cfg.formatter = FormatterEngine::parse(v);
    }
    cfg.stub_paths = json_stub_paths(obj);
    if let Some(v) = obj
        .get("typeshedPath")
        .or_else(|| obj.get("typeshed-path"))
        .and_then(|v| v.as_str())
    {
        cfg.typeshed_path = Some(PathBuf::from(v));
    }
    if let Some(v) = obj
        .get("typeshedCommit")
        .or_else(|| obj.get("typeshed-commit"))
        .and_then(|v| v.as_str())
    {
        cfg.typeshed_commit = Some(v.to_owned());
    }
    if let Some(v) = obj
        .get("typeshedStorePath")
        .or_else(|| obj.get("typeshed-store-path"))
        .and_then(|v| v.as_str())
    {
        cfg.typeshed_store_path = Some(PathBuf::from(v));
    }

    Some(cfg)
}

/// Parse pyproject.toml for `[tool.basilisk]` or, failing that, `[tool.pyright]`
/// (pyright compatibility) analysis settings.
///
/// Uses the real `toml` parser, so array-valued fields (`include`, `exclude`,
/// `extra-paths`, `stub-paths`) parse correctly — the previous line scanner
/// silently dropped them (issue #173).
fn load_pyproject_config(path: &Path) -> Option<WorkspaceConfig> {
    let content = std::fs::read_to_string(path).ok()?;
    let table: toml::Table = content.parse().ok()?;
    let tool = table.get("tool")?.as_table()?;
    let section = tool
        .get("basilisk")
        .or_else(|| tool.get("pyright"))?
        .as_table()?;
    Some(workspace_config_from_toml(section))
}

/// Map one `[tool.basilisk]` / `[tool.pyright]` table onto a `WorkspaceConfig`.
///
/// Accepts Basilisk's kebab-case spellings and pyright's camelCase equivalents.
fn workspace_config_from_toml(section: &toml::Table) -> WorkspaceConfig {
    let mut cfg = WorkspaceConfig {
        typeshed_configuration_error: toml_typeshed_type_error(section),
        ..WorkspaceConfig::default()
    };
    if let Some(v) = toml_str(section, &["python-version", "pythonVersion"]) {
        cfg.python_version = Some(v.to_owned());
    }
    if let Some(v) = toml_str(section, &["python-platform", "pythonPlatform"]) {
        cfg.python_platform = Some(v.to_owned());
    }
    // Explicit interpreter binary — VS Code's `basilisk.python`, or a
    // `python`/`python-path` config key ([TYPESHEDRT-ACCEPTANCE-TARGET]).
    if let Some(v) = toml_str(section, &["python", "python-path", "pythonPath"]) {
        cfg.python_interpreter = Some(PathBuf::from(v));
    }
    if let Some(paths) = toml_paths(section, &["include"]) {
        cfg.include = paths;
    }
    if let Some(paths) = toml_paths(section, &["exclude"]) {
        cfg.exclude = paths;
    }
    if let Some(paths) = toml_paths(section, &["extra-paths", "extraPaths"]) {
        cfg.extra_paths = paths;
    }
    if let Some(v) = toml_str(section, &["venv-path", "venvPath"]) {
        cfg.venv_path = Some(PathBuf::from(v));
    }
    if let Some(v) = toml_str(section, &["venv"]) {
        cfg.venv = Some(v.to_owned());
    }
    if let Some(v) = toml_str(section, &["analysis-mode", "analysisMode"]) {
        cfg.analysis_mode = AnalysisMode::parse(v);
    }
    if let Some(v) = toml_str(section, &["formatter"]) {
        cfg.formatter = FormatterEngine::parse(v);
    }
    // Pyright spells this `stubPath` (singular, one path string); Basilisk's
    // own key is the plural array. Accept both, list first — see
    // `json_stub_paths` for why the singular spelling must not be dropped.
    if let Some(paths) = toml_paths(section, &["stub-paths", "stubPaths"]).or_else(|| {
        toml_str(section, &["stub-path", "stubPath"]).map(|path| vec![PathBuf::from(path)])
    }) {
        cfg.stub_paths = paths;
    }
    if let Some(v) = toml_str(section, &["typeshed-path", "typeshedPath"]) {
        cfg.typeshed_path = Some(PathBuf::from(v));
    }
    if let Some(v) = toml_str(section, &["typeshed-commit", "typeshedCommit"]) {
        cfg.typeshed_commit = Some(v.to_owned());
    }
    if let Some(v) = toml_str(section, &["typeshed-url", "typeshedUrl"]) {
        cfg.typeshed_url = Some(v.to_owned());
    }
    if let Some(v) = toml_str(section, &["typeshed-cache-path", "typeshedCachePath"]) {
        cfg.typeshed_cache_path = Some(PathBuf::from(v));
    }
    if let Some(v) = toml_bool(section, &["typeshed-cache", "typeshedCache"]) {
        cfg.typeshed_cache = v;
    }
    if let Some(v) = toml_bool(section, &["typeshed-verify", "typeshedVerify"]) {
        cfg.typeshed_verify = v;
    }
    cfg
}

/// First string value found among the given key spellings.
fn toml_str<'a>(table: &'a toml::Table, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| table.get(*key).and_then(toml::Value::as_str))
}

/// First boolean value found among the given key spellings.
fn toml_bool(table: &toml::Table, keys: &[&str]) -> Option<bool> {
    keys.iter()
        .find_map(|key| table.get(*key).and_then(toml::Value::as_bool))
}

fn toml_typeshed_type_error(table: &toml::Table) -> Option<String> {
    for key in [
        "typeshed-path",
        "typeshedPath",
        "typeshed-commit",
        "typeshedCommit",
        "typeshed-url",
        "typeshedUrl",
        "typeshed-cache-path",
        "typeshedCachePath",
    ] {
        if table.get(key).is_some_and(|value| !value.is_str()) {
            return Some(format!("{key} must be a string"));
        }
    }
    for key in [
        "typeshed-cache",
        "typeshedCache",
        "typeshed-verify",
        "typeshedVerify",
    ] {
        if table.get(key).is_some_and(|value| !value.is_bool()) {
            return Some(format!("{key} must be a boolean"));
        }
    }
    None
}

fn json_typeshed_type_error(object: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    for key in [
        "typeshed-path",
        "typeshedPath",
        "typeshed-commit",
        "typeshedCommit",
        "typeshed-url",
        "typeshedUrl",
        "typeshed-cache-path",
        "typeshedCachePath",
    ] {
        if object.get(key).is_some_and(|value| !value.is_string()) {
            return Some(format!("{key} must be a string"));
        }
    }
    for key in [
        "typeshed-cache",
        "typeshedCache",
        "typeshed-verify",
        "typeshedVerify",
    ] {
        if object.get(key).is_some_and(|value| !value.is_boolean()) {
            return Some(format!("{key} must be a boolean"));
        }
    }
    None
}

/// First array value found among the given key spellings, as `PathBuf`s.
fn toml_paths(table: &toml::Table, keys: &[&str]) -> Option<Vec<PathBuf>> {
    let arr = keys
        .iter()
        .find_map(|key| table.get(*key).and_then(toml::Value::as_array))?;
    Some(
        arr.iter()
            .filter_map(|v| v.as_str().map(PathBuf::from))
            .collect(),
    )
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "test-only configuration fixtures use explicit assertion messages"
)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = WorkspaceConfig::default();
        assert_eq!(
            cfg.python_version, None,
            "[STUBRES-TYPESHED-VERSION] no Python target is manufactured"
        );
        assert!(cfg.include.is_empty());
        // [CHKARCH-CONFIG-EXCLUDE] An unconfigured workspace resolves to the
        // same effective exclude list `basilisk check` uses, so the editor and
        // the CLI scan identical trees. The scan applies this list and nothing
        // else — an empty default here would make it scan `node_modules`.
        assert_eq!(
            cfg.exclude,
            basilisk_config::DEFAULT_EXCLUDES
                .iter()
                .map(PathBuf::from)
                .collect::<Vec<_>>(),
            "the default config must carry DEFAULT_EXCLUDES, not an empty list"
        );
        assert!(cfg.typeshed_path.is_none());
    }

    /// [STUBRES-TYPESHED-CONFIG]: the LSP consumes the same complete source
    /// policy as the CLI/config crate; path fields are rooted at the workspace.
    #[test]
    fn test_load_runtime_typeshed_policy_from_pyproject() {
        let dir = std::env::temp_dir().join("basilisk_cfg_typeshed_runtime");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("pyproject.toml"),
            concat!(
                "[tool.basilisk]\n",
                "typeshed-commit = \"83c2518a9e6abbda0c44592c3483de459198f887\"\n",
                "typeshed-url = \"https://mirror.invalid/{sha}.zip\"\n",
                "typeshed-cache-path = \".cache/typeshed\"\n",
                "typeshed-cache = false\n",
                "typeshed-verify = false\n",
            ),
        )
        .unwrap();

        let cfg = load_analysis_config(&dir);
        assert_eq!(
            cfg.typeshed_commit.as_deref(),
            Some("83c2518a9e6abbda0c44592c3483de459198f887")
        );
        assert_eq!(
            cfg.typeshed_url.as_deref(),
            Some("https://mirror.invalid/{sha}.zip")
        );
        assert_eq!(cfg.typeshed_cache_path, Some(dir.join(".cache/typeshed")));
        assert!(!cfg.typeshed_cache);
        assert!(!cfg.typeshed_verify);
        assert_eq!(cfg.python_version, None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_json_config() {
        let dir = std::env::temp_dir().join("basilisk_cfg_test");
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("pyrightconfig.json");
        std::fs::write(
            &config_path,
            r#"{
                "pythonVersion": "3.11",
                "include": ["src"],
                "exclude": ["tests", "build"],
                "extraPaths": ["vendor"]
            }"#,
        )
        .unwrap();

        let cfg = load_json_config(&config_path).unwrap();
        assert_eq!(cfg.python_version.as_deref(), Some("3.11"));
        assert_eq!(cfg.include, vec![PathBuf::from("src")]);
        assert_eq!(
            cfg.exclude,
            vec![PathBuf::from("tests"), PathBuf::from("build")]
        );
        assert_eq!(cfg.extra_paths, vec![PathBuf::from("vendor")]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_config_fallback_to_default() {
        let dir = std::env::temp_dir().join("basilisk_cfg_empty");
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = load_config(&dir);
        assert_eq!(cfg.python_version, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn analysis_config_skips_formatter_style_but_keeps_checker_paths() {
        let dir = std::env::temp_dir().join("basilisk_cfg_analysis_only");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("pyproject.toml"),
            "[tool.basilisk]\nstub-paths = [\"stubs\"]\n[tool.ruff]\nline-length = 120\n",
        )
        .unwrap();

        let analysis = load_analysis_config(&dir);
        assert_eq!(analysis.stub_paths, vec![dir.join("stubs")]);
        assert_eq!(analysis.format_style, FormatStyle::default());
        assert_eq!(load_config(&dir).format_style.line_length, Some(120));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_typeshed_path_from_pyproject_relative() {
        let dir = std::env::temp_dir().join("basilisk_cfg_typeshed_toml");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("pyproject.toml"),
            "[tool.basilisk]\ntypeshed-path = \"typeshed-mp\"\n",
        )
        .unwrap();

        // A relative `typeshed-path` resolves against the workspace root, like
        // `stub-paths` ([STUBRES-CUSTOM-TYPESHED]).
        let cfg = load_config(&dir);
        assert_eq!(cfg.typeshed_path, Some(dir.join("typeshed-mp")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_typeshed_path_from_json() {
        let dir = std::env::temp_dir().join("basilisk_cfg_typeshed_json");
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("pyrightconfig.json");
        std::fs::write(&config_path, r#"{ "typeshedPath": "ts" }"#).unwrap();

        let cfg = load_json_config(&config_path).unwrap();
        assert_eq!(cfg.typeshed_path, Some(PathBuf::from("ts")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    // The pyproject loader uses the real `toml` parser, so array-valued fields
    // like `include` / `exclude` / `stub-paths` under `[tool.basilisk]` parse
    // correctly — the previous line scanner silently dropped them (issue #173).
    #[test]
    fn test_load_pyproject_basilisk_arrays() {
        let dir = std::env::temp_dir().join("basilisk_cfg_pyproject_arrays");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("pyproject.toml"),
            concat!(
                "[tool.basilisk]\n",
                "python-version = \"3.11\"\n",
                "include = [\"src\", \"tools\"]\n",
                "exclude = [\"**/generated/**\", \"*.pb.py\"]\n",
                "extra-paths = [\"vendor\"]\n",
                "stub-paths = [\"stubs\", \"more-stubs\"]\n",
            ),
        )
        .unwrap();

        let cfg = load_config(&dir);
        assert_eq!(cfg.python_version.as_deref(), Some("3.11"));
        assert_eq!(
            cfg.include,
            vec![PathBuf::from("src"), PathBuf::from("tools")]
        );
        assert_eq!(
            cfg.exclude,
            vec![PathBuf::from("**/generated/**"), PathBuf::from("*.pb.py")]
        );
        assert_eq!(cfg.extra_paths, vec![PathBuf::from("vendor")]);
        // Relative stub paths resolve against the workspace root.
        assert_eq!(
            cfg.stub_paths,
            vec![dir.join("stubs"), dir.join("more-stubs")]
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // Pyright's stub-directory key is the SINGULAR `stubPath` holding one path
    // string, not the plural array. Reading only the plural made a real
    // pyrightconfig.json — the exact file this compatibility loader exists to
    // understand — silently lose its stub directory, so every stub in it went
    // unresolved with no diagnostic ([STUBRES-PEP561-MAPPING], step 1).
    #[test]
    fn pyright_singular_stub_path_is_not_silently_dropped() {
        let dir = std::env::temp_dir().join("basilisk_cfg_pyright_stub_path");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("pyrightconfig.json"),
            "{\"stubPath\": \"typings\"}\n",
        )
        .unwrap();

        let cfg = load_config(&dir);
        assert_eq!(
            cfg.stub_paths,
            vec![dir.join("typings")],
            "pyright's singular `stubPath` must contribute a stub directory"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // When both spellings appear the explicit list wins: it is strictly more
    // expressive, so honouring the single-path key instead would lose entries.
    #[test]
    fn explicit_stub_paths_list_wins_over_singular_stub_path() {
        let dir = std::env::temp_dir().join("basilisk_cfg_stub_path_both");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("pyrightconfig.json"),
            "{\"stubPath\": \"typings\", \"stubPaths\": [\"a\", \"b\"]}\n",
        )
        .unwrap();

        let cfg = load_config(&dir);
        assert_eq!(cfg.stub_paths, vec![dir.join("a"), dir.join("b")]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // The same singular spelling must work in the TOML surface too.
    #[test]
    fn pyright_singular_stub_path_is_honoured_in_pyproject() {
        let dir = std::env::temp_dir().join("basilisk_cfg_toml_stub_path");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("pyproject.toml"),
            "[tool.pyright]\nstubPath = \"typings\"\n",
        )
        .unwrap();

        let cfg = load_config(&dir);
        assert_eq!(cfg.stub_paths, vec![dir.join("typings")]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // `[tool.pyright]` (camelCase spellings) is the compatibility fallback when
    // no `[tool.basilisk]` table exists.
    #[test]
    fn test_load_pyproject_pyright_fallback() {
        let dir = std::env::temp_dir().join("basilisk_cfg_pyproject_pyright");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("pyproject.toml"),
            concat!(
                "[tool.pyright]\n",
                "pythonVersion = \"3.10\"\n",
                "exclude = [\"build\"]\n",
                "extraPaths = [\"vendor\"]\n",
            ),
        )
        .unwrap();

        let cfg = load_config(&dir);
        assert_eq!(cfg.python_version.as_deref(), Some("3.10"));
        assert_eq!(cfg.exclude, vec![PathBuf::from("build")]);
        assert_eq!(cfg.extra_paths, vec![PathBuf::from("vendor")]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn typeshed_request_rejects_raw_source_conflict_and_bad_mirror() {
        let conflict = WorkspaceConfig {
            typeshed_path: Some(PathBuf::from("custom")),
            typeshed_commit: Some("83c2518a9e6abbda0c44592c3483de459198f887".to_owned()),
            ..WorkspaceConfig::default()
        };
        assert!(typeshed_request(&conflict)
            .expect_err("conflicting source settings must fail closed")
            .contains("mutually exclusive"));

        let mirror = WorkspaceConfig {
            typeshed_url: Some("http://secret.invalid/{sha}.zip".to_owned()),
            ..WorkspaceConfig::default()
        };
        let error = typeshed_request(&mirror).expect_err("HTTP mirror must fail closed");
        assert!(error.contains("HTTPS"));
        assert!(
            !error.contains("secret.invalid"),
            "mirror URL must be redacted"
        );
    }

    /// [STUBRES-TYPESHED-CONFIG]: an explicitly malformed acquisition key
    /// cannot disappear into a default Latest/verified/cache-on request.
    #[test]
    fn malformed_typeshed_setting_types_fail_closed() {
        for source in [
            "typeshed-commit = 42",
            "typeshed-path = false",
            "typeshed-cache = \"false\"",
            "typeshed-verify = \"true\"",
        ] {
            let table: toml::Table = source.parse().expect("fixture TOML");
            let config = workspace_config_from_toml(&table);
            assert!(
                typeshed_request(&config).is_err(),
                "malformed value must fail closed: {source}"
            );
        }

        for (key, value) in [
            ("typeshedCommit", serde_json::json!(42)),
            ("typeshedPath", serde_json::json!(false)),
            ("typeshedCache", serde_json::json!("false")),
            ("typeshedVerify", serde_json::json!("true")),
        ] {
            let mut object = serde_json::Map::new();
            let _ = object.insert(key.to_owned(), value);
            let config = WorkspaceConfig {
                typeshed_configuration_error: json_typeshed_type_error(&object),
                ..WorkspaceConfig::default()
            };
            assert!(
                typeshed_request(&config).is_err(),
                "malformed JSON value must fail closed: {key}"
            );
        }
    }

    #[test]
    fn typeshed_request_preserves_exact_policy_controls() {
        let config = WorkspaceConfig {
            typeshed_commit: Some("83C2518A9E6ABBDA0C44592C3483DE459198F887".to_owned()),
            typeshed_url: Some("https://mirror.invalid/{sha}.zip".to_owned()),
            typeshed_cache: false,
            typeshed_verify: false,
            ..WorkspaceConfig::default()
        };
        let request = typeshed_request(&config).expect("valid exact request");
        assert!(matches!(
            request.selection,
            basilisk_stubs::typeshed::source::SourceSelection::ExactCommit { .. }
        ));
        assert!(!request.use_cache);
        assert!(!request.verify_content);
    }
}
