//! Workspace configuration reader.
//!
//! Parses `pyrightconfig.json` and `pyproject.toml` `[tool.basilisk]` /
//! `[tool.pyright]` sections to configure strictness, include/exclude paths,
//! and Python version.

use std::path::{Path, PathBuf};

/// Controls which files the LSP server analyses.
///
/// See `docs/WHOLE-MODULE-ANALYSIS-SPEC.md` for the full specification.
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
    pub fn from_str(s: &str) -> Self {
        match s {
            "openFilesOnly" => Self::OpenFilesOnly,
            "crossModule" => Self::CrossModule,
            _ => Self::WholeModule,
        }
    }
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
    /// Paths to exclude from analysis (relative to workspace root).
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
#[must_use]
pub fn load_config(root: &Path) -> WorkspaceConfig {
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
        if let Some(cfg) = load_pyproject_config(&pyproject) {
            return cfg;
        }
    }

    WorkspaceConfig::default()
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
        cfg.include = arr
            .iter()
            .filter_map(|v| v.as_str().map(PathBuf::from))
            .collect();
    }
    if let Some(arr) = obj.get("exclude").and_then(|v| v.as_array()) {
        cfg.exclude = arr
            .iter()
            .filter_map(|v| v.as_str().map(PathBuf::from))
            .collect();
    }
    if let Some(arr) = obj.get("extraPaths").and_then(|v| v.as_array()) {
        cfg.extra_paths = arr
            .iter()
            .filter_map(|v| v.as_str().map(PathBuf::from))
            .collect();
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
        cfg.analysis_mode = AnalysisMode::from_str(v);
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
                    cfg.analysis_mode = AnalysisMode::from_str(value);
                }
                _ => {}
            }
        }
    }

    Some(cfg)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = WorkspaceConfig::default();
        assert_eq!(cfg.python_version.as_deref(), Some("3.12"));
        assert!(cfg.strict);
        assert!(cfg.include.is_empty());
        assert!(cfg.exclude.is_empty());
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
}
