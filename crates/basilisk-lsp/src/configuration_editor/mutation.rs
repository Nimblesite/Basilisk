//! Mutation validation and effective-severity change projection.
//!
//! Implements [CONFIGEDITOR-OPERATIONS]: a mutation is `SetRule`,
//! `RemoveRule`, `SetTag`, or `RemoveTag` — nothing else. Requesting
//! `disabled` for a `pep`-tagged rule (directly, or via a tag entry that
//! would resolve one to `disabled`) is a request error
//! ([CHKARCH-CONFIG-MODEL]).

use std::collections::HashSet;

use basilisk_config::{
    BasiliskConfig, CacheConfigMutation, ConfigDocument, ConfigurationUpdate, TypeshedConfigKey,
};
use tower_lsp::jsonrpc::{Error, Result as LspResult};

use super::catalog::{descriptors, effective_severity, wire_to_config, SelectionError};
use super::model::{
    CacheSettingChange, CacheSettingKey, ConfigurationImpact, EditorMutation, ResolvedRuleChange,
    RuleDescriptor, RuleSeverity, TypeshedSettingChange, TypeshedSettingKey,
};
use super::protocol::{path_uri, rpc_error, rpc_error_data};
use super::snapshot::{count_i64, Inventory};

/// Reject active configuration whose rule entries name unknown rules.
pub(super) fn validate_document_rules(document: &ConfigDocument) -> LspResult<()> {
    let catalog = descriptors();
    let known: HashSet<&str> = catalog.iter().map(|rule| rule.code.as_str()).collect();
    let unknown = document
        .config
        .nearest_tables()
        .into_iter()
        .flat_map(|tables| tables.rules.keys())
        .find(|code| !known.contains(code.as_str()));
    if let Some(code) = unknown {
        Err(rpc_error_data(
            "unknownRule",
            "active configuration contains an unknown rule",
            serde_json::json!({ "rule": code, "sourceUri": path_uri(&document.path) }),
        ))
    } else {
        Ok(())
    }
}

/// Fold the requested mutations into one validated entry update.
///
/// Implements [CONFIGEDITOR-OPERATIONS] and `EditorMutation` in
/// `models/configuration_editor.td`: unknown codes and tags are request
/// errors, and an explicit `SetRule(disabled)` on a `pep` rule fails before
/// any patch is rendered.
pub(super) fn build_update(
    mutations: &[EditorMutation],
    catalog: &[RuleDescriptor],
) -> LspResult<ConfigurationUpdate> {
    let known_codes: HashSet<&str> = catalog.iter().map(|rule| rule.code.as_str()).collect();
    let known_tags: HashSet<&str> = catalog
        .iter()
        .flat_map(|rule| rule.tags.iter().map(String::as_str))
        .collect();
    let mut update = ConfigurationUpdate::default();
    for mutation in mutations {
        match mutation {
            EditorMutation::SetRule { code, severity } => {
                require_known_rule(&known_codes, code)?;
                if *severity == RuleSeverity::Disabled && basilisk_checker::is_pep_rule(code) {
                    return Err(pep_disable_error(std::slice::from_ref(code)));
                }
                let _ = update
                    .rules
                    .rules
                    .insert(code.clone(), Some(wire_to_config(*severity)));
            }
            EditorMutation::RemoveRule { code } => {
                require_known_rule(&known_codes, code)?;
                let _ = update.rules.rules.insert(code.clone(), None);
            }
            EditorMutation::SetTag { tag, severity } => {
                require_known_tag(&known_tags, tag)?;
                let _ = update
                    .rules
                    .rule_tags
                    .insert(tag.clone(), Some(wire_to_config(*severity)));
            }
            EditorMutation::RemoveTag { tag } => {
                require_known_tag(&known_tags, tag)?;
                let _ = update.rules.rule_tags.insert(tag.clone(), None);
            }
            EditorMutation::SetTypeshedSetting { key, value } => {
                let persisted = validate_typeshed_value(*key, value)?;
                let _ = update
                    .typeshed
                    .entries
                    .insert(typeshed_config_key(*key), Some(persisted));
            }
            EditorMutation::RemoveTypeshedSetting { key } => {
                let _ = update
                    .typeshed
                    .entries
                    .insert(typeshed_config_key(*key), None);
            }
            EditorMutation::SetCacheSetting { key, value } => {
                update.cache.mutations.push(cache_set(*key, value)?);
            }
            EditorMutation::RemoveCacheSetting { key } => {
                update.cache.mutations.push(match key {
                    CacheSettingKey::CacheEnabled => CacheConfigMutation::RemoveEnabled,
                    CacheSettingKey::CacheDir => CacheConfigMutation::RemoveDir,
                });
            }
        }
    }
    Ok(update)
}

/// Validate one cache-setting write ([LSPCFGED-CACHE], [CHKCACHE-CONFIG]).
///
/// The wire carries every setting as text so the mutation union stays one
/// shape, but `cache` is a TOML boolean: only the two canonical spellings are
/// accepted here, so a value the parser would silently drop never reaches the
/// document. `cache-dir` must name something.
fn cache_set(key: CacheSettingKey, value: &str) -> LspResult<CacheConfigMutation> {
    match key {
        CacheSettingKey::CacheEnabled => match value {
            "true" => Ok(CacheConfigMutation::SetEnabled(true)),
            "false" => Ok(CacheConfigMutation::SetEnabled(false)),
            _ => Err(invalid_cache_setting(
                key,
                "cache must be exactly \"true\" or \"false\"",
            )),
        },
        CacheSettingKey::CacheDir if !value.trim().is_empty() => {
            Ok(CacheConfigMutation::SetDir(value.to_owned()))
        }
        CacheSettingKey::CacheDir => Err(invalid_cache_setting(
            key,
            "cache-dir requires a non-empty path",
        )),
    }
}

fn invalid_cache_setting(key: CacheSettingKey, message: &str) -> Error {
    rpc_error_data(
        "invalidCacheSetting",
        message,
        serde_json::json!({ "key": format!("{key:?}") }),
    )
}

/// Project exact persisted cache-setting changes into the preview
/// ([LSPCFGED-CACHE]). Values are the rendered TOML text, so a boolean flip
/// reads `false → true` rather than as an opaque key name.
pub(super) fn resolved_cache_changes(
    before: &BasiliskConfig,
    after: &BasiliskConfig,
) -> Vec<CacheSettingChange> {
    [CacheSettingKey::CacheEnabled, CacheSettingKey::CacheDir]
        .into_iter()
        .filter_map(|key| {
            let previous = config_cache_value(before, key);
            let next = config_cache_value(after, key);
            (previous != next).then_some(CacheSettingChange {
                key,
                before: previous,
                after: next,
            })
        })
        .collect()
}

fn config_cache_value(config: &BasiliskConfig, key: CacheSettingKey) -> Option<String> {
    match key {
        CacheSettingKey::CacheEnabled => config.cache_enabled.map(|flag| flag.to_string()),
        CacheSettingKey::CacheDir => config
            .cache_dir
            .as_ref()
            .map(|dir| dir.to_string_lossy().into_owned()),
    }
}

fn typeshed_config_key(key: TypeshedSettingKey) -> TypeshedConfigKey {
    match key {
        TypeshedSettingKey::TypeshedPath => TypeshedConfigKey::TypeshedPath,
        TypeshedSettingKey::TypeshedCommit => TypeshedConfigKey::TypeshedCommit,
        TypeshedSettingKey::TypeshedPackage => TypeshedConfigKey::TypeshedPackage,
        TypeshedSettingKey::TypeshedStorePath => TypeshedConfigKey::TypeshedStorePath,
    }
}

fn validate_typeshed_value(key: TypeshedSettingKey, value: &str) -> LspResult<String> {
    match key {
        TypeshedSettingKey::TypeshedCommit if basilisk_config::is_full_commit_sha(value) => {
            Ok(value.to_owned())
        }
        TypeshedSettingKey::TypeshedCommit => Err(invalid_typeshed_setting(
            key,
            "typeshed-commit must be a full 40-character hexadecimal SHA",
        )),
        TypeshedSettingKey::TypeshedPackage
            if crate::config::parse_typeshed_package(value).is_ok() =>
        {
            Ok(value.to_owned())
        }
        TypeshedSettingKey::TypeshedPackage => Err(invalid_typeshed_setting(
            key,
            "typeshed-package must be of the form `name@sha256:<64-hex>`",
        )),
        TypeshedSettingKey::TypeshedPath | TypeshedSettingKey::TypeshedStorePath
            if !value.trim().is_empty() =>
        {
            Ok(value.to_owned())
        }
        TypeshedSettingKey::TypeshedPath | TypeshedSettingKey::TypeshedStorePath => Err(
            invalid_typeshed_setting(key, "Typeshed setting requires a non-empty path"),
        ),
    }
}

fn invalid_typeshed_setting(key: TypeshedSettingKey, message: &str) -> Error {
    rpc_error_data(
        "invalidTypeshedSetting",
        message,
        serde_json::json!({ "key": format!("{key:?}") }),
    )
}

/// Reject impossible source combinations and malformed custom trees after all
/// mutations have been folded into the hypothetical configuration.
pub(super) fn require_valid_typeshed_configuration(config: &BasiliskConfig) -> LspResult<()> {
    // The three source selectors are mutually exclusive
    // ([STUBRES-TYPESHED-CONFIG]).
    let active = [
        config.typeshed_path.is_some(),
        config.typeshed_commit.is_some(),
        config.typeshed_package.is_some(),
    ]
    .iter()
    .filter(|&&set| set)
    .count();
    if active > 1 {
        return Err(rpc_error(
            "invalidTypeshedSetting",
            "typeshed-path, typeshed-commit, and typeshed-package are mutually exclusive",
        ));
    }
    let Some(path) = config.typeshed_path.as_ref() else {
        return Ok(());
    };
    let resolved = if path.is_absolute() {
        path.clone()
    } else {
        config
            .project_root
            .as_ref()
            .map_or_else(|| path.clone(), |root| root.join(path))
    };
    if resolved.join("stdlib").is_dir() {
        Ok(())
    } else {
        Err(rpc_error_data(
            "invalidTypeshedSetting",
            "typeshed-path must name a directory containing top-level stdlib/",
            serde_json::json!({ "path": resolved }),
        ))
    }
}

/// Reject a hypothetical configuration that resolves any `pep` rule to
/// `disabled` ([CHKARCH-CONFIG-MODEL]) — by rule entry or tag entry.
pub(super) fn require_no_pep_disable(config: &BasiliskConfig) -> LspResult<()> {
    let violations = basilisk_checker::pep_disable_violations(config);
    if violations.is_empty() {
        Ok(())
    } else {
        Err(pep_disable_error(&violations))
    }
}

fn pep_disable_error<S: AsRef<str>>(codes: &[S]) -> Error {
    rpc_error_data(
        "pepRuleDisable",
        "pep rules are graded, never disabled",
        serde_json::json!({
            "rules": codes.iter().map(AsRef::as_ref).collect::<Vec<_>>(),
        }),
    )
}

fn require_known_rule(known: &HashSet<&str>, code: &str) -> LspResult<()> {
    if known.contains(code) {
        Ok(())
    } else {
        Err(selection_error(SelectionError::UnknownRule(
            code.to_owned(),
        )))
    }
}

fn require_known_tag(known: &HashSet<&str>, tag: &str) -> LspResult<()> {
    if known.contains(tag) {
        Ok(())
    } else {
        Err(selection_error(SelectionError::UnknownTag(tag.to_owned())))
    }
}

/// Project the fully resolved per-rule effective-severity changes.
///
/// Implements [CONFIGEDITOR-MODEL]: a preview reports what actually changes
/// after resolution — rules whose effective severity is identical on both
/// sides are omitted.
pub(super) fn resolved_changes(
    catalog: &[RuleDescriptor],
    before: &BasiliskConfig,
    after: &BasiliskConfig,
) -> Vec<ResolvedRuleChange> {
    catalog
        .iter()
        .filter_map(|descriptor| {
            let previous = effective_severity(descriptor, before);
            let next = effective_severity(descriptor, after);
            (previous != next).then(|| ResolvedRuleChange {
                code: descriptor.code.clone(),
                before: previous,
                after: next,
            })
        })
        .collect()
}

fn config_typeshed_value(config: &BasiliskConfig, key: TypeshedSettingKey) -> Option<String> {
    match key {
        TypeshedSettingKey::TypeshedPath => config
            .typeshed_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
        TypeshedSettingKey::TypeshedCommit => config.typeshed_commit.clone(),
        TypeshedSettingKey::TypeshedPackage => config.typeshed_package.clone(),
        TypeshedSettingKey::TypeshedStorePath => config
            .typeshed_store_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
    }
}

/// Project exact persisted Typeshed setting changes into the preview.
pub(super) fn resolved_typeshed_changes(
    before: &BasiliskConfig,
    after: &BasiliskConfig,
) -> Vec<TypeshedSettingChange> {
    [
        TypeshedSettingKey::TypeshedPath,
        TypeshedSettingKey::TypeshedCommit,
        TypeshedSettingKey::TypeshedPackage,
        TypeshedSettingKey::TypeshedStorePath,
    ]
    .into_iter()
    .filter_map(|key| {
        let previous = config_typeshed_value(before, key);
        let next = config_typeshed_value(after, key);
        (previous != next).then_some(TypeshedSettingChange {
            key,
            before: previous,
            after: next,
        })
    })
    .collect()
}

/// Fold both inventories into the complete before/after impact partition.
pub(super) fn build_impact(before: &Inventory, after: &Inventory) -> ConfigurationImpact {
    ConfigurationImpact {
        errors_before: count_i64(before.errors),
        errors_after: count_i64(after.errors),
        warnings_before: count_i64(before.warnings),
        warnings_after: count_i64(after.warnings),
        infos_before: count_i64(before.infos),
        infos_after: count_i64(after.infos),
    }
}

pub(super) fn require_revision(document: &ConfigDocument, expected: &str) -> LspResult<()> {
    if document.revision == expected {
        Ok(())
    } else {
        Err(rpc_error_data(
            "revisionConflict",
            "configuration changed; refresh and preview again",
            serde_json::json!({ "expected": expected, "actual": document.revision }),
        ))
    }
}

pub(super) fn selection_error(error: SelectionError) -> Error {
    match error {
        SelectionError::UnknownRule(rule) => rpc_error_data(
            "unknownRule",
            "request names an unknown rule",
            serde_json::json!({ "rule": rule }),
        ),
        SelectionError::UnknownTag(tag) => rpc_error_data(
            "unknownTag",
            "request names an unknown tag",
            serde_json::json!({ "tag": tag }),
        ),
    }
}

/// Reject an empty mutation list before any state is touched.
pub(super) fn require_mutations(mutations: &[EditorMutation]) -> LspResult<()> {
    if mutations.is_empty() {
        Err(rpc_error(
            "invalidMutation",
            "configuration preview requires at least one mutation",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "mutation_tests.rs"]
mod tests;
