//! Root-aware adoption commands backed by folder-level plain rule entries.
//!
//! Implements [AUTOFIX-ADOPTION] / [AUTOFIX-ADOPTION-FLOW]: adoption records
//! current error debt as ordinary warning-severity rule entries in the root's
//! active configuration — plain code → severity entries in the one
//! configuration model ([CHKARCH-CONFIG-MODEL]), with no exact-file
//! overrides, ownership markers, or sidecar state. There is no post-save
//! graduation; re-running adoption is the explicit, reviewable way to
//! tighten ([AUTOFIX-ADOPTION-RULES]).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use basilisk_config::{RuleConfigUpdate, RuleSeverity};
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{MessageType, Url};
use tracing::{info, warn};

use super::LspServer;

/// Adopt the current errors in one file as warning-severity rule entries in
/// the owning root's configuration.
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
    let update = adoption_update(&codes);
    let _ = crate::configuration_editor::apply_rule_updates(server, &root, &update, "adoptFile")
        .await?;
    let demoted = codes.len();
    info!(uri = %uri, demoted, "adopted file debt into active configuration");
    server
        .client
        .log_message(
            MessageType::INFO,
            format!("Basilisk: adopted file with {demoted} warning entr(ies)"),
        )
        .await;
    Ok(Some(
        serde_json::json!({ "adopted": true, "demoted": demoted }),
    ))
}

/// Adopt every indexed file's error debt, grouped by its owning workspace
/// root — one folder-level update per root.
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
    let files = grouped
        .values()
        .map(|(file_count, _)| file_count)
        .sum::<usize>();
    let demoted = grouped
        .values()
        .map(|(_, update)| update.rules.len())
        .sum::<usize>();
    for (root, (_, update)) in grouped {
        let _ = crate::configuration_editor::apply_rule_updates(
            server,
            &root,
            &update,
            "adoptWorkspace",
        )
        .await?;
    }
    info!(files, demoted, "adopted workspace debt");
    server
        .client
        .log_message(
            MessageType::INFO,
            format!("Basilisk: adopted {files} file(s) with {demoted} warning entr(ies)"),
        )
        .await;
    Ok(Some(serde_json::json!({
        "adopted": true,
        "files": files,
        "demoted": demoted,
    })))
}

/// Remove the folder-level warning entries covering one file's current
/// diagnostics, restoring the surrounding severity ([AUTOFIX-ADOPTION-FLOW]).
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
    let Some(codes) = diagnostic_codes_for_file(server, &path).await else {
        return Ok(Some(serde_json::json!({ "unadopted": false })));
    };
    let document = crate::configuration_editor::configuration_document(server, &root)?;
    let adopted: BTreeMap<String, Option<RuleSeverity>> = document
        .config
        .nearest_tables()
        .map(|tables| {
            codes
                .iter()
                .filter(|code| {
                    tables.rules.get(*code).copied() == Some(RuleSeverity::Warning)
                })
                .map(|code| (code.clone(), None))
                .collect()
        })
        .unwrap_or_default();
    if adopted.is_empty() {
        return Ok(Some(serde_json::json!({ "unadopted": false })));
    }
    let update = RuleConfigUpdate {
        rules: adopted,
        rule_tags: BTreeMap::new(),
    };
    let _ =
        crate::configuration_editor::apply_rule_updates(server, &root, &update, "unadoptFile")
            .await?;
    info!(uri = %uri, "removed adoption entries from active configuration");
    Ok(Some(serde_json::json!({ "unadopted": true })))
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
            Some(error_codes(&entry.diagnostics))
        })
        .await
}

async fn diagnostic_codes_for_file(server: &LspServer, path: &Path) -> Option<BTreeSet<String>> {
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
                    .map(|diagnostic| diagnostic.code.code.to_owned())
                    .collect(),
            )
        })
        .await
}

fn error_codes(diagnostics: &[basilisk_checker::Diagnostic]) -> BTreeSet<String> {
    diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(
                diagnostic.severity,
                basilisk_checker::Severity::Error | basilisk_checker::Severity::SafetyViolation
            )
        })
        .map(|diagnostic| diagnostic.code.code.to_owned())
        .collect()
}

async fn collect_workspace_adoptions(
    server: &LspServer,
    roots: &[PathBuf],
) -> BTreeMap<PathBuf, (usize, RuleConfigUpdate)> {
    server
        .with_index(|index| {
            let mut grouped: BTreeMap<PathBuf, (usize, RuleConfigUpdate)> = BTreeMap::new();
            for entry in &index.files {
                let path = entry.key();
                let Some(root) = roots
                    .iter()
                    .filter(|root| path_is_within(path, root))
                    .max_by_key(|root| root.components().count())
                else {
                    continue;
                };
                let codes = error_codes(&entry.diagnostics);
                if codes.is_empty() {
                    continue;
                }
                let target = grouped.entry(root.clone()).or_default();
                target.0 += 1;
                for (code, severity) in adoption_update(&codes).rules {
                    let _ = target.1.rules.insert(code, severity);
                }
            }
            Some(grouped)
        })
        .await
        .unwrap_or_default()
}

/// Demote every firing error code to a plain warning-severity rule entry —
/// one visible line of configuration per code ([AUTOFIX-ADOPTION]).
fn adoption_update(codes: &BTreeSet<String>) -> RuleConfigUpdate {
    RuleConfigUpdate {
        rules: codes
            .iter()
            .map(|code| (code.clone(), Some(RuleSeverity::Warning)))
            .collect(),
        rule_tags: BTreeMap::new(),
    }
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
    use std::collections::BTreeSet;
    use std::path::Path;

    use basilisk_config::RuleSeverity;

    use super::{adoption_update, command_uri, path_is_within};

    // Implements [AUTOFIX-ADOPTION-FLOW]: adoption commands take exactly one
    // file URI; anything else is logged and refused, never guessed.
    #[test]
    fn command_uri_accepts_only_a_leading_string_uri() {
        assert_eq!(command_uri(&[], "adoptFile"), None);
        assert_eq!(command_uri(&[serde_json::json!(42)], "adoptFile"), None);
        assert_eq!(
            command_uri(&[serde_json::json!("not a uri")], "adoptFile"),
            None
        );
        let parsed = command_uri(
            &[serde_json::json!("file:///workspace/app.py")],
            "adoptFile",
        );
        assert_eq!(
            parsed.map(|uri| uri.to_string()),
            Some("file:///workspace/app.py".to_owned())
        );
    }

    // Implements [AUTOFIX-ADOPTION]: adoption writes plain folder-level
    // warning entries — one per demoted rule code, no scopes, no markers.
    #[test]
    fn adoption_update_demotes_every_code_to_a_plain_warning_entry() {
        let codes: BTreeSet<String> = [
            "BSK-0001".to_owned(),
            "assignment_compatibility".to_owned(),
        ]
        .into_iter()
        .collect();
        let update = adoption_update(&codes);
        assert_eq!(update.rules.len(), 2);
        assert!(update.rule_tags.is_empty());
        assert!(update
            .rules
            .values()
            .all(|severity| *severity == Some(RuleSeverity::Warning)));
    }

    #[test]
    fn path_is_within_requires_a_real_ancestor() {
        let root = Path::new("/workspace/project");
        assert!(path_is_within(
            Path::new("/workspace/project/src/app.py"),
            root
        ));
        assert!(!path_is_within(Path::new("/workspace/other/app.py"), root));
        // Missing paths cannot be rescued by canonicalization.
        assert!(!path_is_within(
            Path::new("/does/not/exist/app.py"),
            Path::new("/does/not/exist-either")
        ));
    }
}
