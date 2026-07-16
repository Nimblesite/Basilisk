//! Validated, revisioned configuration documents and structure-aware patches.
//!
//! Implements [CONFIGEDITOR-SOURCES]. This is the reusable persistence domain;
//! LSP and CLI callers provide rule/tag intent but never parse or edit config
//! text themselves.

mod patch;
mod write;

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test fixtures use direct assertions for compact failure output"
)]
mod tests;

use std::path::{Path, PathBuf};

use crate::BasiliskConfig;

pub use patch::{build_rule_patch, RuleConfigUpdate};
pub use write::apply_config_patch;

/// A validated active configuration source with optimistic-lock revision.
///
/// The active source is always the root's `pyproject.toml` `[tool.basilisk]`
/// — the only configuration format ([CHKARCH-CONFIG-FILE]). A legacy
/// `basilisk.json` is never read or written.
#[derive(Debug, Clone)]
pub struct ConfigDocument {
    /// Workspace root this source belongs to.
    pub root: PathBuf,
    /// Active source path (existing or the default creation target).
    pub path: PathBuf,
    /// Whether the active target already exists.
    pub exists: bool,
    /// Whether filesystem metadata marks the source read-only.
    pub read_only: bool,
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
/// The active source is always the root's `pyproject.toml` — existing, or the
/// creation target when absent ([LSPARCH-CONFIG-SEEDING]). Malformed active
/// sources are errors and never fall through to defaults.
///
/// # Errors
///
/// Returns [`ConfigDocumentError`] when the active source cannot be read or
/// parsed, or when its configuration structure is invalid.
pub fn discover_config_document(root: &Path) -> Result<ConfigDocument, ConfigDocumentError> {
    let path = active_config_path(root);
    let content = if path.is_file() {
        std::fs::read_to_string(&path).map_err(|error| ConfigDocumentError::Read {
            path: path.clone(),
            message: error.to_string(),
        })?
    } else {
        String::new()
    };
    build_config_document(root, path, content)
}

/// Discover and validate the active configuration using editor-held content.
///
/// Source selection still follows the normal one-file precedence rules; only
/// the bytes are supplied by the caller. This lets an LSP treat an open dirty
/// buffer as authoritative, including when the on-disk source is malformed.
///
/// # Errors
///
/// Returns [`ConfigDocumentError`] when `content` is not a valid document.
pub fn discover_config_document_with_content(
    root: &Path,
    content: String,
) -> Result<ConfigDocument, ConfigDocumentError> {
    build_config_document(root, active_config_path(root), content)
}

/// Return the path selected by the one-active-configuration rule: the root's
/// `pyproject.toml`.
#[must_use]
pub fn active_config_path(root: &Path) -> PathBuf {
    root.join("pyproject.toml")
}

fn build_config_document(
    root: &Path,
    path: PathBuf,
    content: String,
) -> Result<ConfigDocument, ConfigDocumentError> {
    let exists = path.is_file();
    let mut config = validate_content(&path, &content)?;
    config.project_root = Some(root.to_path_buf());
    let read_only = exists
        && std::fs::metadata(&path).map_or(true, |metadata| metadata.permissions().readonly());
    Ok(ConfigDocument {
        root: root.to_path_buf(),
        path,
        exists,
        read_only,
        revision: content_revision(&content),
        content,
        config,
    })
}

pub(super) fn validate_content(
    path: &Path,
    content: &str,
) -> Result<BasiliskConfig, ConfigDocumentError> {
    if content.is_empty() {
        return Ok(BasiliskConfig::default());
    }
    let table: toml::Table =
        content
            .parse()
            .map_err(|error: toml::de::Error| ConfigDocumentError::Invalid {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?;
    let has_basilisk = validate_toml_structure(path, &table)?;
    let config = if has_basilisk {
        crate::parse::parse_pyproject_content(content)
    } else {
        Some(BasiliskConfig::default())
    };
    config.ok_or_else(|| ConfigDocumentError::Invalid {
        path: path.to_path_buf(),
        message: "configuration root has the wrong shape".to_owned(),
    })
}

/// Validate the `[tool.basilisk]` shape: `rules` and `rule-tags` must be
/// `"<key>" = "<severity>"` tables ([CHKARCH-CONFIG-MODEL]).
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
    validate_toml_severity_table(path, basilisk.get("rules"), "rules")?;
    validate_toml_severity_table(path, basilisk.get("rule-tags"), "rule-tags")?;
    Ok(true)
}

fn validate_toml_severity_table(
    path: &Path,
    value: Option<&toml::Value>,
    table_name: &str,
) -> Result<(), ConfigDocumentError> {
    let Some(value) = value else { return Ok(()) };
    let Some(entries) = value.as_table() else {
        return invalid(path, &format!("`{table_name}` must be a table"));
    };
    for (key, severity) in entries {
        let valid = severity
            .as_str()
            .and_then(crate::RuleSeverity::parse)
            .is_some();
        if !valid {
            return Err(ConfigDocumentError::Invalid {
                path: path.to_path_buf(),
                message: format!("`{table_name}` entry `{key}` has an invalid severity"),
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
