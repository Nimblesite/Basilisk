//! Structure-preserving rule and tag mutations for active config documents.
//!
//! Implements [CONFIGEDITOR-SOURCES]: the writer validates the original
//! structure, applies plain entry updates, and validates the complete
//! replacement. Empty tables are never pruned — an empty table means
//! `analyze` runs nothing, and pruning it would re-arm the one-time seed
//! ([LSPARCH-CONFIG-SEEDING]).

use std::collections::BTreeMap;

use toml_edit::{value, DocumentMut, Item, Table};

use super::{content_revision, validate_content, ConfigDocument, ConfigDocumentError, ConfigPatch};
use crate::parse::{CACHE_DIR_KEY, CACHE_KEY};
use crate::RuleSeverity;

/// Expanded, validated entry updates. `None` removes the entry.
///
/// The only mutations a config file can express ([CHKARCH-CONFIG-MODEL],
/// `EditorMutation` in `models/configuration_editor.td`): set or remove a
/// per-rule entry, set or remove a tag entry. There are no scopes, presets,
/// or path patterns.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuleConfigUpdate {
    /// `[tool.basilisk.rules]` — code → explicit severity, or `None` to
    /// remove the entry.
    pub rules: BTreeMap<String, Option<RuleSeverity>>,
    /// `[tool.basilisk.rule-tags]` — tag → explicit severity, or `None` to
    /// remove the entry.
    pub rule_tags: BTreeMap<String, Option<RuleSeverity>>,
}

/// Closed persistence allowlist for typeshed settings — the whole runtime
/// surface is these three keys ([STUBRES-TYPESHED-CONFIG]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TypeshedConfigKey {
    /// Custom typeshed folder.
    TypeshedPath,
    /// Exact full commit SHA pin.
    TypeshedCommit,
    /// Verified content-addressed store directory.
    TypeshedStorePath,
}

impl TypeshedConfigKey {
    const fn as_str(self) -> &'static str {
        match self {
            Self::TypeshedPath => "typeshed-path",
            Self::TypeshedCommit => "typeshed-commit",
            Self::TypeshedStorePath => "typeshed-store-path",
        }
    }
}

/// Atomic typeshed setting updates. Every key holds a TOML string (a path or
/// a full SHA); `None` removes the explicit key.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeshedConfigUpdate {
    /// Key to replacement string, or `None` to remove the explicit key.
    pub entries: BTreeMap<TypeshedConfigKey, Option<String>>,
}

/// One persistent-result-cache write ([CHKCACHE-CONFIG]).
///
/// Key and value type are one unit, so `cache = "true"` or `cache-dir = false`
/// cannot be constructed — a caller cannot render a document the parser would
/// then silently ignore. The in-session Salsa memo layer appears nowhere here:
/// it is always on and has no key at all ([CHKARCH-INCREMENTAL-SALSA]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheConfigMutation {
    /// Write `cache = <flag>`.
    SetEnabled(bool),
    /// Remove the explicit `cache` key; the default (off) applies again.
    RemoveEnabled,
    /// Write `cache-dir = "<path>"`.
    SetDir(String),
    /// Remove the explicit `cache-dir` key; the default location applies again.
    RemoveDir,
}

/// Ordered persistent-cache writes. Later entries win over earlier ones for
/// the same key, exactly as a repeated TOML assignment would.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CacheConfigUpdate {
    /// The writes to apply, in request order.
    pub mutations: Vec<CacheConfigMutation>,
}

/// One atomic configuration-editor transaction.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigurationUpdate {
    /// Rule and tag entry updates.
    pub rules: RuleConfigUpdate,
    /// Typeshed acquisition-setting updates.
    pub typeshed: TypeshedConfigUpdate,
    /// Persistent result-cache setting updates ([CHKCACHE-CONFIG]).
    pub cache: CacheConfigUpdate,
}

/// Build and validate a complete replacement without writing it.
///
/// # Errors
///
/// Returns [`ConfigDocumentError`] when the source is read-only, malformed,
/// has a wrong-shaped mutation target, or the rendered replacement is invalid.
pub fn build_rule_patch(
    document: &ConfigDocument,
    update: &RuleConfigUpdate,
) -> Result<ConfigPatch, ConfigDocumentError> {
    build_configuration_patch(
        document,
        &ConfigurationUpdate {
            rules: update.clone(),
            typeshed: TypeshedConfigUpdate::default(),
            cache: CacheConfigUpdate::default(),
        },
    )
}

/// Build one validated replacement containing rule/tag and Typeshed updates.
///
/// # Errors
///
/// Returns [`ConfigDocumentError`] under the same conditions as
/// [`build_rule_patch`].
pub fn build_configuration_patch(
    document: &ConfigDocument,
    update: &ConfigurationUpdate,
) -> Result<ConfigPatch, ConfigDocumentError> {
    if document.read_only {
        return Err(ConfigDocumentError::ReadOnly {
            path: document.path.clone(),
        });
    }
    let content = patch_toml(&document.content, update, &document.path)?;
    // Validate the complete rendered document before exposing a patch.
    let mut config = validate_content(&document.path, &content)?;
    config.project_root = Some(document.root.clone());
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
    update: &ConfigurationUpdate,
    path: &std::path::Path,
) -> Result<String, ConfigDocumentError> {
    let mut document =
        content
            .parse::<DocumentMut>()
            .map_err(|error| ConfigDocumentError::Invalid {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?;
    if !update.rules.rules.is_empty() {
        let rules = nested_table_mut(
            document.as_table_mut(),
            &["tool", "basilisk", "rules"],
            path,
        )?;
        apply_table_updates(rules, &update.rules.rules);
    }
    if !update.rules.rule_tags.is_empty() {
        let tags = nested_table_mut(
            document.as_table_mut(),
            &["tool", "basilisk", "rule-tags"],
            path,
        )?;
        apply_table_updates(tags, &update.rules.rule_tags);
    }
    if !update.typeshed.entries.is_empty() {
        let basilisk = nested_table_mut(document.as_table_mut(), &["tool", "basilisk"], path)?;
        apply_typeshed_updates(basilisk, &update.typeshed.entries);
    }
    if !update.cache.mutations.is_empty() {
        let basilisk = nested_table_mut(document.as_table_mut(), &["tool", "basilisk"], path)?;
        apply_cache_updates(basilisk, &update.cache.mutations);
    }
    let rendered = document.to_string();
    Ok(match newline_style(content) {
        "\r\n" => rendered.replace("\r\n", "\n").replace('\n', "\r\n"),
        _ => rendered,
    })
}

fn apply_typeshed_updates(
    table: &mut Table,
    entries: &BTreeMap<TypeshedConfigKey, Option<String>>,
) {
    for (key, setting) in entries {
        match setting {
            Some(text) => table[key.as_str()] = value(text.as_str()),
            None => {
                let _ = table.remove(key.as_str());
            }
        }
    }
}

/// Apply the persistent-cache writes in request order ([CHKCACHE-CONFIG]).
///
/// `cache` is written as a TOML boolean and `cache-dir` as a TOML string, the
/// only shapes the parser reads, so an applied edit is never a silent no-op.
fn apply_cache_updates(table: &mut Table, mutations: &[CacheConfigMutation]) {
    for mutation in mutations {
        match mutation {
            CacheConfigMutation::SetEnabled(flag) => table[CACHE_KEY] = value(*flag),
            CacheConfigMutation::RemoveEnabled => {
                let _ = table.remove(CACHE_KEY);
            }
            CacheConfigMutation::SetDir(path) => table[CACHE_DIR_KEY] = value(path.as_str()),
            CacheConfigMutation::RemoveDir => {
                let _ = table.remove(CACHE_DIR_KEY);
            }
        }
    }
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
    table
        .entry(key)
        .or_insert_with(|| {
            // Implicit, like the intermediates the parser creates for dotted
            // headers: a table that only exists to hold a deeper mutation
            // target renders no bare `[tool]`-style header of its own.
            // toml_edit still renders it the moment it holds direct entries,
            // and tables already present in the document keep their explicit
            // headers ([CONFIGEDITOR-SOURCES] never prunes).
            let mut created = Table::new();
            created.set_implicit(true);
            Item::Table(created)
        })
        .as_table_mut()
        .ok_or_else(|| ConfigDocumentError::Invalid {
            path: source_path.to_path_buf(),
            message: format!("`{key}` must be a table"),
        })
}

fn apply_table_updates(table: &mut Table, entries: &BTreeMap<String, Option<RuleSeverity>>) {
    for (key, severity) in entries {
        match severity {
            Some(severity) => table[key] = value(severity.as_str()),
            None => {
                let _ = table.remove(key);
            }
        }
    }
}

fn newline_style(content: &str) -> &'static str {
    if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}
