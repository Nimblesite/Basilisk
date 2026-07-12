//! Structure-preserving rule mutations for active configuration documents.

use std::collections::BTreeMap;

use toml_edit::{value, DocumentMut, Item, Table};

use super::{content_revision, validate_content, ConfigDocument, ConfigDocumentError, ConfigFormat, ConfigPatch};
use crate::RuleSeverity;

/// Scope for an expanded rule update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleConfigScope {
    /// `[tool.basilisk.rules]` / JSON `rules`.
    Project,
    /// One path-pattern entry. Exact file paths use this same representation.
    Path {
        /// Relative path or glob pattern.
        pattern: String,
        /// Mark an exact-file entry as editor-generated adoption debt.
        adoption: bool,
    },
}

/// Expanded, validated rule updates. `None` removes the explicit override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleConfigUpdate {
    /// Mutation scope.
    pub scope: RuleConfigScope,
    /// Stable code → explicit severity or inherited/reset.
    pub rules: BTreeMap<String, Option<RuleSeverity>>,
}

/// Build and validate a complete replacement without writing it.
pub fn build_rule_patch(
    document: &ConfigDocument,
    updates: &[RuleConfigUpdate],
) -> Result<ConfigPatch, ConfigDocumentError> {
    if document.read_only {
        return Err(ConfigDocumentError::ReadOnly { path: document.path.clone() });
    }
    let content = match document.format {
        ConfigFormat::PyprojectToml => patch_toml(&document.content, updates, &document.path)?,
        ConfigFormat::BasiliskJson => patch_json(&document.content, updates, &document.path)?,
    };
    // Validate the complete rendered document before exposing a patch. The
    // current disk projection is intentionally not used as the replacement
    // projection; apply the same expanded operations to a clone instead.
    let config = validate_content(&document.path, document.format, &content)?;
    Ok(ConfigPatch {
        path: document.path.clone(),
        base_revision: document.revision.clone(),
        revision: content_revision(&content),
        content,
        config,
    })
}

fn patch_toml(
    content: &str,
    updates: &[RuleConfigUpdate],
    path: &std::path::Path,
) -> Result<String, ConfigDocumentError> {
    let mut document = content.parse::<DocumentMut>().map_err(|error| ConfigDocumentError::Invalid {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    for update in updates {
        match &update.scope {
            RuleConfigScope::Project => {
                let rules = nested_table_mut(
                    document.as_table_mut(),
                    &["tool", "basilisk", "rules"],
                    path,
                )?;
                apply_table_updates(rules, &update.rules);
            }
            RuleConfigScope::Path { pattern, adoption } => {
                let paths = nested_table_mut(
                    document.as_table_mut(),
                    &["tool", "basilisk", "per-path-overrides"],
                    path,
                )?;
                let entry = child_table_mut(paths, pattern, path)?;
                if *adoption {
                    entry["adoption"] = value(true);
                }
                let rules = child_table_mut(entry, "rules", path)?;
                apply_table_updates(rules, &update.rules);
                if rules.is_empty() {
                    let _ = entry.remove("rules");
                    if *adoption {
                        let _ = entry.remove("adoption");
                    }
                }
                if entry.is_empty() {
                    let _ = paths.remove(pattern);
                }
            }
        }
    }
    let rendered = document.to_string();
    Ok(match newline_style(content) {
        "\r\n" => rendered.replace("\r\n", "\n").replace('\n', "\r\n"),
        _ => rendered,
    })
}

fn nested_table_mut<'a>(
    mut root: &'a mut Table,
    keys: &[&str],
    source_path: &std::path::Path,
) -> Result<&'a mut Table, ConfigDocumentError> {
    for key in keys {
        root = child_table_mut(root, key, source_path)?;
    }
    Ok(root)
}

fn child_table_mut<'a>(
    table: &'a mut Table,
    key: &str,
    source_path: &std::path::Path,
) -> Result<&'a mut Table, ConfigDocumentError> {
    let item = table
        .entry(key)
        .or_insert_with(|| Item::Table(Table::new()));
    if !item.is_table() {
        return Err(ConfigDocumentError::Invalid {
            path: source_path.to_path_buf(),
            message: format!("`{key}` must be a table"),
        });
    }
    item.as_table_mut().ok_or_else(|| ConfigDocumentError::Invalid {
        path: source_path.to_path_buf(),
        message: format!("`{key}` must be a table"),
    })
}

fn apply_table_updates(table: &mut Table, rules: &BTreeMap<String, Option<RuleSeverity>>) {
    for (code, severity) in rules {
        match severity {
            Some(severity) => table[code] = value(severity.as_str()),
            None => {
                let _ = table.remove(code);
            }
        }
    }
}

fn newline_style(content: &str) -> &'static str {
    if content.contains("\r\n") { "\r\n" } else { "\n" }
}

fn patch_json(
    content: &str,
    updates: &[RuleConfigUpdate],
    path: &std::path::Path,
) -> Result<String, ConfigDocumentError> {
    let mut root: serde_json::Value = if content.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(content).map_err(|error| ConfigDocumentError::Invalid {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?
    };
    let object = root.as_object_mut().ok_or_else(|| ConfigDocumentError::Invalid {
        path: path.to_path_buf(),
        message: "JSON configuration must be an object".to_owned(),
    })?;
    for update in updates {
        let rules = match &update.scope {
            RuleConfigScope::Project => object_entry_object(object, "rules", path)?,
            RuleConfigScope::Path { pattern, adoption } => {
                let paths = object_entry_object(object, "perPathOverrides", path)?;
                let entry = object_entry_object(paths, pattern, path)?;
                if *adoption {
                    let _ = entry.insert("adoption".to_owned(), serde_json::Value::Bool(true));
                }
                object_entry_object(entry, "rules", path)?
            }
        };
        for (code, severity) in &update.rules {
            match severity {
                Some(severity) => {
                    let _ = rules.insert(code.clone(), serde_json::json!(severity.as_str()));
                }
                None => {
                    let _ = rules.remove(code);
                }
            }
        }
    }
    serde_json::to_string_pretty(&root)
        .map(|mut text| { text.push('\n'); text })
        .map_err(|error| ConfigDocumentError::Invalid {
            path: path.to_path_buf(),
            message: error.to_string(),
        })
}

fn object_entry_object<'a>(
    object: &'a mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    path: &std::path::Path,
) -> Result<&'a mut serde_json::Map<String, serde_json::Value>, ConfigDocumentError> {
    object
        .entry(key.to_owned())
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .ok_or_else(|| ConfigDocumentError::Invalid {
            path: path.to_path_buf(),
            message: format!("`{key}` must be an object"),
        })
}
