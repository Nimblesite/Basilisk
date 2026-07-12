//! Validated, revisioned configuration documents and structure-aware patches.
//!
//! Implements [CONFIGEDITOR-SOURCES]. This is the reusable persistence domain;
//! LSP and CLI callers provide rule intent but never parse or edit config text.

mod adoption;
mod patch;
mod write;

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "test fixtures use direct assertions for compact failure output"
)]
mod tests;

use std::path::{Path, PathBuf};

use crate::BasiliskConfig;

pub use adoption::adoption_rule_overrides;
pub use patch::{build_rule_patch, RuleConfigScope, RuleConfigUpdate};
pub use write::apply_config_patch;

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
            Self::Read { path, message } => {
                write!(f, "failed to read {}: {message}", path.display())
            }
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
///
/// # Errors
///
/// Returns [`ConfigDocumentError`] when the active source cannot be read or
/// parsed, or when its configuration structure is invalid.
pub fn discover_config_document(root: &Path) -> Result<ConfigDocument, ConfigDocumentError> {
    let source = active_config_source(root);
    let content = if source.path.is_file() {
        std::fs::read_to_string(&source.path).map_err(|error| ConfigDocumentError::Read {
            path: source.path.clone(),
            message: error.to_string(),
        })?
    } else {
        String::new()
    };
    build_config_document(root, source, content)
}

/// Discover and validate the active configuration using editor-held content.
///
/// Source selection still follows the normal one-file precedence rules; only
/// the bytes are supplied by the caller. This lets an LSP treat an open dirty
/// buffer as authoritative, including when the on-disk source is malformed.
///
/// # Errors
///
/// Returns [`ConfigDocumentError`] when `content` is not a valid document for
/// the active source format.
pub fn discover_config_document_with_content(
    root: &Path,
    content: String,
) -> Result<ConfigDocument, ConfigDocumentError> {
    build_config_document(root, active_config_source(root), content)
}

/// Return the path selected by the one-active-configuration precedence rules.
#[must_use]
pub fn active_config_path(root: &Path) -> PathBuf {
    active_config_source(root).path
}

struct ActiveConfigSource {
    path: PathBuf,
    format: ConfigFormat,
    shadowed_sources: Vec<PathBuf>,
}

fn active_config_source(root: &Path) -> ActiveConfigSource {
    let json_path = root.join("basilisk.json");
    let toml_path = root.join("pyproject.toml");
    let (path, format, shadowed_sources) = if json_path.is_file() {
        let shadowed = toml_path.is_file().then_some(toml_path);
        (
            json_path,
            ConfigFormat::BasiliskJson,
            shadowed.into_iter().collect(),
        )
    } else {
        (toml_path, ConfigFormat::PyprojectToml, Vec::new())
    };
    ActiveConfigSource {
        path,
        format,
        shadowed_sources,
    }
}

fn build_config_document(
    root: &Path,
    source: ActiveConfigSource,
    content: String,
) -> Result<ConfigDocument, ConfigDocumentError> {
    let ActiveConfigSource {
        path,
        format,
        shadowed_sources,
    } = source;
    let exists = path.is_file();
    let mut config = validate_content(&path, format, &content)?;
    config.project_root = Some(root.to_path_buf());
    let read_only = exists
        && std::fs::metadata(&path).map_or(true, |metadata| metadata.permissions().readonly());
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
            let value: serde_json::Value =
                serde_json::from_str(content).map_err(|error| ConfigDocumentError::Invalid {
                    path: path.to_path_buf(),
                    message: error.to_string(),
                })?;
            validate_json_structure(path, &value)?;
            crate::parse::parse_json_content(content)
        }
        ConfigFormat::PyprojectToml => {
            let table: toml::Table =
                content
                    .parse()
                    .map_err(|error: toml::de::Error| ConfigDocumentError::Invalid {
                        path: path.to_path_buf(),
                        message: error.to_string(),
                    })?;
            let has_basilisk = validate_toml_structure(path, &table)?;
            if has_basilisk {
                crate::parse::parse_pyproject_content(content)
            } else {
                Some(BasiliskConfig::default())
            }
        }
    };
    config.ok_or_else(|| ConfigDocumentError::Invalid {
        path: path.to_path_buf(),
        message: "configuration root has the wrong shape".to_owned(),
    })
}

fn validate_json_structure(
    path: &Path,
    value: &serde_json::Value,
) -> Result<(), ConfigDocumentError> {
    let root = require_json_object(path, value, "configuration root")?;
    validate_rule_object(path, root.get("rules"))?;
    if root.contains_key("perPathOverrides") && root.contains_key("per-path-overrides") {
        return invalid(
            path,
            "cannot define both `perPathOverrides` and `per-path-overrides`",
        );
    }
    let paths = root
        .get("perPathOverrides")
        .or_else(|| root.get("per-path-overrides"));
    if let Some(paths) = paths {
        let entries = require_json_object(path, paths, "per-path overrides")?;
        for (pattern, entry) in entries {
            let entry = require_json_object(path, entry, &format!("path override `{pattern}`"))?;
            validate_rule_object(path, entry.get("rules"))?;
            validate_json_disabled(path, entry.get("disabled"), pattern)?;
            if entry
                .get("adoption")
                .is_some_and(|value| !value.is_boolean())
            {
                return invalid(
                    path,
                    &format!("path override `{pattern}` adoption must be a boolean"),
                );
            }
        }
    }
    Ok(())
}

fn validate_json_disabled(
    path: &Path,
    value: Option<&serde_json::Value>,
    pattern: &str,
) -> Result<(), ConfigDocumentError> {
    let Some(value) = value else { return Ok(()) };
    let Some(codes) = value.as_array() else {
        return invalid(
            path,
            &format!("path override `{pattern}` disabled must be an array"),
        );
    };
    if codes.iter().any(|code| !code.is_string()) {
        return invalid(
            path,
            &format!("path override `{pattern}` disabled entries must be strings"),
        );
    }
    Ok(())
}

fn validate_rule_object(
    path: &Path,
    value: Option<&serde_json::Value>,
) -> Result<(), ConfigDocumentError> {
    let Some(value) = value else { return Ok(()) };
    let rules = require_json_object(path, value, "rules")?;
    for (code, severity) in rules {
        let valid = severity
            .as_str()
            .and_then(crate::RuleSeverity::parse)
            .is_some();
        if !valid {
            return Err(ConfigDocumentError::Invalid {
                path: path.to_path_buf(),
                message: format!("rule `{code}` has an invalid severity"),
            });
        }
    }
    Ok(())
}

fn require_json_object<'a>(
    path: &Path,
    value: &'a serde_json::Value,
    name: &str,
) -> Result<&'a serde_json::Map<String, serde_json::Value>, ConfigDocumentError> {
    value
        .as_object()
        .ok_or_else(|| ConfigDocumentError::Invalid {
            path: path.to_path_buf(),
            message: format!("{name} must be an object"),
        })
}

fn validate_toml_structure(path: &Path, table: &toml::Table) -> Result<bool, ConfigDocumentError> {
    let Some(tool_value) = table.get("tool") else {
        return Ok(false);
    };
    let Some(tool) = tool_value.as_table() else {
        return invalid(path, "`tool` must be a table");
    };
    let Some(basilisk_value) = tool.get("basilisk") else {
        return Ok(false);
    };
    let Some(basilisk) = basilisk_value.as_table() else {
        return invalid(path, "`tool.basilisk` must be a table");
    };
    validate_toml_rule_table(path, basilisk.get("rules"))?;
    if let Some(paths_value) = basilisk.get("per-path-overrides") {
        let Some(paths) = paths_value.as_table() else {
            return invalid(path, "`tool.basilisk.per-path-overrides` must be a table");
        };
        for (pattern, entry_value) in paths {
            let Some(entry) = entry_value.as_table() else {
                return invalid(path, &format!("path override `{pattern}` must be a table"));
            };
            validate_toml_rule_table(path, entry.get("rules"))?;
            validate_toml_disabled(path, entry.get("disabled"), pattern)?;
            if entry.get("adoption").is_some_and(|value| !value.is_bool()) {
                return invalid(
                    path,
                    &format!("path override `{pattern}` adoption must be a boolean"),
                );
            }
        }
    }
    Ok(true)
}

fn validate_toml_disabled(
    path: &Path,
    value: Option<&toml::Value>,
    pattern: &str,
) -> Result<(), ConfigDocumentError> {
    let Some(value) = value else { return Ok(()) };
    let Some(codes) = value.as_array() else {
        return invalid(
            path,
            &format!("path override `{pattern}` disabled must be an array"),
        );
    };
    if codes.iter().any(|code| !code.is_str()) {
        return invalid(
            path,
            &format!("path override `{pattern}` disabled entries must be strings"),
        );
    }
    Ok(())
}

fn validate_toml_rule_table(
    path: &Path,
    value: Option<&toml::Value>,
) -> Result<(), ConfigDocumentError> {
    let Some(value) = value else { return Ok(()) };
    let Some(rules) = value.as_table() else {
        return invalid(path, "rules must be a table");
    };
    for (code, severity) in rules {
        let valid = severity
            .as_str()
            .and_then(crate::RuleSeverity::parse)
            .is_some();
        if !valid {
            return Err(ConfigDocumentError::Invalid {
                path: path.to_path_buf(),
                message: format!("rule `{code}` has an invalid severity"),
            });
        }
    }
    Ok(())
}

fn invalid<T>(path: &Path, message: &str) -> Result<T, ConfigDocumentError> {
    Err(ConfigDocumentError::Invalid {
        path: path.to_path_buf(),
        message: message.to_owned(),
    })
}

pub(super) fn content_revision(content: &str) -> String {
    let hash = content
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325_u64, |value, byte| {
            (value ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        });
    format!("fnv1a64:{hash:016x}")
}
