//! Initialization and configuration handlers for the Basilisk LSP server.
//!
//! Covers `initialize`, `initialized`, `shutdown`, and `did_change_configuration`.

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    CallHierarchyServerCapability, CodeActionKind, CodeActionOptions, CodeActionProviderCapability,
    CodeLensOptions, ColorProviderCapability, CompletionOptions, DeclarationCapability,
    DidChangeConfigurationParams, ExecuteCommandOptions, FoldingRangeProviderCapability,
    HoverProviderCapability, InitializeParams, InitializeResult, MessageType, OneOf, RenameOptions,
    SelectionRangeProviderCapability, SemanticTokensFullOptions, SemanticTokensLegend,
    SemanticTokensOptions, SemanticTokensServerCapabilities, ServerCapabilities, ServerInfo,
    SignatureHelpOptions, TextDocumentSyncCapability, TextDocumentSyncKind,
    TypeDefinitionProviderCapability, Url, WorkDoneProgressOptions,
};
use tracing::info;

use crate::config::AnalysisMode;
use crate::workspace::WorkspaceIndex;

use super::LspServer;

/// Handle the `initialize` request: collect workspace roots, determine analysis
/// mode, build the workspace index and return server capabilities.
pub(super) async fn initialize(
    server: &LspServer,
    params: InitializeParams,
) -> LspResult<InitializeResult> {
    // Collect workspace roots.
    let mut roots: Vec<std::path::PathBuf> = Vec::new();
    if let Some(folders) = &params.workspace_folders {
        for folder in folders {
            if let Ok(path) = folder.uri.to_file_path() {
                roots.push(path);
            }
        }
    }
    if roots.is_empty() {
        if let Some(ref root_uri) = params.root_uri {
            if let Ok(path) = root_uri.to_file_path() {
                roots.push(path);
            }
        }
    }

    // Determine analysis mode from InitializationOptions then config files.
    let mode = crate::workspace_analysis::resolve_analysis_mode(
        params.initialization_options.as_ref(),
        &roots,
    );

    // Store workspace roots for later use by import resolution.
    *server.workspace_roots.write().await = roots.clone();

    // Build the workspace index now so `initialized()` can scan immediately.
    let index = WorkspaceIndex::new(roots, mode);
    *server.index.write().await = Some(index);

    Ok(InitializeResult {
        server_info: Some(ServerInfo {
            name: "basilisk".to_owned(),
            version: Some(env!("CARGO_PKG_VERSION").to_owned()),
        }),
        capabilities: build_capabilities(),
    })
}

/// Build the full `ServerCapabilities` for the `initialize` response.
fn build_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(
            TextDocumentSyncKind::INCREMENTAL,
        )),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        call_hierarchy_provider: Some(CallHierarchyServerCapability::Simple(true)),
        // type_hierarchy_provider is not in lsp-types 0.94's ServerCapabilities;
        // it is injected at the JSON level by websocket::inject_missing_capabilities.
        code_lens_provider: Some(CodeLensOptions {
            resolve_provider: Some(false),
        }),
        code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
            code_action_kinds: Some(vec![
                CodeActionKind::QUICKFIX,
                CodeActionKind::SOURCE_ORGANIZE_IMPORTS,
                CodeActionKind::REFACTOR,
            ]),
            ..Default::default()
        })),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec![".".to_owned()]),
            resolve_provider: Some(true),
            ..Default::default()
        }),
        declaration_provider: Some(DeclarationCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        type_definition_provider: Some(TypeDefinitionProviderCapability::Simple(true)),
        document_formatting_provider: Some(OneOf::Left(true)),
        document_highlight_provider: Some(OneOf::Left(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
        selection_range_provider: Some(SelectionRangeProviderCapability::Simple(true)),
        workspace_symbol_provider: Some(OneOf::Left(true)),
        signature_help_provider: Some(SignatureHelpOptions {
            trigger_characters: Some(vec!["(".to_owned(), ",".to_owned()]),
            retrigger_characters: None,
            work_done_progress_options: WorkDoneProgressOptions::default(),
        }),
        references_provider: Some(OneOf::Left(true)),
        rename_provider: Some(OneOf::Right(RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: WorkDoneProgressOptions::default(),
        })),
        execute_command_provider: Some(ExecuteCommandOptions {
            commands: basilisk_common::commands::ALL
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
            work_done_progress_options: WorkDoneProgressOptions::default(),
        }),
        inlay_hint_provider: Some(OneOf::Left(true)),
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
            SemanticTokensOptions {
                legend: SemanticTokensLegend {
                    token_types: crate::semantic_tokens::TOKEN_TYPES.to_vec(),
                    token_modifiers: crate::semantic_tokens::TOKEN_MODIFIERS.to_vec(),
                },
                full: Some(SemanticTokensFullOptions::Bool(true)),
                range: None,
                work_done_progress_options: WorkDoneProgressOptions::default(),
            },
        )),
        color_provider: Some(ColorProviderCapability::Simple(true)),
        ..Default::default()
    }
}

/// Handle the `initialized` notification: scan workspace if in whole-module or
/// cross-module mode, otherwise just log that scanning was skipped.
pub(super) async fn initialized(server: &LspServer) {
    server
        .client
        .log_message(MessageType::INFO, "Basilisk LSP initialized")
        .await;

    let guard = server.index.read().await;
    let Some(index) = guard.as_ref() else { return };

    match index.mode {
        AnalysisMode::OpenFilesOnly => {
            server
                .client
                .log_message(
                    MessageType::INFO,
                    "Basilisk: analysisMode=openFilesOnly — skipping workspace scan",
                )
                .await;
        }
        AnalysisMode::WholeModule | AnalysisMode::CrossModule => {
            server
                .client
                .log_message(MessageType::INFO, "Basilisk: scanning workspace files...")
                .await;
            let scan_result = scan_resolve_and_check(server, index).await;
            drop(guard);
            for (uri, diags) in scan_result.diagnostics {
                server.client.publish_diagnostics(uri, diags, None).await;
            }
            server
                .client
                .log_message(
                    MessageType::INFO,
                    format!(
                        "Basilisk: workspace scan complete — {} files, {} error(s)",
                        scan_result.file_count, scan_result.error_count
                    ),
                )
                .await;
            return;
        }
    }
    drop(guard);
}

/// Handle `didChangeConfiguration`: update the analysis mode on the index and
/// either trigger a workspace scan or clear diagnostics as appropriate.
pub(super) async fn did_change_configuration(
    server: &LspServer,
    params: DidChangeConfigurationParams,
) {
    let settings = params.settings;
    super::diaglog!("[DIAG] did_change_configuration: settings={settings}");
    info!(settings = %settings, "did_change_configuration received");

    let mut mode = None;
    if let Some(mode_str) = settings
        .get("analysisMode")
        .or_else(|| settings.get("basilisk").and_then(|b| b.get("analysisMode")))
        .and_then(|v| v.as_str())
    {
        mode = Some(AnalysisMode::parse(mode_str));
    }
    let Some(new_mode) = mode else {
        info!("did_change_configuration: no analysisMode found, ignoring");
        return;
    };

    // Update the mode on the index.
    {
        let mut guard = server.index.write().await;
        if let Some(index) = guard.as_mut() {
            index.mode = new_mode;
        }
    }

    server
        .client
        .log_message(
            MessageType::INFO,
            format!("Basilisk: analysis mode changed to {new_mode:?}"),
        )
        .await;

    match new_mode {
        AnalysisMode::WholeModule | AnalysisMode::CrossModule => {
            run_workspace_scan(server, new_mode).await;
        }
        AnalysisMode::OpenFilesOnly => {
            clear_non_open_diagnostics(server).await;
        }
    }
}

/// Scan the whole workspace and publish diagnostics for all files.
async fn run_workspace_scan(server: &LspServer, _mode: AnalysisMode) {
    let guard = server.index.read().await;
    let Some(index) = guard.as_ref() else { return };
    let scan_result = scan_resolve_and_check(server, index).await;
    drop(guard);

    for (uri, diags) in scan_result.diagnostics {
        server.client.publish_diagnostics(uri, diags, None).await;
    }
    server
        .client
        .log_message(
            MessageType::INFO,
            format!(
                "Basilisk: workspace scan complete — {} files, {} error(s)",
                scan_result.file_count, scan_result.error_count
            ),
        )
        .await;
}

/// Result of a full workspace scan with import resolution and optional cross-module
/// symbol population.
struct ScanResult {
    diagnostics: Vec<(Url, Vec<tower_lsp::lsp_types::Diagnostic>)>,
    file_count: usize,
    error_count: usize,
}

/// Scan workspace files, resolve imports, and optionally run cross-module symbol
/// population. Returns diagnostics ready to publish.
///
/// This is the single source of truth for the scan+resolve+crossmod pipeline,
/// used by both `initialized()` and `run_workspace_scan()`.
async fn scan_resolve_and_check(server: &LspServer, index: &WorkspaceIndex) -> ScanResult {
    let (results, file_count, error_count) = index.scan();

    // Resolve imports for all scanned files.
    let roots = server.workspace_roots.read().await;
    let config = roots
        .first()
        .map(|r| crate::config::load_config(r))
        .unwrap_or_default();
    let search_paths = crate::import_resolver::ImportSearchPaths::from_config(&roots, &config);
    crate::import_resolver::resolve_workspace_imports(index, &search_paths);
    drop(roots);

    // Cross-module: populate imported symbols and rebuild import graph, then re-check.
    let diagnostics = if matches!(index.mode, AnalysisMode::CrossModule) {
        crate::cross_module::populate_cross_module_symbols(index);
        index.build_import_graph();
        info!("cross-module symbol population complete");
        recheck_with_cross_module_symbols(index)
    } else {
        results
    };

    ScanResult {
        diagnostics,
        file_count,
        error_count,
    }
}

/// Clear diagnostics for all non-open files (used when switching to `openFilesOnly`).
async fn clear_non_open_diagnostics(server: &LspServer) {
    let guard = server.index.read().await;
    let Some(index) = guard.as_ref() else { return };
    let to_clear: Vec<Url> = index
        .files
        .iter()
        .filter(|entry| !entry.value().is_open)
        .filter_map(|entry| Url::from_file_path(entry.key()).ok())
        .collect();
    drop(guard);
    for uri in to_clear {
        server.client.publish_diagnostics(uri, vec![], None).await;
    }
}

/// Re-check all workspace files using their current `ResolvedModule` (which now
/// contains cross-module `imported_symbols`). Returns fresh LSP diagnostics.
///
/// This is called after `populate_cross_module_symbols()` so that checker rules
/// can see imported symbols from other modules.
fn recheck_with_cross_module_symbols(
    index: &WorkspaceIndex,
) -> Vec<(Url, Vec<tower_lsp::lsp_types::Diagnostic>)> {
    let mut results = Vec::new();

    for mut entry in index.files.iter_mut() {
        let Some(resolved) = &entry.resolved else {
            continue;
        };

        let checker_diags = basilisk_checker::check(resolved);
        let lsp_diags: Vec<tower_lsp::lsp_types::Diagnostic> = checker_diags
            .iter()
            .map(|d| crate::workspace_analysis::bsk_to_lsp(d, &entry.text))
            .collect();

        entry.diagnostics = checker_diags;

        if let Some(uri) = crate::workspace_scan::path_to_uri(entry.key()) {
            results.push((uri, lsp_diags));
        }
    }

    results
}

/// Handle the `shutdown` request: stop all debug sessions.
pub(super) async fn shutdown(server: &LspServer) -> LspResult<()> {
    server.debug_manager.stop_all().await;
    Ok(())
}
