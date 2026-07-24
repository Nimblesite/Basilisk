//! Safe/all file and workspace fix command implementation.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use tower_lsp::jsonrpc::{Error, ErrorCode, Result as LspResult};
use tower_lsp::lsp_types::{
    DocumentChanges, MessageType, OneOf, OptionalVersionedTextDocumentIdentifier, TextDocumentEdit,
    TextEdit, Url, WorkspaceEdit,
};
use tracing::{error, info, warn};

use crate::code_actions::mass_fix::{ALL_FIXABLE_RULES, SAFE_FIXABLE_RULES};
use crate::workspace::{FileEntry, WorkspaceIndex};

use super::LspServer;

#[derive(Clone, Debug)]
struct FixTarget {
    uri: Url,
    source: String,
    version: Option<i32>,
    edits: Vec<TextEdit>,
}

/// Whether `command` belongs to the mass-fix family.
pub(super) fn is_fix_command(command: &str) -> bool {
    matches!(
        command,
        basilisk_common::commands::FIX_FILE
            | basilisk_common::commands::FIX_FILE_ALL
            | basilisk_common::commands::FIX_WORKSPACE
            | basilisk_common::commands::FIX_WORKSPACE_ALL
    )
}

/// Dispatch safe defaults and explicit unsafe-inclusive variants.
pub(super) async fn dispatch(
    server: &LspServer,
    command: &str,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    match command {
        basilisk_common::commands::FIX_FILE => fix_file(server, args, SAFE_FIXABLE_RULES).await,
        basilisk_common::commands::FIX_FILE_ALL => fix_file(server, args, ALL_FIXABLE_RULES).await,
        basilisk_common::commands::FIX_WORKSPACE => {
            fix_workspace(server, args, SAFE_FIXABLE_RULES).await
        }
        basilisk_common::commands::FIX_WORKSPACE_ALL => {
            fix_workspace(server, args, ALL_FIXABLE_RULES).await
        }
        _ => Ok(None),
    }
}

async fn fix_file(
    server: &LspServer,
    args: &[serde_json::Value],
    allowed_rules: &'static [&'static str],
) -> LspResult<Option<serde_json::Value>> {
    let Some(uri) = args
        .first()
        .and_then(serde_json::Value::as_str)
        .and_then(|value| Url::parse(value).ok())
    else {
        return Ok(None);
    };
    let target = server
        .with_index(|index| index_target(index, &uri, allowed_rules))
        .await;
    let Some(target) = target else {
        warn!(uri = %uri, "fixFile: no fixable diagnostics");
        return Ok(Some(serde_json::json!({ "fixed": 0 })));
    };
    let edit_count = target.edits.len();
    apply_targets(server, std::slice::from_ref(&target)).await?;
    report_file_result(server, &uri, edit_count).await;
    Ok(Some(serde_json::json!({ "fixed": edit_count })))
}

async fn report_file_result(server: &LspServer, uri: &Url, edit_count: usize) {
    info!(uri = %uri, edit_count, "fixFile: completed");
    server
        .client
        .log_message(
            MessageType::INFO,
            format!("Basilisk: fixed {edit_count} issues in {uri}"),
        )
        .await;
}

async fn fix_workspace(
    server: &LspServer,
    args: &[serde_json::Value],
    allowed_rules: &'static [&'static str],
) -> LspResult<Option<serde_json::Value>> {
    let roots = initialized_roots(server).await;
    let root = parse_requested_root(args, &roots)?;
    let Some(targets) = collect_workspace_targets(server, root.as_deref(), allowed_rules).await
    else {
        warn!("fixWorkspace: workspace index not available");
        return Ok(Some(empty_workspace_result()));
    };
    if targets.is_empty() {
        info!(root = ?root, "fixWorkspace: no fixable diagnostics found");
        return Ok(Some(empty_workspace_result()));
    }
    let file_count = targets.len();
    let edit_count = targets.iter().map(|target| target.edits.len()).sum();
    apply_targets(server, &targets).await?;
    report_workspace_result(server, file_count, edit_count).await;
    Ok(Some(
        serde_json::json!({ "fixed": edit_count, "files": file_count }),
    ))
}

async fn initialized_roots(server: &LspServer) -> Vec<PathBuf> {
    server
        .with_index(|index| Some(index.roots.clone()))
        .await
        .unwrap_or_default()
}

async fn collect_workspace_targets(
    server: &LspServer,
    root: Option<&Path>,
    allowed_rules: &'static [&'static str],
) -> Option<Vec<FixTarget>> {
    server
        .with_index(|index| Some(index_targets(index, root, allowed_rules)))
        .await
}

fn index_targets(
    index: &WorkspaceIndex,
    root: Option<&Path>,
    allowed_rules: &'static [&'static str],
) -> Vec<FixTarget> {
    let mut targets = index
        .files
        .iter()
        .filter(|entry| path_in_scope(entry.key(), &index.roots, root))
        .filter_map(|entry| {
            let uri = Url::from_file_path(entry.key()).ok()?;
            target_from_entry(&uri, entry.value(), allowed_rules)
        })
        .collect::<Vec<_>>();
    targets.sort_by(|left, right| left.uri.as_str().cmp(right.uri.as_str()));
    targets
}

// Implements [AUTOFIX-MASS-OVERVIEW]: legacy no-argument commands mean every
// initialized root, never every entry in the index (which may include external
// documents opened explicitly by the user).
fn path_in_scope(path: &Path, roots: &[PathBuf], selected: Option<&Path>) -> bool {
    selected.map_or_else(
        || roots.iter().any(|root| path.starts_with(root)),
        |root| path.starts_with(root),
    )
}

fn index_target(
    index: &WorkspaceIndex,
    uri: &Url,
    allowed_rules: &'static [&'static str],
) -> Option<FixTarget> {
    let path = uri.to_file_path().ok()?;
    if let Some(entry) = index.files.get(&path) {
        return target_from_entry(uri, entry.value(), allowed_rules);
    }
    let canonical = path.canonicalize().ok()?;
    let entry = index.files.get(&canonical)?;
    target_from_entry(uri, entry.value(), allowed_rules)
}

fn target_from_entry(
    uri: &Url,
    entry: &FileEntry,
    allowed_rules: &'static [&'static str],
) -> Option<FixTarget> {
    let diagnostics = entry
        .diagnostics
        .iter()
        .map(|diagnostic| crate::workspace_analysis::bsk_to_lsp(diagnostic, &entry.text))
        .collect::<Vec<_>>();
    let edits =
        crate::code_actions::fix_filtered_in_file(uri, &diagnostics, &entry.text, allowed_rules)
            .and_then(|action| action.edit)
            .and_then(|edit| edit.changes)
            .and_then(|mut changes| changes.remove(uri))?;
    (!edits.is_empty()).then(|| FixTarget {
        uri: uri.clone(),
        source: entry.text.clone(),
        version: entry.is_open.then_some(entry.version),
        edits,
    })
}

// Implements [ANALYSIS-INDEX-OPEN] / [AUTOFIX-UNDO]: open buffers carry their
// exact LSP version, while every target remains in one documentChanges edit.
fn workspace_edit(targets: &[FixTarget]) -> WorkspaceEdit {
    let documents = targets
        .iter()
        .map(|target| TextDocumentEdit {
            text_document: OptionalVersionedTextDocumentIdentifier {
                uri: target.uri.clone(),
                version: target.version,
            },
            edits: target.edits.iter().cloned().map(OneOf::Left).collect(),
        })
        .collect();
    WorkspaceEdit {
        changes: None,
        document_changes: Some(DocumentChanges::Edits(documents)),
        change_annotations: None,
    }
}

async fn apply_targets(server: &LspServer, targets: &[FixTarget]) -> LspResult<()> {
    let edit = workspace_edit(targets);
    match server.client.apply_edit(edit).await {
        Ok(response) if response.applied => {
            let edit_count = targets
                .iter()
                .map(|target| target.edits.len())
                .sum::<usize>();
            info!(edit_count, files = targets.len(), "mass-fix edits applied");
        }
        Ok(response) => {
            return Err(edit_rejected(
                response
                    .failure_reason
                    .as_deref()
                    .unwrap_or("client rejected edit"),
            ));
        }
        Err(failure) => return Err(edit_rejected(&failure.to_string())),
    }
    converge_index(server, targets).await;
    Ok(())
}

// Implements [AUTOFIX-CONFLICTS] / [ANALYSIS-INDEX-OPEN]: once the client has
// accepted the deterministic non-overlapping edits, mirror those same edits in
// the index, reanalyse, and publish the remaining diagnostics before replying.
async fn converge_index(server: &LspServer, targets: &[FixTarget]) {
    let results = server
        .with_index(|index| Some(apply_targets_to_index(index, targets)))
        .await
        .unwrap_or_default();
    for (uri, diagnostics) in results {
        server
            .publish_diagnostics_if_enabled(uri, diagnostics)
            .await;
    }
}

fn apply_targets_to_index(
    index: &WorkspaceIndex,
    targets: &[FixTarget],
) -> Vec<(Url, Vec<tower_lsp::lsp_types::Diagnostic>)> {
    let mut results = BTreeMap::new();
    for target in targets {
        if let Some(diagnostics) =
            index.apply_accepted_text_edits(&target.uri, &target.source, &target.edits)
        {
            let _ = results.insert(target.uri.to_string(), (target.uri.clone(), diagnostics));
        }
    }
    for (uri, diagnostics) in index.reresolve_imports_and_recheck() {
        let _ = results.insert(uri.to_string(), (uri, diagnostics));
    }
    results.into_values().collect()
}

async fn report_workspace_result(server: &LspServer, file_count: usize, edit_count: usize) {
    server
        .client
        .log_message(
            MessageType::INFO,
            format!("Basilisk: fixed {edit_count} issues across {file_count} file(s)"),
        )
        .await;
}

fn parse_requested_root(
    args: &[serde_json::Value],
    roots: &[PathBuf],
) -> LspResult<Option<PathBuf>> {
    let Some(argument) = args.first() else {
        return Ok(None);
    };
    if argument.is_string() {
        return Ok(None);
    }
    let object = argument
        .as_object()
        .ok_or_else(|| invalid_root("root argument must be an object"))?;
    let value = object
        .get("rootUri")
        .ok_or_else(|| invalid_root("rootUri is required"))?
        .as_str()
        .ok_or_else(|| invalid_root("rootUri must be a string"))?;
    let uri = Url::parse(value).map_err(|failure| invalid_root(&failure.to_string()))?;
    let path = uri
        .to_file_path()
        .map_err(|()| invalid_root("rootUri must be a file URI"))?;
    roots
        .iter()
        .find(|root| same_path(root, &path))
        .cloned()
        .map(Some)
        .ok_or_else(|| invalid_root("rootUri is not initialized by this server"))
}

fn same_path(left: &Path, right: &Path) -> bool {
    left == right
        || left
            .canonicalize()
            .ok()
            .zip(right.canonicalize().ok())
            .is_some_and(|(left, right)| left == right)
}

fn invalid_root(message: &str) -> Error {
    Error {
        code: ErrorCode::InvalidParams,
        message: message.to_owned().into(),
        data: None,
    }
}

fn edit_rejected(message: &str) -> Error {
    error!(reason = message, "mass-fix workspace edit was not applied");
    Error {
        code: ErrorCode::ServerError(-32021),
        message: "client did not apply the mass-fix workspace edit".into(),
        data: Some(serde_json::json!({ "kind": "clientRejectedEdit", "reason": message })),
    }
}

fn empty_workspace_result() -> serde_json::Value {
    serde_json::json!({ "fixed": 0, "files": 0 })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::error::Error as StdError;
    use std::path::{Path, PathBuf};

    use tower_lsp::lsp_types::{DocumentChanges, Url};

    use super::{
        apply_targets_to_index, index_target, parse_requested_root, path_in_scope, workspace_edit,
    };
    use crate::code_actions::mass_fix::SAFE_FIXABLE_RULES;
    use crate::config::AnalysisMode;
    use crate::workspace::WorkspaceIndex;

    #[test]
    fn no_root_scope_excludes_external_open_documents() {
        let roots = vec![PathBuf::from("/workspace/a"), PathBuf::from("/workspace/b")];
        assert!(path_in_scope(
            Path::new("/workspace/a/src/app.py"),
            &roots,
            None
        ));
        assert!(path_in_scope(
            Path::new("/workspace/b/src/app.py"),
            &roots,
            None
        ));
        assert!(!path_in_scope(Path::new("/external/open.py"), &roots, None));
        assert!(!path_in_scope(
            Path::new("/workspace/b/src/app.py"),
            &roots,
            Some(Path::new("/workspace/a"))
        ));
    }

    #[test]
    fn object_root_arguments_are_strictly_validated() -> Result<(), Box<dyn StdError>> {
        let root = PathBuf::from("/workspace/a");
        let roots = vec![root.clone()];
        let valid = parse_requested_root(
            &[serde_json::json!({ "rootUri": "file:///workspace/a" })],
            &roots,
        )?;
        assert_eq!(valid, Some(root));
        for argument in [
            serde_json::json!({}),
            serde_json::json!({ "rootUri": null }),
            serde_json::json!({ "rootUri": 42 }),
            serde_json::json!({ "rootUri": "not a uri" }),
            serde_json::json!({ "rootUri": "file:///outside" }),
        ] {
            let error = parse_requested_root(&[argument], &roots)
                .err()
                .ok_or("malformed root must fail")?;
            assert_eq!(error.code, tower_lsp::jsonrpc::ErrorCode::InvalidParams);
        }
        assert_eq!(parse_requested_root(&[], &roots)?, None);
        assert_eq!(
            parse_requested_root(&[serde_json::json!("file:///legacy.py")], &roots)?,
            None
        );
        Ok(())
    }

    #[test]
    fn open_targets_use_versioned_document_changes() -> Result<(), Box<dyn StdError>> {
        let config = basilisk_config::BasiliskConfig::with_rule_entries(enabled_fix_test_rules());
        let index =
            WorkspaceIndex::new(vec![PathBuf::from("/")], AnalysisMode::WholeModule, config);
        let uri = Url::parse("file:///versioned.py")?;
        let source = "x: int = 42\ny = None\n";
        let _ = index.set_open(&uri, source, 17);
        let target =
            index_target(&index, &uri, SAFE_FIXABLE_RULES).ok_or("expected a safe fix target")?;
        let edit = workspace_edit(std::slice::from_ref(&target));
        let Some(DocumentChanges::Edits(documents)) = edit.document_changes else {
            return Err("expected text document edits".into());
        };
        assert_eq!(documents.len(), 1);
        let document = documents.first().ok_or("expected one document edit")?;
        assert_eq!(document.text_document.version, Some(17));
        assert!(edit.changes.is_none());
        Ok(())
    }

    #[test]
    fn accepted_partial_fixes_converge_index_and_keep_remaining_diagnostics(
    ) -> Result<(), Box<dyn StdError>> {
        let config = basilisk_config::BasiliskConfig::with_rule_entries(enabled_fix_test_rules());
        let index =
            WorkspaceIndex::new(vec![PathBuf::from("/")], AnalysisMode::WholeModule, config);
        let uri = Url::parse("file:///converged.py")?;
        let source = "x: int = 42\ny = None\n";
        let _ = index.set_open(&uri, source, 23);
        let target =
            index_target(&index, &uri, SAFE_FIXABLE_RULES).ok_or("expected a safe fix target")?;
        let published = apply_targets_to_index(&index, std::slice::from_ref(&target));
        let entry = index
            .files
            .get(Path::new("/converged.py"))
            .ok_or("updated index entry missing")?;
        assert_eq!(entry.text, "x = 42\ny = None\n");
        assert_eq!(entry.version, 23);
        assert!(entry.is_open);
        assert!(entry
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.code == "BSK-0003"));
        assert!(published.iter().any(|(published_uri, diagnostics)| {
            published_uri == &uri
                && diagnostics.iter().any(|diagnostic| {
                    matches!(
                        diagnostic.code.as_ref(),
                        Some(tower_lsp::lsp_types::NumberOrString::String(code))
                            if code == "BSK-0003"
                    )
                })
        }));
        Ok(())
    }

    fn enabled_fix_test_rules() -> HashMap<String, basilisk_config::RuleSeverity> {
        HashMap::from([
            ("BSK-0003".to_owned(), basilisk_config::RuleSeverity::Error),
            (
                "BSK-0050".to_owned(),
                basilisk_config::RuleSeverity::Warning,
            ),
        ])
    }
}
