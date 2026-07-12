//! Validated, revisioned configuration documents and structure-aware patches.
//!
//! Implements [CONFIGEDITOR-SOURCES]. This is the reusable persistence domain;
//! LSP and CLI callers provide rule intent but never parse or edit config text.

mod patch;

use std::path::{Path, PathBuf};

use crate::BasiliskConfig;

pub use patch::{build_rule_patch, RuleConfigScope, RuleConfigUpdate};

/// The active on-disk representation for one workspace root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    /// `[tool.basilisk]` inside `pyproject.toml`.
    PyprojectToml,
    /// Root-level `basilisk.json`.
    BasiliskJson,
}

/// A validated active configuration source with optimistic-lock revision.
#[derive(Debug, Clone)]
pub struct ConfigDocument {
    /// Workspace root this source belongs to.
    pub root: PathBuf,
    /// Active source path (existing or the default creation target).
    pub path: PathBuf,
    /// Active source representation.
    pub format: ConfigFormat,
    /// Whether the active target already exists.
    pub exists: bool,
    /// Whether filesystem metadata marks the source read-only.
    pub read_only: bool,
    /// Lower-priority existing sources ignored by discovery.
    pub shadowed_sources: Vec<PathBuf>,
    /// Exact source content used to compute [`Self::revision`].
    pub content: String,
    /// Stable content revision used for preview/apply conflict checks.
    pub revision: String,
    /// Parsed analysis projection.
    pub config: BasiliskConfig,
}

/// A validated replacement for one active config document.
#[derive(Debug, Clone)]
pub struct ConfigPatch {
    /// Source path to replace.
    pub path: PathBuf,
    /// Revision this patch was built from.
    pub base_revision: String,
    /// Fully validated replacement content.
    pub content: String,
    /// Revision of the replacement content.
    pub revision: String,
    /// Parsed replacement projection.
    pub config: BasiliskConfig,
}

/// Configuration discovery, validation, or patch failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigDocumentError {
    /// Active source could not be read.
    Read {
        /// Source that could not be read.
        path: PathBuf,
        /// Underlying I/O error.
        message: String,
    },
    /// Active source is malformed or contains an invalid severity.
    Invalid {
        /// Malformed source.
        path: PathBuf,
        /// Parse or validation explanation.
        message: String,
    },
    /// Mutation attempted against an outdated revision.
    RevisionConflict {
        /// Revision supplied by the caller.
        expected: String,
        /// Current on-disk revision.
        actual: String,
    },
    /// Active source is not writable.
    ReadOnly {
        /// Source that cannot be edited.
        path: PathBuf,
    },
}

impl std::fmt::Display for ConfigDocumentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { path, message } => write!(f, "failed to read {}: {message}", path.display()),
            Self::Invalid { path, message } => {
                write!(f, "invalid configuration {}: {message}", path.display())
            }
            Self::RevisionConflict { expected, actual } => {
                write!(f, "configuration revision changed ({expected} != {actual})")
            }
            Self::ReadOnly { path } => write!(f, "configuration is read-only: {}", path.display()),
        }
    }
}

impl std::error::Error for ConfigDocumentError {}

/// Discover and validate the one active config source for `root`.
///
/// Existing `basilisk.json` has priority. Otherwise the existing
/// `pyproject.toml` is active, or becomes the creation target when absent.
/// Malformed active sources are errors and never fall through to a shadowed
/// source or defaults.
pub fn discover_config_document(root: &Path) -> Result<ConfigDocument, ConfigDocumentError> {
    let json_path = root.join("basilisk.json");
    let toml_path = root.join("pyproject.toml");
    let (path, format, shadowed_sources) = if json_path.is_file() {
        let shadowed = toml_path.is_file().then_some(toml_path);
        (json_path, ConfigFormat::BasiliskJson, shadowed.into_iter().collect())
    } else {
        (toml_path, ConfigFormat::PyprojectToml, Vec::new())
    };
    let exists = path.is_file();
    let content = if exists {
        std::fs::read_to_string(&path).map_err(|error| ConfigDocumentError::Read {
            path: path.clone(),
            message: error.to_string(),
        })?
    } else {
        String::new()
    };
    let config = validate_content(&path, format, &content)?;
    let read_only = exists
        && std::fs::metadata(&path)
            .map(|metadata| metadata.permissions().readonly())
            .unwrap_or(true);
    Ok(ConfigDocument {
        root: root.to_path_buf(),
        path,
        format,
        exists,
        read_only,
        shadowed_sources,
        revision: content_revision(&content),
        content,
        config,
    })
}

pub(super) fn validate_content(
    path: &Path,
    format: ConfigFormat,
    content: &str,
) -> Result<BasiliskConfig, ConfigDocumentError> {
    if content.is_empty() {
        return Ok(BasiliskConfig::default());
    }
    let config = match format {
        ConfigFormat::BasiliskJson => {
            let value: serde_json::Value = serde_json::from_str(content).map_err(|error| {
                ConfigDocumentError::Invalid { path: path.to_path_buf(), message: error.to_string() }
            })?;
            validate_json_severities(path, &value)?;
            crate::parse::parse_json_content(content)
        }
        ConfigFormat::PyprojectToml => {
            let table: toml::Table = content.parse().map_err(|error: toml::de::Error| {
                ConfigDocumentError::Invalid { path: path.to_path_buf(), message: error.to_string() }
            })?;
            validate_toml_severities(path, &table)?;
            crate::parse::parse_pyproject_content(content)
                .or_else(|| Some(BasiliskConfig::default()))
        }
    };
    config.ok_or_else(|| ConfigDocumentError::Invalid {
        path: path.to_path_buf(),
        message: "configuration root has the wrong shape".to_owned(),
    })
}

fn validate_json_severities(path: &Path, value: &serde_json::Value) -> Result<(), ConfigDocumentError> {
    validate_rule_object(path, value.get("rules"))?;
    let paths = value.get("perPathOverrides").or_else(|| value.get("per-path-overrides"));
    if let Some(entries) = paths.and_then(serde_json::Value::as_object) {
        for entry in entries.values() {
            validate_rule_object(path, entry.get("rules"))?;
        }
    }
    Ok(())
}

fn validate_rule_object(path: &Path, value: Option<&serde_json::Value>) -> Result<(), ConfigDocumentError> {
    let Some(rules) = value.and_then(serde_json::Value::as_object) else { return Ok(()) };
    for (code, severity) in rules {
        let valid = severity.as_str().and_then(crate::RuleSeverity::parse).is_some();
        if !valid {
            return Err(ConfigDocumentError::Invalid {
                path: path.to_path_buf(),
                message: format!("rule `{code}` has an invalid severity"),
            });
        }
    }
    Ok(())
}

fn validate_toml_severities(path: &Path, table: &toml::Table) -> Result<(), ConfigDocumentError> {
    let Some(basilisk) = table.get("tool").and_then(|v| v.get("basilisk")) else { return Ok(()) };
    validate_toml_rule_table(path, basilisk.get("rules"))?;
    if let Some(paths) = basilisk.get("per-path-overrides").and_then(toml::Value::as_table) {
        for entry in paths.values() {
            validate_toml_rule_table(path, entry.get("rules"))?;
        }
    }
    Ok(())
}

fn validate_toml_rule_table(path: &Path, value: Option<&toml::Value>) -> Result<(), ConfigDocumentError> {
    let Some(rules) = value.and_then(toml::Value::as_table) else { return Ok(()) };
    for (code, severity) in rules {
        let valid = severity.as_str().and_then(crate::RuleSeverity::parse).is_some();
        if !valid {
            return Err(ConfigDocumentError::Invalid {
                path: path.to_path_buf(),
                message: format!("rule `{code}` has an invalid severity"),
            });
        }
    }
    Ok(())
}

pub(super) fn content_revision(content: &str) -> String {
    let hash = content.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |value, byte| {
        (value ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    format!("fnv1a64:{hash:016x}")
}
