//! Root-aware adoption commands backed by exact-path active-config overrides.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use basilisk_config::{RuleConfigScope, RuleConfigUpdate, RuleSeverity};
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{MessageType, Url};
use tracing::{info, warn};

use super::LspServer;

/// Adopt the current errors in one file as warning-severity exact-path rules.
pub(super) async fn execute_adopt_file(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let Some(uri) = command_uri(args, "adoptFile") else {
        return Ok(None);
    };
    let Ok(path) = uri.to_file_path() else {
        return Ok(None);
    };
    let Some(root) = owning_root(server, &path).await else {
        warn!(uri = %uri, "adoptFile: file is outside every workspace root");
        return Ok(None);
    };
    let Some(codes) = error_codes_for_file(server, &path).await else {
        warn!(uri = %uri, "adoptFile: file not found in workspace index");
        return Ok(Some(serde_json::json!({ "adopted": false, "demoted": 0 })));
    };
    if codes.is_empty() {
        return Ok(Some(serde_json::json!({ "adopted": true, "demoted": 0 })));
    }
    let update = adoption_update(&root, &path, &codes);
    let _ = crate::configuration_editor::apply_rule_updates(server, &root, &[update], "adoptFile")
        .await?;
    let demoted = codes.len();
    info!(uri = %uri, demoted, "adopted file in active configuration");
    server
        .client
        .log_message(
            MessageType::INFO,
            format!("Basilisk: adopted file with {demoted} warning override(s)"),
        )
        .await;
    Ok(Some(
        serde_json::json!({ "adopted": true, "demoted": demoted }),
    ))
}

/// Adopt every indexed file with errors, grouped by its owning workspace root.
pub(super) async fn execute_adopt_workspace(
    server: &LspServer,
    _args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let roots = server.workspace_roots.read().await.clone();
    if roots.is_empty() {
        warn!("adoptWorkspace: no workspace roots available");
        return Ok(None);
    }
    let grouped = collect_workspace_adoptions(server, &roots).await;
    let files = grouped.values().map(Vec::len).sum::<usize>();
    let demoted = grouped
        .values()
        .flatten()
        .map(|update| update.rules.len())
        .sum::<usize>();
    for (root, updates) in grouped {
        let _ = crate::configuration_editor::apply_rule_updates(
            server,
            &root,
            &updates,
            "adoptWorkspace",
        )
        .await?;
    }
    server
        .client
        .log_message(
            MessageType::INFO,
            format!("Basilisk: adopted {files} file(s) with {demoted} warning override(s)"),
        )
        .await;
    Ok(Some(serde_json::json!({
        "adopted": true,
        "files": files,
        "demoted": demoted,
    })))
}

/// Remove the editor-owned exact-path adoption entry for one file.
pub(super) async fn execute_unadopt_file(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let Some(uri) = command_uri(args, "unadoptFile") else {
        return Ok(None);
    };
    let Ok(path) = uri.to_file_path() else {
        return Ok(None);
    };
    let Some(root) = owning_root(server, &path).await else {
        return Ok(None);
    };
    let pattern = relative_pattern(&root, &path);
    let document = crate::configuration_editor::configuration_document(server, &root)?;
    let adoption = basilisk_config::adoption_rule_overrides(&document);
    let Some(rules) = adoption.get(&pattern) else {
        return Ok(Some(serde_json::json!({ "unadopted": false })));
    };
    let update = RuleConfigUpdate {
        scope: RuleConfigScope::Path {
            pattern,
            adoption: false,
        },
        rules: rules.keys().map(|code| (code.clone(), None)).collect(),
    };
    let _ =
        crate::configuration_editor::apply_rule_updates(server, &root, &[update], "unadoptFile")
            .await?;
    info!(uri = %uri, "removed file adoption from active configuration");
    Ok(Some(serde_json::json!({ "unadopted": true })))
}

/// Remove adopted rule overrides that no longer have a diagnostic after save.
pub(super) async fn graduate_fixed_rules(server: &LspServer, uri: &Url) -> LspResult<()> {
    let Ok(path) = uri.to_file_path() else {
        return Ok(());
    };
    let Some(root) = owning_root(server, &path).await else {
        return Ok(());
    };
    let pattern = relative_pattern(&root, &path);
    let document = crate::configuration_editor::configuration_document(server, &root)?;
    let adoption = basilisk_config::adoption_rule_overrides(&document);
    let Some(adopted_rules) = adoption.get(&pattern) else {
        return Ok(());
    };
    let remaining = server
        .with_index(|index| {
            let entry = index.files.get(&path).or_else(|| {
                let canonical = path.canonicalize().ok()?;
                index.files.get(&canonical)
            })?;
            checked_diagnostic_codes(&entry)
        })
        .await;
    let Some(remaining) = remaining else {
        // A parse/resolve failure has no trustworthy checker result. Never
        // interpret that absence as proof that all adopted debt was fixed.
        return Ok(());
    };
    let graduated: BTreeMap<String, Option<RuleSeverity>> = adopted_rules
        .keys()
        .filter(|code| !remaining.contains(*code))
        .map(|code| (code.clone(), None))
        .collect();
    if graduated.is_empty() {
        return Ok(());
    }
    let update = RuleConfigUpdate {
        scope: RuleConfigScope::Path {
            pattern,
            adoption: true,
        },
        rules: graduated,
    };
    let _ =
        crate::configuration_editor::apply_rule_updates(server, &root, &[update], "autoGraduate")
            .await?;
    Ok(())
}

fn checked_diagnostic_codes(entry: &crate::workspace::FileEntry) -> Option<BTreeSet<String>> {
    let _ = entry.resolved.as_ref()?;
    Some(
        entry
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.code.to_owned())
            .collect(),
    )
}

fn command_uri(args: &[serde_json::Value], command: &str) -> Option<Url> {
    let value = args.first().and_then(serde_json::Value::as_str);
    let Some(value) = value else {
        warn!(command, "missing URI argument");
        return None;
    };
    match Url::parse(value) {
        Ok(uri) => Some(uri),
        Err(error) => {
            warn!(command, %error, "invalid URI argument");
            None
        }
    }
}

async fn owning_root(server: &LspServer, path: &Path) -> Option<PathBuf> {
    server
        .workspace_roots
        .read()
        .await
        .iter()
        .filter(|root| path_is_within(path, root))
        .max_by_key(|root| root.components().count())
        .cloned()
}

async fn error_codes_for_file(server: &LspServer, path: &Path) -> Option<BTreeSet<String>> {
    server
        .with_index(|index| {
            let entry = index.files.get(path).or_else(|| {
                let canonical = path.canonicalize().ok()?;
                index.files.get(&canonical)
            })?;
            Some(
                entry
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| {
                        matches!(
                            diagnostic.severity,
                            basilisk_checker::Severity::Error
                                | basilisk_checker::Severity::SafetyViolation
                        )
                    })
                    .map(|diagnostic| diagnostic.code.code.to_owned())
                    .collect(),
            )
        })
        .await
}

async fn collect_workspace_adoptions(
    server: &LspServer,
    roots: &[PathBuf],
) -> BTreeMap<PathBuf, Vec<RuleConfigUpdate>> {
    server
        .with_index(|index| {
            let mut grouped: BTreeMap<PathBuf, Vec<RuleConfigUpdate>> = BTreeMap::new();
            for entry in &index.files {
                let path = entry.key();
                let Some(root) = roots
                    .iter()
                    .filter(|root| path_is_within(path, root))
                    .max_by_key(|root| root.components().count())
                else {
                    continue;
                };
                let codes: BTreeSet<String> = entry
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| {
                        matches!(
                            diagnostic.severity,
                            basilisk_checker::Severity::Error
                                | basilisk_checker::Severity::SafetyViolation
                        )
                    })
                    .map(|diagnostic| diagnostic.code.code.to_owned())
                    .collect();
                if !codes.is_empty() {
                    grouped
                        .entry(root.clone())
                        .or_default()
                        .push(adoption_update(root, path, &codes));
                }
            }
            Some(grouped)
        })
        .await
        .unwrap_or_default()
}

fn adoption_update(root: &Path, path: &Path, codes: &BTreeSet<String>) -> RuleConfigUpdate {
    RuleConfigUpdate {
        scope: RuleConfigScope::Path {
            pattern: relative_pattern(root, path),
            adoption: true,
        },
        rules: codes
            .iter()
            .map(|code| (code.clone(), Some(RuleSeverity::Warning)))
            .collect(),
    }
}

fn relative_pattern(root: &Path, path: &Path) -> String {
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    canonical_path
        .strip_prefix(&canonical_root)
        .unwrap_or(&canonical_path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn path_is_within(path: &Path, root: &Path) -> bool {
    path.starts_with(root)
        || path
            .canonicalize()
            .ok()
            .zip(root.canonicalize().ok())
            .is_some_and(|(path, root)| path.starts_with(root))
}

#[cfg(test)]
mod tests {
    use crate::workspace::FileEntry;

    use super::checked_diagnostic_codes;

    // Implements [CONFIGEDITOR-ADOPTION]: invalid saved source cannot erase
    // adoption debt merely because it produced no checker diagnostics.
    #[test]
    fn unresolved_file_is_not_evidence_for_adoption_graduation() {
        let entry = FileEntry {
            source_hash: 0,
            text: "def broken(:\n".to_owned(),
            resolved: None,
            diagnostics: Vec::new(),
            version: 1,
            is_open: true,
        };
        assert!(checked_diagnostic_codes(&entry).is_none());
    }
}
