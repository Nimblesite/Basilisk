//! Compatibility configuration commands backed by the typed editor service.

use std::collections::BTreeMap;
use std::path::PathBuf;

use tower_lsp::jsonrpc::{Error, ErrorCode, Result as LspResult};
use tower_lsp::lsp_types::MessageType;
use tracing::info;

use super::LspServer;

/// Handle the legacy `basilisk.disableRule` command through active config.
pub(super) async fn execute_disable_rule(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let Some(argument) = args.first() else {
        return Ok(None);
    };
    let Some(rule) = argument.get("rule").and_then(serde_json::Value::as_str) else {
        return Ok(None);
    };
    let severity = argument
        .get("severity")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("off");
    validate_rule(rule)?;
    let parsed_severity = parse_severity(severity)?;
    // Implements [CHKARCH-CONFIG-MODEL] via [CONFIGEDITOR-OPERATIONS]: pep
    // rules are graded, never disabled — a disable request is an error.
    if parsed_severity == basilisk_config::RuleSeverity::Disabled
        && basilisk_checker::is_pep_rule(rule)
    {
        return Err(command_error(
            "pepRuleDisable",
            "pep rules are graded, never disabled",
        ));
    }
    let root = command_root(server, argument).await?;
    let update = basilisk_config::RuleConfigUpdate {
        rules: BTreeMap::from([(rule.to_owned(), Some(parsed_severity))]),
        rule_tags: BTreeMap::new(),
    };
    let document = crate::configuration_editor::apply_rule_updates(
        server,
        &root,
        &update,
        "legacyDisableRule",
    )
    .await?;
    report_success(server, rule, severity).await;
    Ok(Some(serde_json::json!({
        "rule": rule,
        "severity": severity,
        "path": document.path.display().to_string(),
    })))
}

fn validate_rule(rule: &str) -> LspResult<()> {
    if basilisk_checker::rule_catalog()
        .iter()
        .any(|descriptor| descriptor.code == rule)
    {
        Ok(())
    } else {
        Err(command_error(
            "unknownRule",
            "disableRule received an unknown rule",
        ))
    }
}

fn parse_severity(value: &str) -> LspResult<basilisk_config::RuleSeverity> {
    match value {
        "off" | "disabled" => Ok(basilisk_config::RuleSeverity::Disabled),
        "error" => Ok(basilisk_config::RuleSeverity::Error),
        "warning" => Ok(basilisk_config::RuleSeverity::Warning),
        "info" => Ok(basilisk_config::RuleSeverity::Info),
        _ => Err(command_error(
            "invalidMutation",
            "disableRule received an invalid severity",
        )),
    }
}

async fn command_root(server: &LspServer, argument: &serde_json::Value) -> LspResult<PathBuf> {
    let roots = server.workspace_roots.read().await;
    select_command_root(&roots, argument.get("uri"))
}

// Implements [CONFIGEDITOR-OPERATIONS]: an explicitly supplied document URI
// is an authority boundary. Invalid and out-of-workspace values must fail
// instead of silently falling back to an unrelated single root.
fn select_command_root(
    roots: &[PathBuf],
    uri_value: Option<&serde_json::Value>,
) -> LspResult<PathBuf> {
    if let Some(value) = uri_value {
        let path = explicit_argument_path(value)?;
        return roots
            .iter()
            .filter(|root| path.starts_with(root))
            .max_by_key(|root| root.components().count())
            .cloned()
            .ok_or_else(|| invalid_params("disableRule URI is outside initialized roots"));
    }
    match roots {
        [root] => Ok(root.clone()),
        _ => Err(command_error(
            "invalidMutation",
            "disableRule requires a document URI in a multi-root workspace",
        )),
    }
}

fn explicit_argument_path(value: &serde_json::Value) -> LspResult<PathBuf> {
    let value = value
        .as_str()
        .ok_or_else(|| invalid_params("disableRule URI must be a string"))?;
    let uri = tower_lsp::lsp_types::Url::parse(value)
        .map_err(|_failure| invalid_params("disableRule URI is invalid"))?;
    uri.to_file_path()
        .map_err(|()| invalid_params("disableRule URI must use the file scheme"))
}

async fn report_success(server: &LspServer, rule: &str, severity: &str) {
    info!(
        rule,
        severity, "disableRule persisted through active configuration"
    );
    server
        .client
        .log_message(
            MessageType::INFO,
            format!("Basilisk: set {rule} to {severity} in the active configuration"),
        )
        .await;
}

fn command_error(kind: &str, message: &str) -> Error {
    Error {
        code: ErrorCode::ServerError(-32020),
        message: message.to_owned().into(),
        data: Some(serde_json::json!({ "kind": kind })),
    }
}

fn invalid_params(message: &str) -> Error {
    Error {
        code: ErrorCode::InvalidParams,
        message: message.to_owned().into(),
        data: None,
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;
    use std::path::PathBuf;

    use super::select_command_root;

    #[test]
    fn explicit_disable_rule_uri_must_be_valid_and_in_scope() -> Result<(), Box<dyn StdError>> {
        let root = PathBuf::from("/workspace/project");
        let roots = vec![root.clone()];
        let valid = serde_json::json!("file:///workspace/project/src/app.py");
        assert_eq!(select_command_root(&roots, Some(&valid))?, root);
        for value in [
            serde_json::json!(null),
            serde_json::json!(42),
            serde_json::json!("not a uri"),
            serde_json::json!("https://example.com/app.py"),
            serde_json::json!("file:///outside/app.py"),
        ] {
            let error = select_command_root(&roots, Some(&value))
                .err()
                .ok_or("invalid explicit URI must fail")?;
            assert_eq!(error.code, tower_lsp::jsonrpc::ErrorCode::InvalidParams);
        }
        Ok(())
    }

    #[test]
    fn absent_disable_rule_uri_keeps_single_root_compatibility() -> Result<(), Box<dyn StdError>> {
        let root = PathBuf::from("/workspace/project");
        assert_eq!(
            select_command_root(std::slice::from_ref(&root), None)?,
            root
        );
        Ok(())
    }
}
