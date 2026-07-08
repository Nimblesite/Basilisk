//! Implements [LSPARCH-CONFIG]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG
//!
//! Workspace configuration reader.
//!
//! Parses `pyrightconfig.json` and `pyproject.toml` `[tool.basilisk]` /
//! `[tool.pyright]` sections to configure strictness, include/exclude paths,
//! and Python version.

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
    /// Paths to include in analysis (relative to workspace root).
    pub include: Vec<PathBuf>,
    /// Gitignore-style glob patterns to exclude from analysis, matched relative
    /// to the workspace root (e.g. `**/bundled/**`, `vendor/**`, `*.pb.py`).
    /// Implements [CHKARCH-CONFIG-EXCLUDE].
    pub exclude: Vec<PathBuf>,
    /// Extra paths for module resolution (e.g. `src/`).
    pub extra_paths: Vec<PathBuf>,
    /// Strictness level.
    pub strict: bool,
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
    /// Formatter engine ([LSPFMT-CONFIG]). Editor settings (VS Code's
    /// `basilisk.formatter` via initializationOptions) override this.
    pub formatter: FormatterEngine,
    /// Style options for the embedded Ruff formatter ([LSPFMT-ENGINE]).
    pub format_style: FormatStyle,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            python_version: Some("3.12".to_owned()),
            python_platform: None,
            include: Vec::new(),
            exclude: Vec::new(),
            extra_paths: Vec::new(),
            strict: true, // Basilisk is strict-by-default
            venv_path: None,
            venv: None,
            analysis_mode: AnalysisMode::WholeModule,
            stub_paths: Vec::new(),
            typeshed_path: None,
            formatter: FormatterEngine::Ruff,
            format_style: FormatStyle::default(),
        }
    }
}

/// Load workspace configuration from the given root directory.
///
/// Searches for (in priority order):
/// 1. `basilisk.json`
/// 2. `pyrightconfig.json`
/// 3. `pyproject.toml` `[tool.basilisk]` or `[tool.pyright]`
///
/// Returns `Default` if no config file is found.
///
/// Relative `stub-paths` and `typeshed-path` are resolved against `root` so
/// that a bare `stub-paths = ["stubs"]` points at `<root>/stubs` regardless of
/// the process's current working directory (issue #173).
#[must_use]
pub fn load_config(root: &Path) -> WorkspaceConfig {
    let mut cfg = load_config_raw(root);
    cfg.stub_paths = cfg
        .stub_paths
        .into_iter()
        .map(|p| if p.is_absolute() { p } else { root.join(p) })
        .collect();
    cfg.typeshed_path = cfg
        .typeshed_path
        .map(|p| if p.is_absolute() { p } else { root.join(p) });
    // Formatter style always comes from `[tool.ruff]` in pyproject.toml,
    // independent of which file supplied the checker config ([LSPFMT-ENGINE]).
    cfg.format_style = load_format_style(root);
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
    // 1. basilisk.json
    let basilisk_json = root.join("basilisk.json");
    if basilisk_json.is_file() {
        if let Some(cfg) = load_json_config(&basilisk_json) {
            return cfg;
        }
    }

    // 2. pyrightconfig.json
    let pyright_json = root.join("pyrightconfig.json");
    if pyright_json.is_file() {
        if let Some(cfg) = load_json_config(&pyright_json) {
            return cfg;
        }
    }

    // 3. pyproject.toml — look for [tool.basilisk] or [tool.pyright]
    let pyproject = root.join("pyproject.toml");
    if pyproject.is_file() {
        if let Some(mut cfg) = load_pyproject_config(&pyproject) {
            // `load_pyproject_config` is a line scanner that cannot parse TOML
            // arrays (and mishandles inline comments), so `stub-paths` was
            // silently dropped (issue #173). Delegate the path fields to the
            // canonical toml-crate parser in `basilisk-config`, which reads the
            // same `[tool.basilisk]` section correctly.
            let bcfg = basilisk_config::load_basilisk_config(root);
            cfg.stub_paths = bcfg.stub_paths;
            cfg.typeshed_path = bcfg.typeshed_path;
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

/// Parse a JSON config file (basilisk.json or pyrightconfig.json).
fn load_json_config(path: &Path) -> Option<WorkspaceConfig> {
    let content = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let obj = json.as_object()?;

    let mut cfg = WorkspaceConfig::default();

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
    if let Some(v) = obj.get("typeCheckingMode").and_then(|v| v.as_str()) {
        cfg.strict = v == "strict" || v == "all";
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
    if let Some(arr) = obj
        .get("stubPaths")
        .or_else(|| obj.get("stub-paths"))
        .and_then(|v| v.as_array())
    {
        cfg.stub_paths = json_path_list(arr);
    }
    if let Some(v) = obj
        .get("typeshedPath")
        .or_else(|| obj.get("typeshed-path"))
        .and_then(|v| v.as_str())
    {
        cfg.typeshed_path = Some(PathBuf::from(v));
    }

    Some(cfg)
}

/// Parse pyproject.toml for `[tool.basilisk]` or `[tool.pyright]` config.
///
/// This is a minimal TOML parser — we only extract the fields we care about
/// from the raw JSON structure produced by `serde_json` after converting the
/// relevant TOML section. For a proper implementation, a TOML crate would be
/// used, but we avoid adding dependencies for now.
fn load_pyproject_config(path: &Path) -> Option<WorkspaceConfig> {
    let content = std::fs::read_to_string(path).ok()?;

    // Minimal extraction: find [tool.basilisk] or [tool.pyright] section.
    // We look for key = value pairs after the section header.
    let section_name = if content.contains("[tool.basilisk]") {
        "[tool.basilisk]"
    } else if content.contains("[tool.pyright]") {
        "[tool.pyright]"
    } else {
        return None;
    };

    let section_start = content.find(section_name)?;
    let after_header = &content[section_start + section_name.len()..];

    // Extract lines until the next section header.
    let mut cfg = WorkspaceConfig::default();
    for line in after_header.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            break; // next section
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            match key {
                "pythonVersion" | "python_version" => {
                    cfg.python_version = Some(value.to_owned());
                }
                "pythonPlatform" | "python_platform" => {
                    cfg.python_platform = Some(value.to_owned());
                }
                "typeCheckingMode" | "type_checking_mode" => {
                    cfg.strict = value == "strict" || value == "all";
                }
                "venvPath" | "venv_path" => {
                    cfg.venv_path = Some(PathBuf::from(value));
                }
                "venv" => {
                    cfg.venv = Some(value.to_owned());
                }
                "analysisMode" | "analysis_mode" => {
                    cfg.analysis_mode = AnalysisMode::parse(value);
                }
                "formatter" => {
                    cfg.formatter = FormatterEngine::parse(value);
                }
                "stubPaths" | "stub_paths" | "stub-paths" => {
                    // Simple single-value handling; array parsing requires TOML crate.
                    cfg.stub_paths.push(PathBuf::from(value));
                }
                "typeshedPath" | "typeshed_path" | "typeshed-path" => {
                    cfg.typeshed_path = Some(PathBuf::from(value));
                }
                _ => {}
            }
        }
    }

    Some(cfg)
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only code: unwrap acceptable in unit tests"
)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = WorkspaceConfig::default();
        assert_eq!(cfg.python_version.as_deref(), Some("3.12"));
        assert!(cfg.strict);
        assert!(cfg.include.is_empty());
        assert!(cfg.exclude.is_empty());
        assert!(cfg.typeshed_path.is_none());
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
                "typeCheckingMode": "basic",
                "include": ["src"],
                "exclude": ["tests", "build"],
                "extraPaths": ["vendor"]
            }"#,
        )
        .unwrap();

        let cfg = load_json_config(&config_path).unwrap();
        assert_eq!(cfg.python_version.as_deref(), Some("3.11"));
        assert!(!cfg.strict);
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
        assert_eq!(cfg.python_version.as_deref(), Some("3.12"));
        assert!(cfg.strict);
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
        let config_path = dir.join("basilisk.json");
        std::fs::write(&config_path, r#"{ "typeshedPath": "ts" }"#).unwrap();

        let cfg = load_json_config(&config_path).unwrap();
        assert_eq!(cfg.typeshed_path, Some(PathBuf::from("ts")));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
