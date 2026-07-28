//! Implements [LSPARCH-ARCH-MODSTRUCT]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-MODSTRUCT
//!
//! Initialization and configuration handlers for the Basilisk LSP server.
//!
//! Covers `initialize`, `initialized`, `shutdown`, and `did_change_configuration`.

use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    CallHierarchyServerCapability, CodeActionKind, CodeActionOptions, CodeActionProviderCapability,
    CodeLensOptions, ColorProviderCapability, CompletionOptions, DeclarationCapability,
    DidChangeConfigurationParams, DidChangeWatchedFilesRegistrationOptions,
    DidChangeWorkspaceFoldersParams, ExecuteCommandOptions, FileOperationFilter,
    FileOperationPattern, FileOperationPatternKind, FileOperationRegistrationOptions,
    FileSystemWatcher, FoldingRangeProviderCapability, GlobPattern, HoverProviderCapability,
    InitializeParams, InitializeResult, MessageType, OneOf, Registration, RenameOptions,
    SelectionRangeProviderCapability, SemanticTokensFullOptions, SemanticTokensLegend,
    SemanticTokensOptions, SemanticTokensServerCapabilities, ServerCapabilities, ServerInfo,
    SignatureHelpOptions, TextDocumentSyncCapability, TextDocumentSyncKind,
    TypeDefinitionProviderCapability, Url, WorkDoneProgressOptions,
    WorkspaceFileOperationsServerCapabilities, WorkspaceFoldersServerCapabilities,
    WorkspaceServerCapabilities,
};
use tower_lsp::Client;
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

    // Implements [LSPARCH-DIAGNOSTIC-SCOPE]: `basilisk.analyze = false`
    // restricts published diagnostics to check scope (`pep` rules only).
    // Default true — the LSP publishes the union of both scopes. Project
    // configuration never selects scope.
    if let Some(analyze) = params
        .initialization_options
        .as_ref()
        .and_then(parse_analyze)
    {
        server
            .analyze_enabled
            .store(analyze, std::sync::atomic::Ordering::Relaxed);
        info!(analyze, "diagnostic scope option applied");
    }

    // Implements [LSPARCH-CONFIG-SEEDING]: seed each unconfigured root's
    // pyproject.toml with the two-line strict-by-default seed BEFORE the
    // first analysis, so the checker config loaded below already sees it.
    for root in &roots {
        let _ = crate::config_seed::seed_root_if_unconfigured(root);
    }

    // Honor the Type Checking toggle (`basilisk.enabled`) supplied at startup so
    // a client that opens with type checking off never sees a flash of
    // diagnostics from the initial scan. Implements [ANALYSIS-ENABLED] (#65/#119).
    if let Some(enabled) = params
        .initialization_options
        .as_ref()
        .and_then(parse_enabled)
    {
        *server.type_checking_enabled.write().await = enabled;
    }

    // Resolve the formatter engine ([LSPFMT-CONFIG]): the editor setting
    // (initializationOptions `formatter`, e.g. VS Code's `basilisk.formatter`)
    // wins over config files; the default is the embedded Ruff engine.
    let formatter = params
        .initialization_options
        .as_ref()
        .and_then(parse_formatter)
        .unwrap_or_else(|| {
            roots
                .first()
                .map(|r| crate::config::load_config(r).formatter)
                .unwrap_or_default()
        });
    let formatting_enabled = formatter != crate::config::FormatterEngine::Disabled;
    server
        .formatting_enabled
        .store(formatting_enabled, std::sync::atomic::Ordering::Relaxed);

    // Store workspace roots for later use by import resolution.
    (*server.workspace_roots.write().await).clone_from(&roots);
    let python_interpreter = params
        .initialization_options
        .as_ref()
        .and_then(parse_python_interpreter);
    (*server.python_interpreter.write().await).clone_from(&python_interpreter);

    // Resolve every root's terminal Typeshed generation BEFORE answering
    // `initialize`: resolution is a local store/bundle read
    // ([STUBRES-TYPESHED-OFFLINE]), so the initialize payload below carries
    // real Ready/NoSource statuses and no client ever renders an
    // intermediate state ([LSPCFGED-TYPESHED]). Statuses ride the payload;
    // change notifications only flow for later generation changes.
    server.typeshed_generations.write().await.clear();
    resolve_typeshed_for_roots(server, roots.clone(), false).await;

    // Load project-level checker config (pyproject.toml [tool.basilisk]) so
    // that rule severity overrides, per-module, and per-path settings match
    // the CLI. The loader walks ancestor directories and merges cumulatively
    // ([CHKARCH-CONFIG-DISCOVERY], GitHub #311), so a workspace folder opened
    // inside a project still discovers the project's config.
    let checker_config = roots
        .first()
        .map(|root| checker_config_for_root(root, python_interpreter.as_deref()))
        .unwrap_or_default();

    // Resolve the environment actually in use (python / uv / this binary) and
    // surface it via `experimental.basilisk.resolvedEnvironment` so editors can
    // show what auto-detect found ([LSPARCH-RESOLVED-ENV], GitHub #153). The
    // payload MERGES into the experimental capabilities from
    // `build_capabilities` — `configurationEditor`
    // ([LSPARCH-CONFIG-EDITOR-PROTOCOL]) must survive alongside it.
    let mut capabilities = build_capabilities(formatting_enabled);
    capabilities.experimental = Some(super::resolved_env::experimental_payload(
        capabilities.experimental.take(),
        params.initialization_options.as_ref(),
        &roots,
    ));
    let typeshed_generations = server.typeshed_generations.read().await;
    capabilities.experimental = Some(super::typeshed_status::experimental_payload(
        capabilities.experimental.take(),
        &typeshed_generations,
    ));
    drop(typeshed_generations);

    // Build the workspace index now so `initialized()` can scan immediately.
    let index = WorkspaceIndex::new(roots, mode, checker_config);
    *server.index.write().await = Some(index);

    Ok(InitializeResult {
        server_info: Some(ServerInfo {
            name: "basilisk".to_owned(),
            // [LSPFMT-PROVENANCE]: every client surfaces which Ruff formatter
            // is embedded, alongside the binary version.
            version: Some(format!(
                "{} (Ruff formatter {})",
                env!("CARGO_PKG_VERSION"),
                crate::formatting::EMBEDDED_RUFF_FORMATTER_VERSION
            )),
        }),
        capabilities,
    })
}

/// Read the analyze-scope opt-out from `initializationOptions`
/// ([LSPARCH-DIAGNOSTIC-SCOPE]). Accepts the flat `analyze` key and the
/// nested `{ basilisk: { analyze } }` shape.
fn parse_analyze(value: &serde_json::Value) -> Option<bool> {
    value
        .get("analyze")
        .or_else(|| value.get("basilisk").and_then(|b| b.get("analyze")))
        .and_then(serde_json::Value::as_bool)
}

/// Read the `formatter` engine from `initializationOptions` ([LSPFMT-CONFIG]).
fn parse_formatter(value: &serde_json::Value) -> Option<crate::config::FormatterEngine> {
    value
        .get("formatter")
        .or_else(|| value.get("basilisk").and_then(|b| b.get("formatter")))
        .and_then(serde_json::Value::as_str)
        .map(crate::config::FormatterEngine::parse)
}

/// Read the explicit editor-selected Python binary.
///
/// VS Code sends this as `initializationOptions.basilisk.python`; accepting the
/// flat form as well keeps the server usable by other LSP clients. Empty
/// strings mean auto-detect and therefore do not manufacture a target.
fn parse_python_interpreter(value: &serde_json::Value) -> Option<std::path::PathBuf> {
    value
        .get("python")
        .or_else(|| value.get("basilisk").and_then(|b| b.get("python")))
        .and_then(serde_json::Value::as_str)
        .filter(|path| !path.trim().is_empty())
        .map(std::path::PathBuf::from)
}

/// Build the full `ServerCapabilities` for the `initialize` response.
///
/// Formatting capabilities are advertised only while the formatter engine is
/// enabled ([LSPFMT-CAPABILITIES], [LSPFMT-CONFIG]).
fn build_capabilities(formatting_enabled: bool) -> ServerCapabilities {
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
                crate::code_actions::mass_fix::fix_all_kind(),
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
        document_formatting_provider: formatting_enabled.then_some(OneOf::Left(true)),
        document_range_formatting_provider: formatting_enabled.then_some(OneOf::Left(true)),
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
        // Implements [LSPARCH-CMDREG]
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
        workspace: Some(WorkspaceServerCapabilities {
            workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                supported: Some(true),
                change_notifications: Some(OneOf::Left(true)),
            }),
            file_operations: Some(WorkspaceFileOperationsServerCapabilities {
                will_rename: Some(FileOperationRegistrationOptions {
                    filters: vec![FileOperationFilter {
                        scheme: Some("file".to_owned()),
                        pattern: FileOperationPattern {
                            glob: "**/*.py".to_owned(),
                            matches: Some(FileOperationPatternKind::File),
                            options: None,
                        },
                    }],
                }),
                ..Default::default()
            }),
        }),
        // [LSPARCH-CONFIG-EDITOR-PROTOCOL]: presence of `configurationEditor`
        // is the whole capability — the editor ships with the server, so
        // there is no protocol version to negotiate.
        experimental: Some(serde_json::json!({
            "basilisk": {
                "configurationEditor": true
            }
        })),
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

    install_initial_search_paths(server).await;

    let statuses = server
        .typeshed_generations
        .read()
        .await
        .values()
        .filter_map(super::typeshed_status::TypeshedGeneration::ready_status)
        .cloned()
        .collect::<Vec<_>>();
    for status in statuses {
        super::typeshed_status::show_high_warnings(&server.client, &status).await;
    }

    // Implements [LSPDEBUG-PYRES-WARM] — pay the first-interpreter-spawn cost
    // here, in the background, rather than in front of the user's first debug
    // session. On win32 the first Python process a server spawns costs ~15s
    // against ~0.4s for every one after it, and that stall used to land
    // entirely on the first F5. Spawned, never awaited: initialization must
    // not wait on it, and a workspace with no usable interpreter must still
    // initialize.
    let debug_manager = Arc::clone(&server.debug_manager);
    let roots = Arc::clone(&server.workspace_roots);
    drop(tokio::spawn(async move {
        let root = roots
            .read()
            .await
            .first()
            .cloned()
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let python = crate::debug::resolve_python(&root);
        debug_manager.warm_debugpy(&python).await;
    }));

    // Spawn file watcher registration in the background so it never blocks
    // the message loop (register_capability sends a request to the client).
    let client = server.client.clone();
    drop(tokio::spawn(async move {
        register_file_watchers(&client).await;
    }));

    // Implements [LSPARCH-CONFIG]: the server watches the active
    // configuration source itself. Client `didChangeWatchedFiles` support is
    // an optimization, never a requirement — clients without watchers (Zed)
    // get identical reactive configuration behaviour.
    let watcher = crate::configuration_editor::spawn_configuration_watcher(
        server.refresh_handles(),
        Arc::clone(&server.workspace_roots),
    );
    if let Some(previous) = server.config_watcher.lock().await.replace(watcher) {
        previous.abort();
    }

    let ready_roots = typeshed_ready_roots(server).await;
    let root_count = server.workspace_roots.read().await.len();
    if ready_roots.is_empty() {
        server
            .client
            .log_message(
                MessageType::WARNING,
                "Basilisk: analysis is blocked because no workspace root has an active Typeshed source",
            )
            .await;
        return;
    }
    if ready_roots.len() < root_count {
        server
            .client
            .log_message(
                MessageType::WARNING,
                "Basilisk: some workspace roots are blocked; healthy roots will continue independently",
            )
            .await;
    }

    // Read the analysis mode, then release the lock immediately so the
    // message loop is not blocked while the workspace scan runs.
    let mode = {
        let guard = server.index.read().await;
        guard.as_ref().map(WorkspaceIndex::mode)
    };

    let Some(mode) = mode else { return };

    match mode {
        // Implements [ANALYSIS-STARTUP-OPEN]
        AnalysisMode::OpenFilesOnly => {
            // `didOpen` may arrive before this notification. The document
            // handler preserves that buffer's authoritative text; this is the
            // convergence point that analyses and publishes every deferred
            // open file owned by a root with a Ready generation.
            let guard = server.index.read().await;
            let Some(index) = guard.as_ref() else { return };
            let results = index.refresh_open_files_for_roots(&ready_roots);
            drop(guard);
            for (uri, diagnostics) in results {
                server
                    .publish_diagnostics_if_enabled(uri, diagnostics)
                    .await;
            }

            // No workspace scan runs in this mode, so zero-file rollups are
            // already final ([EXTACT-MODULES-HEADER-LOADING], GitHub #144).
            server
                .initial_scan_complete
                .store(true, std::sync::atomic::Ordering::Relaxed);
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

            // Spawn the workspace scan in the background so the server can
            // respond to requests (documentSymbol, etc.) while scanning.
            // tower-lsp v0.20 processes messages sequentially — blocking here
            // prevents ALL request handling until the scan completes.
            run_workspace_scan(server, mode).await;
        }
    }

    // Spawn initial test discovery in the background.
    spawn_initial_test_discovery(server);
}

/// Resolve each root's terminal generation from local sources only — never
/// the network ([STUBRES-TYPESHED-OFFLINE]). Identical root policies may
/// resolve the same immutable store entry, but each result is still keyed by
/// its owning root so later configuration changes can replace one generation
/// without cross-root bleed.
async fn resolve_typeshed_for_roots(
    server: &LspServer,
    roots: Vec<std::path::PathBuf>,
    notify: bool,
) {
    let interpreter = server.python_interpreter.read().await.clone();
    for root in roots {
        let mut config = crate::config::load_config(&root);
        config.python_interpreter.clone_from(&interpreter);
        let generation = crate::configuration_editor::resolve_workspace(config).await;
        if let Some(failure) = generation.no_source_failure() {
            // A NoSource root produces NO diagnostics, so a silent trace-log line
            // would leave the user believing a clean workspace is being checked.
            // Drive an immediate, client-agnostic error — on the initialize path
            // too (`notify` only gates the status *change* notification, which the
            // initialize payload already carries). [STUBRES-TYPESHED-WARN]
            tracing::warn!(
                root = %root.display(),
                reason = failure.reason(),
                "Typeshed source is not on this machine; analysis will not run for this root"
            );
            super::typeshed_status::show_no_source_error(&server.client, &root, failure).await;
        }
        let _ = server
            .typeshed_generations
            .write()
            .await
            .insert(root.clone(), generation.clone());
        if notify {
            super::typeshed_status::notify_generation(&server.client, &root, &generation).await;
        }
    }
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

    // Update test explorer config if present.
    update_test_explorer_config(server, &settings).await;

    // Apply the Type Checking toggle (`basilisk.enabled`) first: the LSP owns
    // diagnostics, so flipping it must clear-or-restore them, and while disabled
    // every scan/publish below is suppressed. Implements [ANALYSIS-ENABLED]
    // (#65/#119). Returning here on a settled-disabled state stops an unrelated
    // setting change (e.g. analysisMode) from re-publishing through the gate.
    if !apply_type_checking_toggle(server, &settings).await {
        return;
    }

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

    // Check-and-set through a READ guard only ([ANALYSIS-PUBLISH], GitHub
    // #264): `WorkspaceIndex::set_mode` is interior-mutable, so a mode flip
    // never queues a WRITE behind a long-running scan's read guard — a queued
    // writer makes tokio's fair `RwLock` block every subsequent reader, which
    // saturates tower-lsp's bounded handler slots and stalls the whole
    // message loop (delaying `didClose` clears past any test/UX budget).
    let changed = {
        let guard = server.index.read().await;
        guard.as_ref().is_some_and(|index| {
            let changed = index.mode() != new_mode;
            if changed {
                index.set_mode(new_mode);
            }
            changed
        })
    };
    if !changed {
        info!(
            ?new_mode,
            "did_change_configuration: mode unchanged, skipping scan"
        );
        return;
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

// Implements [ANALYSIS-ENABLED] — the Type Checking toggle (`basilisk.enabled`).
/// Parse the `basilisk.enabled` toggle from a settings / init-options value.
///
/// Accepts both shapes the editor sends: the flat
/// `initializationOptions = readBasiliskSettings()` (top-level `enabled`) and the
/// `didChangeConfiguration` payload `{ basilisk: { enabled } }` (nested).
fn parse_enabled(value: &serde_json::Value) -> Option<bool> {
    value
        .get("enabled")
        .or_else(|| value.get("basilisk").and_then(|b| b.get("enabled")))
        .and_then(serde_json::Value::as_bool)
}

/// Apply the Type Checking toggle from a `didChangeConfiguration` payload.
///
/// Returns `true` when the caller should continue to analysis-mode handling, and
/// `false` when the toggle fully handled this notification — because it just
/// cleared diagnostics (disabled), re-scanned (re-enabled), or the server is
/// settled-disabled and every subsequent scan/publish must stay suppressed.
/// Implements [ANALYSIS-ENABLED] (GitHub #65 / #119).
async fn apply_type_checking_toggle(server: &LspServer, settings: &serde_json::Value) -> bool {
    let current = server.is_type_checking_enabled().await;

    if let Some(new_enabled) = parse_enabled(settings) {
        if new_enabled != current {
            *server.type_checking_enabled.write().await = new_enabled;
            if new_enabled {
                info!("type checking re-enabled — re-scanning workspace");
                rescan_after_enable(server).await;
            } else {
                info!("type checking disabled — clearing published diagnostics");
                clear_all_diagnostics(server).await;
            }
            return false;
        }
    }

    // No toggle change: proceed to mode handling only while enabled, so an
    // unrelated setting change cannot resurrect diagnostics while disabled.
    current
}

/// Publish empty diagnostics for every indexed URI, clearing stale errors when
/// type checking is switched off. Bypasses the enable gate deliberately — the
/// flag is already `false`, but clearing must still reach the editor.
/// Implements [ANALYSIS-ENABLED] / [ANALYSIS-PUBLISH].
async fn clear_all_diagnostics(server: &LspServer) {
    let guard = server.index.read().await;
    let Some(index) = guard.as_ref() else { return };
    let uris: Vec<Url> = index
        .files
        .iter()
        .filter_map(|entry| Url::from_file_path(entry.key()).ok())
        .collect();
    drop(guard);
    let count = uris.len();
    for uri in uris {
        server.client.publish_diagnostics(uri, vec![], None).await;
    }
    info!(count, "cleared diagnostics for all indexed files");
}

/// Re-publish diagnostics after type checking is re-enabled, matching the active
/// analysis mode. Implements [ANALYSIS-ENABLED].
async fn rescan_after_enable(server: &LspServer) {
    let mode = {
        let guard = server.index.read().await;
        guard.as_ref().map(WorkspaceIndex::mode)
    };
    let Some(mode) = mode else { return };

    match mode {
        AnalysisMode::WholeModule | AnalysisMode::CrossModule => {
            run_workspace_scan(server, mode).await;
        }
        AnalysisMode::OpenFilesOnly => {
            // Only open files are indexed in this mode; re-check and re-publish
            // each one (the gate is already back on, so publishing is correct).
            let guard = server.index.read().await;
            let Some(index) = guard.as_ref() else { return };
            let results = index.recheck_all_files();
            drop(guard);
            for (uri, diags) in results {
                server.publish_diagnostics_if_enabled(uri, diags).await;
            }
        }
    }
}

/// Handle `workspace/didChangeWorkspaceFolders`: update roots and re-scan.
///
/// When the editor adds or removes workspace folders, we update our root list
/// and trigger a fresh workspace scan so import resolution and diagnostics
/// reflect the new folder set.
pub(super) async fn did_change_workspace_folders(
    server: &LspServer,
    params: DidChangeWorkspaceFoldersParams,
) {
    let event = params.event;

    let mut roots = server.workspace_roots.write().await;
    let mut removed_roots = Vec::new();
    let mut added_roots = Vec::new();

    // Remove folders that were closed.
    for removed in &event.removed {
        if let Ok(path) = removed.uri.to_file_path() {
            roots.retain(|r| r != &path);
            removed_roots.push(path);
        }
    }

    // Add newly opened folders.
    for added in &event.added {
        if let Ok(path) = added.uri.to_file_path() {
            if !roots.contains(&path) {
                roots.push(path.clone());
                added_roots.push(path);
            }
        }
    }

    let updated_roots = roots.clone();
    drop(roots);

    info!(
        roots = ?updated_roots,
        added = event.added.len(),
        removed = event.removed.len(),
        "workspace folders changed"
    );

    // Rebuild the workspace index with the new root set.
    let mode = {
        let guard = server.index.read().await;
        guard
            .as_ref()
            .map_or(AnalysisMode::OpenFilesOnly, WorkspaceIndex::mode)
    };

    // Newly added roots get the one-time seed too ([LSPARCH-CONFIG-SEEDING]).
    for root in &updated_roots {
        let _ = crate::config_seed::seed_root_if_unconfigured(root);
    }

    {
        let mut generations = server.typeshed_generations.write().await;
        for root in &removed_roots {
            let _ = generations.remove(root);
        }
    }
    resolve_typeshed_for_roots(server, added_roots, true).await;

    let interpreter = server.python_interpreter.read().await.clone();
    let checker_config = updated_roots
        .first()
        .map(|root| checker_config_for_root(root, interpreter.as_deref()))
        .unwrap_or_default();
    let index = WorkspaceIndex::new(updated_roots, mode, checker_config);
    *server.index.write().await = Some(index);
    install_initial_search_paths(server).await;

    // Re-scan if in a whole-workspace analysis mode.
    if !typeshed_ready_roots(server).await.is_empty()
        && matches!(mode, AnalysisMode::WholeModule | AnalysisMode::CrossModule)
    {
        run_workspace_scan(server, mode).await;
    }
}

// Implements [LSPUV-WATCHERS] (uv.lock, .python-version, pyproject.toml;
// `.venv/pyvenv.cfg` is startup-only detection).
/// Register file watchers for uv-related configuration files.
///
/// Watches `**/uv.lock`, `**/.python-version`, and `**/pyproject.toml` so
/// that the server is notified when these files change on disk.
async fn register_file_watchers(client: &Client) {
    let watchers = vec![
        FileSystemWatcher {
            glob_pattern: GlobPattern::String("**/uv.lock".into()),
            kind: None,
        },
        FileSystemWatcher {
            glob_pattern: GlobPattern::String("**/.python-version".into()),
            kind: None,
        },
        FileSystemWatcher {
            glob_pattern: GlobPattern::String("**/pyproject.toml".into()),
            kind: None,
        },
    ];

    let registration_options =
        serde_json::to_value(DidChangeWatchedFilesRegistrationOptions { watchers });

    let register_options = match registration_options {
        Ok(options) => options,
        Err(err) => {
            tracing::warn!("failed to serialize file watcher registration options: {err}");
            return;
        }
    };

    let registration = Registration {
        id: "uv-file-watchers".to_owned(),
        method: "workspace/didChangeWatchedFiles".to_owned(),
        register_options: Some(register_options),
    };

    if let Err(err) = client.register_capability(vec![registration]).await {
        tracing::warn!("failed to register uv file watchers: {err}");
    } else {
        info!("registered config file watchers (uv.lock, .python-version, pyproject.toml)");
    }
}

/// Scan the whole workspace in a background task and publish diagnostics.
///
/// The scan is ALWAYS spawned: tower-lsp drives every handler on one
/// cooperative task, so a scan computed inside a notification handler starves
/// all other messages — including the `textDocument/didClose` whose clear the
/// editor is waiting on — for the scan's full duration (GitHub #264).
async fn run_workspace_scan(server: &LspServer, _mode: AnalysisMode) {
    let scan_roots = typeshed_ready_roots(server).await;
    let complete_workspace = scan_roots.len() == server.workspace_roots.read().await.len();
    let scan_typeshed = active_typeshed_for_roots(server, &scan_roots).await;
    let scan_index = Arc::clone(&server.index);
    let scan_client = server.client.clone();
    // Clone the toggle so the spawned scan suppresses publication when type
    // checking is off. Implements [ANALYSIS-ENABLED].
    let scan_enabled = Arc::clone(&server.type_checking_enabled);
    let scan_analyze = Arc::clone(&server.analyze_enabled);
    let scan_complete = Arc::clone(&server.initial_scan_complete);
    let scan_python = server.python_interpreter.read().await.clone();
    drop(tokio::spawn(async move {
        let ScanResult {
            diagnostics,
            file_count,
            error_count,
        } = {
            let guard = scan_index.read().await;
            let Some(index) = guard.as_ref() else { return };
            scan_resolve_and_check_with_roots(
                index,
                &scan_roots,
                scan_python.as_deref(),
                scan_typeshed.as_ref(),
            )
        };
        publish_scan_results(
            &scan_index,
            &scan_client,
            &scan_enabled,
            &scan_analyze,
            diagnostics,
        )
        .await;
        // Zero-file rollups are trustworthy from here on. Flip the flag BEFORE
        // notifying, so a refetch triggered by the notification reads
        // `scanComplete: true`. The notification is what settles the client's
        // loading state even when the scan published nothing — a genuinely
        // empty workspace produces no diagnostics events at all
        // ([EXTACT-MODULES-HEADER-LOADING], GitHub #144).
        scan_complete.store(complete_workspace, std::sync::atomic::Ordering::Relaxed);
        scan_client
            .send_notification::<super::activity_panel::ScanCompleteNotification>(
                serde_json::json!({
                    "totalFiles": file_count,
                    "partial": !complete_workspace
                }),
            )
            .await;
        scan_client
            .log_message(
                MessageType::INFO,
                format!(
                    "Basilisk: workspace scan complete — {file_count} files, {error_count} error(s)"
                ),
            )
            .await;
    }));
}

// Implements [ANALYSIS-PUBLISH] — scan results must reflect the index state at
// publish time, not at compute time.
/// Publish scan results, skipping entries that went stale while the scan ran.
///
/// A file closed mid-scan was removed from the index by `did_close`, which
/// also cleared its diagnostics — republishing the scan's snapshot would
/// resurrect them with nothing left to ever clear them (GitHub #264). And
/// after a mid-scan flip to `openFilesOnly`, only open files may publish.
async fn publish_scan_results(
    index: &RwLock<Option<WorkspaceIndex>>,
    client: &Client,
    enabled: &RwLock<bool>,
    analyze: &std::sync::atomic::AtomicBool,
    diagnostics: Vec<(Url, Vec<tower_lsp::lsp_types::Diagnostic>)>,
) {
    for (uri, diags) in diagnostics {
        let still_current = {
            let guard = index.read().await;
            guard
                .as_ref()
                .is_some_and(|idx| scan_entry_still_current(idx, &uri))
        };
        if still_current {
            super::publish_diagnostics_gated(client, enabled, analyze, uri, diags).await;
        } else {
            super::diaglog!("[DIAG] scan publish SKIPPED (stale) uri={uri}");
        }
    }
}

/// Whether a scan result computed for `uri` still reflects the index state:
/// the file is still tracked, and — under `openFilesOnly` — still open.
fn scan_entry_still_current(index: &WorkspaceIndex, uri: &Url) -> bool {
    let Ok(path) = uri.to_file_path() else {
        return false;
    };
    match index.files.get(&path) {
        None => false,
        Some(entry) => !matches!(index.mode(), AnalysisMode::OpenFilesOnly) || entry.is_open,
    }
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
/// This is the single source of truth for the scan+resolve+crossmod pipeline —
/// every scan goes through `run_workspace_scan`'s spawned task, which calls
/// this with cloned data so it can run off the message loop.
fn scan_resolve_and_check_with_roots(
    index: &WorkspaceIndex,
    roots: &[std::path::PathBuf],
    python_interpreter: Option<&std::path::Path>,
    typeshed: Option<&crate::import_resolver::ActiveTypeshed>,
) -> ScanResult {
    let search_paths_by_root = build_search_paths_by_root(roots, python_interpreter, typeshed);
    for (root, search_paths) in &search_paths_by_root {
        info!(
            root = %root.display(),
            site_packages = ?search_paths.site_packages,
            workspace_members = search_paths.workspace_members.len(),
            stub_paths = search_paths.stub_paths.len(),
            has_registry = search_paths.registry.is_some(),
            "built LSP import search paths"
        );
    }
    // Cache the search paths so incremental single-file re-checks (didOpen /
    // didChange) resolve third-party imports identically to this scan, instead
    // of resurrecting false imports_unresolved. Implements [ANALYSIS-INCR-IMPORTS].
    index.set_search_paths_by_root(search_paths_by_root);

    // One pass: the scan primes the salsa engine with the whole workspace and
    // analyses each file exactly once through the memoized queries — imports
    // resolved and, in crossModule mode, `imported_symbols` populated. Every
    // later edit hits the memos this pass created. Implements
    // [CHKARCH-INCREMENTAL-SALSA] adoption / [ANALYSIS-STARTUP-WHOLE].
    let (mut diagnostics, file_count, _initial_error_count) = index.scan_roots(roots);

    // Open files are skipped by the scan (editor text is authoritative), but
    // their pre-scan diagnostics were computed without the search paths —
    // re-analyse them through the engine so open editors converge too.
    diagnostics.extend(index.refresh_open_files_for_roots(roots));

    if matches!(index.mode(), AnalysisMode::CrossModule) {
        // Reverse-edge lookups for cross-file references/rename.
        index.build_import_graph();
        info!("cross-module analysis complete");
    }

    // Recount errors after re-check (resolved imports reduce false E0010s).
    let final_error_count: usize = diagnostics
        .iter()
        .map(|(_, diags)| {
            diags
                .iter()
                .filter(|d| {
                    d.severity
                        .is_none_or(|s| s == tower_lsp::lsp_types::DiagnosticSeverity::ERROR)
                })
                .count()
        })
        .sum();

    ScanResult {
        diagnostics,
        file_count,
        error_count: final_error_count,
    }
}

async fn active_typeshed_for_roots(
    server: &LspServer,
    roots: &[std::path::PathBuf],
) -> Option<crate::import_resolver::ActiveTypeshed> {
    let interpreter = server.python_interpreter.read().await.clone();
    let generations = server.typeshed_generations.read().await;
    let bindings = roots
        .iter()
        .filter_map(|root| {
            let snapshot = Arc::clone(generations.get(root)?.ready_snapshot()?);
            let target = stub_target_for_root(root, interpreter.as_deref());
            Some((root.clone(), snapshot, target))
        })
        .collect();
    crate::import_resolver::ActiveTypeshed::from_roots(bindings)
}

fn stub_target_for_root(
    root: &std::path::Path,
    interpreter: Option<&std::path::Path>,
) -> Option<basilisk_stubs::types::StubTarget> {
    let mut config = crate::config::load_config(root);
    if let Some(interpreter) = interpreter {
        config.python_interpreter = Some(interpreter.to_path_buf());
    }
    if config.python_version.is_none() {
        config.python_version = basilisk_uv::python_version::resolve_target_python_version(root);
    }
    if config.python_platform.is_none() {
        config.python_platform =
            selected_interpreter_platform(root, config.python_interpreter.as_deref());
    }
    crate::import_resolver::stub_target_from_config(&config)
}

fn checker_config_for_root(
    root: &std::path::Path,
    interpreter: Option<&std::path::Path>,
) -> basilisk_config::BasiliskConfig {
    let mut config = basilisk_config::load_basilisk_config(root);
    if config.python_platform.is_none() {
        let analysis_config = crate::config::load_config(root);
        config.python_platform = selected_interpreter_platform(
            root,
            interpreter.or(analysis_config.python_interpreter.as_deref()),
        );
    }
    config
}

fn selected_interpreter_platform(
    root: &std::path::Path,
    interpreter: Option<&std::path::Path>,
) -> Option<String> {
    let selected = interpreter.map_or_else(
        || std::path::PathBuf::from(crate::debug::resolve_python(root)),
        std::path::Path::to_path_buf,
    );
    basilisk_uv::python_version::read_python_platform(&selected)
}

async fn typeshed_ready_roots(server: &LspServer) -> Vec<std::path::PathBuf> {
    let roots = server.workspace_roots.read().await;
    let generations = server.typeshed_generations.read().await;
    roots
        .iter()
        .filter(|root| {
            generations
                .get(*root)
                .and_then(super::typeshed_status::TypeshedGeneration::ready_snapshot)
                .is_some()
        })
        .cloned()
        .collect()
}

fn root_is_ready_for_path(
    roots: &[std::path::PathBuf],
    ready_roots: &[std::path::PathBuf],
    path: &std::path::Path,
) -> bool {
    roots
        .iter()
        .filter(|root| path.starts_with(root))
        .max_by_key(|root| root.components().count())
        .is_none_or(|owner| ready_roots.contains(owner))
}

/// Whether the longest-prefix workspace root owning `uri` has an active
/// Typeshed generation. Files outside every workspace root retain the legacy
/// behavior and are allowed through.
pub(super) async fn typeshed_ready_for_uri(server: &LspServer, uri: &Url) -> bool {
    let Ok(path) = uri.to_file_path() else {
        return true;
    };
    let roots = server.workspace_roots.read().await.clone();
    let generations = server.typeshed_generations.read().await;
    let ready_roots = roots
        .iter()
        .filter(|root| {
            generations
                .get(*root)
                .and_then(super::typeshed_status::TypeshedGeneration::ready_snapshot)
                .is_some()
        })
        .cloned()
        .collect::<Vec<_>>();
    root_is_ready_for_path(&roots, &ready_roots, &path)
}

async fn install_initial_search_paths(server: &LspServer) {
    let roots = server.workspace_roots.read().await.clone();
    let interpreter = server.python_interpreter.read().await.clone();
    let typeshed = active_typeshed_for_roots(server, &roots).await;
    let search_paths =
        build_search_paths_by_root(&roots, interpreter.as_deref(), typeshed.as_ref());
    let guard = server.index.read().await;
    if let Some(index) = guard.as_ref() {
        index.set_search_paths_by_root(search_paths);
    }
}

fn build_search_paths_by_root(
    roots: &[std::path::PathBuf],
    python_interpreter: Option<&std::path::Path>,
    typeshed: Option<&crate::import_resolver::ActiveTypeshed>,
) -> std::collections::HashMap<std::path::PathBuf, Arc<crate::import_resolver::ImportSearchPaths>> {
    roots
        .iter()
        .map(|root| {
            let config = crate::config::load_config(root);
            let search_paths =
                build_root_search_paths(roots, root, config, python_interpreter, typeshed.cloned());
            (root.clone(), Arc::new(search_paths))
        })
        .collect()
}

pub(crate) fn build_root_search_paths(
    roots: &[std::path::PathBuf],
    root: &std::path::Path,
    mut config: crate::config::WorkspaceConfig,
    python_interpreter: Option<&std::path::Path>,
    typeshed: Option<crate::import_resolver::ActiveTypeshed>,
) -> crate::import_resolver::ImportSearchPaths {
    if let Some(interpreter) = python_interpreter {
        config.python_interpreter = Some(interpreter.to_path_buf());
    }
    let root = root.to_path_buf();
    let registry = build_uv_registry(std::slice::from_ref(&root));
    let mut search_paths =
        crate::import_resolver::search_paths_from_config(roots, &config, registry);
    search_paths.typeshed_snapshot = typeshed;
    search_paths
}

// Implements [ANALYSIS-PUBLISH] (runtime mode-switch → clear non-open files;
// per-mode publish and delete→empty live in run_workspace_scan and document.rs).
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
        super::diaglog!("[DIAG] clear_non_open publish n=0 uri={uri}");
        server.client.publish_diagnostics(uri, vec![], None).await;
    }
}

/// Detect a uv project and build a [`PackageRegistry`] from its lock file.
///
/// Returns `None` if this is not a uv project, if there is no lock file, or
/// if the lock file fails to parse. Errors are logged but never fatal — the
/// LSP falls back to registry-free resolution.
pub(crate) fn build_uv_registry(
    roots: &[std::path::PathBuf],
) -> Option<Arc<basilisk_uv::PackageRegistry>> {
    let uv_info = basilisk_uv::detect_uv_project(roots)?;

    if !uv_info.has_lockfile {
        info!(root = %uv_info.root.display(), "uv project detected but no uv.lock — skipping registry");
        return None;
    }

    let lock_path = uv_info.root.join("uv.lock");
    let lock_file = match basilisk_uv::parse_lock_file(&lock_path) {
        Ok(lock) => lock,
        Err(err) => {
            tracing::warn!(
                path = %lock_path.display(),
                error = %err,
                "failed to parse uv.lock — package registry unavailable"
            );
            return None;
        }
    };

    let deps = basilisk_uv::extract_pyproject_deps(&uv_info.root);
    let registry = basilisk_uv::PackageRegistry::from_lock_file(&lock_file, &deps);

    let pkg_count = registry.all_packages().count();
    info!(
        root = %uv_info.root.display(),
        packages = pkg_count,
        direct_deps = deps.len(),
        "built uv package registry"
    );

    Some(Arc::new(registry))
}

// Implements [LSPARCH-UV-HOTRELOAD] and [LSPUV-LOCK-HOT-RELOAD] (full rebuild on
// uv.lock change — no package-level diff; logs a simpler message than the spec)
/// Rebuild the uv package registry and re-resolve all workspace imports.
///
/// Called after uv commands complete or when `uv.lock` changes on disk.
/// Rebuilds the registry from the current `uv.lock`, updates
/// `WorkspaceIndex.registry`, re-resolves imports, and republishes
/// diagnostics for all indexed files.
pub(super) async fn rebuild_registry_and_resolve(server: &LspServer) {
    let guard = server.index.read().await;
    let Some(index) = guard.as_ref() else { return };

    let roots = server.workspace_roots.read().await.clone();
    let interpreter = server.python_interpreter.read().await.clone();
    let typeshed = active_typeshed_for_roots(server, &roots).await;
    let search_paths =
        build_search_paths_by_root(&roots, interpreter.as_deref(), typeshed.as_ref());
    // Refresh the cached search paths so subsequent incremental re-checks pick
    // up the rebuilt registry / venv. Implements [ANALYSIS-INCR-IMPORTS].
    index.set_search_paths_by_root(search_paths);

    // Re-analyse all files through the salsa engine (the changed
    // `SearchPathsInput` invalidates exactly the import-resolving queries) and
    // publish updated diagnostics.
    let results = index.reresolve_imports_and_recheck();

    drop(guard);

    let file_count = results.len();
    for (uri, diags) in results {
        server.publish_diagnostics_if_enabled(uri, diags).await;
    }

    info!(
        files = file_count,
        "registry rebuilt — diagnostics refreshed"
    );
}

/// Update test explorer configuration from `didChangeConfiguration` settings.
async fn update_test_explorer_config(server: &LspServer, settings: &serde_json::Value) {
    let te = settings
        .get("testExplorer")
        .or_else(|| settings.get("basilisk").and_then(|b| b.get("testExplorer")));

    let Some(te) = te else { return };

    let mut config = server.test_config.write().await;
    if let Some(enabled) = te.get("enabled").and_then(serde_json::Value::as_bool) {
        config.enabled = enabled;
    }
    if let Some(framework) = te.get("framework").and_then(serde_json::Value::as_str) {
        framework.clone_into(&mut config.framework);
    }
    if let Some(pytest_path) = te.get("pytestPath").and_then(serde_json::Value::as_str) {
        pytest_path.clone_into(&mut config.pytest_path);
    }
    if let Some(args) = te.get("args").and_then(serde_json::Value::as_array) {
        config.args = args
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
    }
    if let Some(auto_discover) = te
        .get("autoDiscoverOnSave")
        .and_then(serde_json::Value::as_bool)
    {
        config.auto_discover_on_save = auto_discover;
    }
    if let Some(use_uv_run) = te.get("useUvRun").and_then(serde_json::Value::as_bool) {
        config.use_uv_run = use_uv_run;
    }
    info!(?config, "test explorer config updated");
}

/// Spawn initial test discovery in the background so the message loop stays
/// responsive.  This follows the same pattern as `register_file_watchers`.
fn spawn_initial_test_discovery(server: &LspServer) {
    let client = server.client.clone();
    let index = Arc::clone(&server.index);
    let enabled = Arc::clone(&server.type_checking_enabled);
    let analyze = Arc::clone(&server.analyze_enabled);
    let roots_lock = &server.workspace_roots;

    // Read the workspace root synchronously (fast, just a lock read).
    // We can't move the RwLock into the spawned task, so read it here.
    let root = {
        // Safety: we're in an async context but this is a tokio RwLock.
        // Use try_read since we're in a sync fn — if the lock is held,
        // skip discovery rather than blocking.
        let Some(guard) = roots_lock.try_read().ok() else {
            return;
        };
        guard.first().cloned()
    };

    let Some(root) = root else { return };

    drop(tokio::spawn(async move {
        let items = crate::test_discovery::discover_workspace_tests(&root);
        let count: usize = items
            .iter()
            .map(|file_item| file_item.children.len() + 1)
            .sum();
        info!(count, "initial test discovery complete");

        let value = serde_json::json!({ "items": items });
        client
            .send_notification::<super::test_handlers::TestDiscoveryNotification>(value)
            .await;

        // Check pytest availability using the cloned index.
        check_pytest_from_index(&client, &index, &enabled, &analyze, &root).await;
    }));
}

/// Check pytest / pytest-cov availability using a cloned index Arc.
///
/// Standalone helper so `spawn_initial_test_discovery` can run without
/// holding a reference to `LspServer`.
async fn check_pytest_from_index(
    client: &Client,
    index: &Arc<RwLock<Option<WorkspaceIndex>>>,
    enabled: &RwLock<bool>,
    analyze: &std::sync::atomic::AtomicBool,
    root: &std::path::Path,
) {
    // We need workspace roots to detect uv — use root directly.
    let roots = vec![root.to_path_buf()];
    let is_uv = basilisk_uv::detect_uv_project(&roots).is_some();
    if !is_uv {
        return;
    }

    let has_pytest = {
        let guard = index.read().await;
        guard.as_ref().is_none_or(|idx| {
            idx.registry
                .as_ref()
                .is_none_or(|reg| reg.has_package("pytest"))
        })
    };

    if !has_pytest {
        client
            .log_message(
                MessageType::WARNING,
                "Basilisk: test runner \"pytest\" is not installed — use the quick fix (uv add --dev pytest)"
                    .to_owned(),
            )
            .await;

        let test_files = crate::test_discovery::discover_test_files(root);
        for path in test_files {
            let Ok(uri) = Url::from_file_path(&path) else {
                continue;
            };
            let diag = super::test_handlers::make_pytest_not_found_diagnostic();
            // Suppressed while type checking is off ([ANALYSIS-ENABLED]).
            super::publish_diagnostics_gated(client, enabled, analyze, uri, vec![diag]).await;
        }
    }

    let has_pytest_cov = {
        let guard = index.read().await;
        guard.as_ref().is_none_or(|idx| {
            idx.registry
                .as_ref()
                .is_none_or(|reg| reg.has_package("pytest_cov"))
        })
    };

    if !has_pytest_cov {
        client
            .log_message(
                MessageType::INFO,
                "Basilisk: pytest-cov not found in uv.lock — install it for coverage support"
                    .to_owned(),
            )
            .await;
    }
}

/// Handle the `shutdown` request: stop all debug and profiling sessions and
/// the server-owned configuration watcher ([LSPARCH-CONFIG]).
pub(super) async fn shutdown(server: &LspServer) -> LspResult<()> {
    server.debug_manager.stop_all().await;
    server.profiler_manager.stop_all().await;
    if let Some(watcher) = server.config_watcher.lock().await.take() {
        watcher.abort();
    }
    Ok(())
}

#[cfg(test)]
mod explicit_python_tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use tower_lsp::lsp_types::NumberOrString;

    use super::*;

    #[test]
    fn initialization_option_reads_nested_python_and_ignores_empty_auto_detect() {
        assert_eq!(
            parse_python_interpreter(&serde_json::json!({
                "basilisk": { "python": "/opt/python-target/bin/python" }
            })),
            Some(std::path::PathBuf::from("/opt/python-target/bin/python"))
        );
        assert!(parse_python_interpreter(&serde_json::json!({
            "basilisk": { "python": "" }
        }))
        .is_none());
    }

    #[test]
    fn startup_target_uses_python_version_file_without_selecting_a_typeshed_commit() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let root = std::env::temp_dir().join(format!(
            "basilisk_lsp_python_version_target_{}_{}",
            std::process::id(),
            unique
        ));
        let setup = std::fs::create_dir_all(&root)
            .and_then(|()| std::fs::write(root.join(".python-version"), b"3.14\n"));
        assert!(setup.is_ok(), "fixture setup failed: {setup:?}");
        if setup.is_err() {
            return;
        }

        let target = stub_target_for_root(&root, None);
        assert_eq!(target.map(|target| target.python_version), Some((3, 14)));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn startup_target_uses_selected_interpreter_platform() {
        use std::os::unix::fs::PermissionsExt;

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let root = std::env::temp_dir().join(format!(
            "basilisk_lsp_python_platform_target_{}_{}",
            std::process::id(),
            unique
        ));
        let interpreter = root.join("python");
        let setup = std::fs::create_dir_all(&root)
            .and_then(|()| std::fs::write(root.join(".python-version"), b"3.14\n"))
            .and_then(|()| {
                std::fs::write(&interpreter, b"#!/bin/sh\nprintf 'fixture-platform\\n'\n")
            })
            .and_then(|()| {
                std::fs::set_permissions(&interpreter, std::fs::Permissions::from_mode(0o755))
            });
        assert!(setup.is_ok(), "fixture setup failed: {setup:?}");
        if setup.is_err() {
            return;
        }

        let target = stub_target_for_root(&root, Some(&interpreter));

        assert_eq!(
            target.map(|target| target.platform),
            Some(basilisk_stubs::types::StubTargetPlatform::Concrete(
                "fixture-platform".to_owned()
            ))
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// [TYPESHEDRT-ACCEPTANCE-TARGET]: the editor's explicit Python binary
    /// reaches the startup scan and selects that interpreter's site-packages.
    /// This exercises the complete initialization-option → `WorkspaceConfig` →
    /// import-search path rather than only the ambient detector.
    #[test]
    fn startup_scan_resolves_from_explicit_interpreter_site_packages() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let base = std::env::temp_dir().join(format!(
            "basilisk_lsp_explicit_python_{}_{}",
            std::process::id(),
            unique
        ));
        let root = base.join("workspace");
        let prefix = base.join("target-python");
        let interpreter = prefix.join("bin/python");
        let package = prefix
            .join("lib/python9.9/site-packages")
            .join("bsk_explicit_target_only_991");

        let setup = std::fs::create_dir_all(&root)
            .and_then(|()| std::fs::create_dir_all(interpreter.parent().unwrap_or(&prefix)))
            .and_then(|()| std::fs::create_dir_all(&package))
            .and_then(|()| std::fs::write(&interpreter, b"fake interpreter fixture\n"))
            .and_then(|()| {
                std::fs::write(
                    root.join("main.py"),
                    b"import bsk_explicit_target_only_991\n",
                )
            })
            .and_then(|()| std::fs::write(package.join("py.typed"), b""))
            .and_then(|()| std::fs::write(package.join("__init__.pyi"), b"answer: int\n"));
        assert!(setup.is_ok(), "fixture setup failed: {setup:?}");
        if setup.is_err() {
            return;
        }

        let index = WorkspaceIndex::new(
            vec![root.clone()],
            AnalysisMode::WholeModule,
            basilisk_config::BasiliskConfig::default(),
        );
        let result = scan_resolve_and_check_with_roots(
            &index,
            std::slice::from_ref(&root),
            Some(&interpreter),
            None,
        );
        let unresolved = result.diagnostics.iter().any(|(_, diagnostics)| {
            diagnostics.iter().any(|diagnostic| {
                matches!(
                    diagnostic.code.as_ref(),
                    Some(NumberOrString::String(code)) if code == "imports_unresolved"
                )
            })
        });
        assert!(
            !unresolved,
            "explicit target package must resolve: {:?}",
            result.diagnostics
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    /// [TYPESHEDRT-ACCEPTANCE-TARGET]: each workspace root owns its target
    /// interpreter. A package installed only for root A must not leak into
    /// root B, while both roots still resolve packages from their own target.
    #[test]
    fn startup_scan_isolates_target_packages_per_workspace_root() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let base = std::env::temp_dir().join(format!(
            "basilisk_lsp_multi_root_python_{}_{}",
            std::process::id(),
            unique
        ));
        let root_a = base.join("root-a");
        let root_b = base.join("root-b");
        let prefix_a = base.join("python-a");
        let prefix_b = base.join("python-b");
        let interpreter_a = prefix_a.join("bin/python");
        let interpreter_b = prefix_b.join("bin/python");
        let package_a = prefix_a
            .join("lib/python9.9/site-packages")
            .join("bsk_root_a_only_991");
        let package_b = prefix_b
            .join("lib/python9.9/site-packages")
            .join("bsk_root_b_only_992");

        let config_a = format!("[tool.basilisk]\npython = '{}'\n", interpreter_a.display());
        let config_b = format!("[tool.basilisk]\npython = '{}'\n", interpreter_b.display());
        let setup = std::fs::create_dir_all(&root_a)
            .and_then(|()| std::fs::create_dir_all(&root_b))
            .and_then(|()| std::fs::create_dir_all(interpreter_a.parent().unwrap_or(&prefix_a)))
            .and_then(|()| std::fs::create_dir_all(interpreter_b.parent().unwrap_or(&prefix_b)))
            .and_then(|()| std::fs::create_dir_all(&package_a))
            .and_then(|()| std::fs::create_dir_all(&package_b))
            .and_then(|()| std::fs::write(&interpreter_a, b"fake interpreter A\n"))
            .and_then(|()| std::fs::write(&interpreter_b, b"fake interpreter B\n"))
            .and_then(|()| std::fs::write(root_a.join("pyproject.toml"), config_a))
            .and_then(|()| std::fs::write(root_b.join("pyproject.toml"), config_b))
            .and_then(|()| std::fs::write(root_a.join("own.py"), b"import bsk_root_a_only_991\n"))
            .and_then(|()| std::fs::write(root_b.join("own.py"), b"import bsk_root_b_only_992\n"))
            .and_then(|()| {
                std::fs::write(root_b.join("foreign.py"), b"import bsk_root_a_only_991\n")
            })
            .and_then(|()| std::fs::write(package_a.join("py.typed"), b""))
            .and_then(|()| std::fs::write(package_a.join("__init__.pyi"), b"answer: int\n"))
            .and_then(|()| std::fs::write(package_b.join("py.typed"), b""))
            .and_then(|()| std::fs::write(package_b.join("__init__.pyi"), b"answer: int\n"));
        assert!(setup.is_ok(), "fixture setup failed: {setup:?}");
        if setup.is_err() {
            return;
        }

        let roots = vec![root_a.clone(), root_b.clone()];
        let index = WorkspaceIndex::new(
            roots.clone(),
            AnalysisMode::WholeModule,
            basilisk_config::BasiliskConfig::default(),
        );
        let result = scan_resolve_and_check_with_roots(&index, &roots, None, None);
        let has_unresolved = |path: &std::path::Path| {
            result.diagnostics.iter().any(|(uri, diagnostics)| {
                uri.to_file_path().as_deref() == Ok(path)
                    && diagnostics.iter().any(|diagnostic| {
                        matches!(
                            diagnostic.code.as_ref(),
                            Some(NumberOrString::String(code)) if code == "imports_unresolved"
                        )
                    })
            })
        };

        assert!(
            !has_unresolved(&root_a.join("own.py")),
            "root A must resolve its own target package"
        );
        assert!(
            !has_unresolved(&root_b.join("own.py")),
            "root B must resolve its own target package"
        );
        assert!(
            has_unresolved(&root_b.join("foreign.py")),
            "root B must not inherit root A's target package"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn startup_resolves_custom_typeshed_before_analysis() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let base = std::env::temp_dir().join(format!(
            "basilisk_lsp_initial_typeshed_{}_{}",
            std::process::id(),
            unique
        ));
        let root = base.join("workspace");
        let custom = base.join("custom-typeshed");
        let setup = std::fs::create_dir_all(&root)
            .and_then(|()| std::fs::create_dir_all(custom.join("stdlib")))
            .and_then(|()| std::fs::write(custom.join("stdlib/os.pyi"), b"name: str\n"));
        assert!(setup.is_ok(), "fixture setup failed: {setup:?}");
        if setup.is_err() {
            return;
        }

        let config = crate::config::WorkspaceConfig {
            typeshed_path: Some(custom),
            ..crate::config::WorkspaceConfig::default()
        };
        let generation = crate::configuration_editor::resolve_workspace(config).await;
        let snapshot = generation.ready_snapshot();
        assert!(
            snapshot.is_some(),
            "custom startup resolution: {generation:?}"
        );
        let Some(snapshot) = snapshot.cloned() else {
            let _ = std::fs::remove_dir_all(&base);
            return;
        };

        assert_eq!(
            snapshot.status.active_source,
            basilisk_stubs::typeshed::source::SourceKind::Custom
        );
        assert!(snapshot.read_stub("os").is_some());

        let expected_identity = snapshot.identity.uri_component();
        let active = crate::import_resolver::ActiveTypeshed::from_roots(vec![(
            root.clone(),
            snapshot,
            None,
        )]);
        assert!(active.is_some(), "one root binding");
        let Some(active) = active else {
            let _ = std::fs::remove_dir_all(&base);
            return;
        };
        let index = WorkspaceIndex::new(
            vec![root.clone()],
            AnalysisMode::WholeModule,
            basilisk_config::BasiliskConfig::default(),
        );
        let _ = scan_resolve_and_check_with_roots(
            &index,
            std::slice::from_ref(&root),
            None,
            Some(&active),
        );
        let installed_identity = index
            .search_paths_for_file(&root)
            .map(|(_, paths)| paths)
            .and_then(|paths| paths.typeshed_snapshot.clone())
            .map(|active| active.identity_fingerprint());
        assert_eq!(
            installed_identity.as_deref(),
            Some(expected_identity.as_str())
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    /// [STUBRES-TYPESHED-OFFLINE]: readiness follows the longest owning root;
    /// a ready parent must not authorize analysis inside its blocked child.
    #[test]
    fn nested_workspace_typeshed_readiness_uses_the_longest_owner() {
        let parent = std::path::PathBuf::from("/workspace");
        let child = parent.join("nested");
        let roots = vec![parent.clone(), child.clone()];

        assert!(!root_is_ready_for_path(
            &roots,
            std::slice::from_ref(&parent),
            &child.join("blocked.py")
        ));
        assert!(!root_is_ready_for_path(
            &roots,
            std::slice::from_ref(&child),
            &parent.join("blocked.py")
        ));
        assert!(root_is_ready_for_path(
            &roots,
            std::slice::from_ref(&child),
            &child.join("ready.py")
        ));
        assert!(root_is_ready_for_path(
            &roots,
            &[],
            std::path::Path::new("/outside/file.py")
        ));
    }
}
