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

    // Store workspace roots for later use by import resolution.
    (*server.workspace_roots.write().await).clone_from(&roots);

    // Load project-level checker config (pyproject.toml / basilisk.json) so
    // that rule severity overrides, per-module, and per-path settings match
    // the CLI.
    let checker_config = roots
        .first()
        .map(|r| basilisk_config::load_basilisk_config(r))
        .unwrap_or_default();

    // Build the workspace index now so `initialized()` can scan immediately.
    let index = WorkspaceIndex::new(roots, mode, checker_config);
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

    // Spawn file watcher registration in the background so it never blocks
    // the message loop (register_capability sends a request to the client).
    let client = server.client.clone();
    drop(tokio::spawn(async move {
        register_file_watchers(&client).await;
    }));

    // Read the analysis mode, then release the lock immediately so the
    // message loop is not blocked while the workspace scan runs.
    let mode = {
        let guard = server.index.read().await;
        guard.as_ref().map(|idx| idx.mode)
    };

    let Some(mode) = mode else { return };

    match mode {
        // Implements [ANALYSIS-STARTUP-OPEN]
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

            // Spawn the workspace scan in the background so the server can
            // respond to requests (documentSymbol, etc.) while scanning.
            // tower-lsp v0.20 processes messages sequentially — blocking here
            // prevents ALL request handling until the scan completes.
            let scan_client = server.client.clone();
            let scan_index = Arc::clone(&server.index);
            // Clone the toggle so the spawned scan suppresses publication when a
            // client started with type checking off. Implements [ANALYSIS-ENABLED].
            let scan_enabled = Arc::clone(&server.type_checking_enabled);
            let scan_roots = {
                let roots = server.workspace_roots.read().await;
                roots.clone()
            };
            drop(tokio::spawn(async move {
                let guard = scan_index.read().await;
                if let Some(index) = guard.as_ref() {
                    let scan_result = scan_resolve_and_check_with_roots(index, &scan_roots);
                    drop(guard);
                    for (uri, diags) in scan_result.diagnostics {
                        super::publish_diagnostics_gated(&scan_client, &scan_enabled, uri, diags)
                            .await;
                    }
                    scan_client
                        .log_message(
                            MessageType::INFO,
                            format!(
                                "Basilisk: workspace scan complete — {} files, {} error(s)",
                                scan_result.file_count, scan_result.error_count
                            ),
                        )
                        .await;
                }
            }));
        }
    }

    // Spawn initial test discovery in the background.
    spawn_initial_test_discovery(server);
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

    // Check whether the mode actually changed — skip the expensive write
    // lock + workspace scan if it hasn't.  The background scan spawned by
    // `initialized()` holds a READ lock on the index; acquiring a WRITE
    // lock here would block the entire message loop until that scan
    // finishes, preventing `textDocument/didOpen` and other requests from
    // being processed.
    let current_mode = {
        let guard = server.index.read().await;
        guard.as_ref().map(|idx| idx.mode)
    };
    if current_mode == Some(new_mode) {
        info!(
            ?new_mode,
            "did_change_configuration: mode unchanged, skipping scan"
        );
        return;
    }

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
        guard.as_ref().map(|idx| idx.mode)
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

    // Remove folders that were closed.
    for removed in &event.removed {
        if let Ok(path) = removed.uri.to_file_path() {
            roots.retain(|r| r != &path);
        }
    }

    // Add newly opened folders.
    for added in &event.added {
        if let Ok(path) = added.uri.to_file_path() {
            if !roots.contains(&path) {
                roots.push(path);
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
            .map_or(AnalysisMode::OpenFilesOnly, |idx| idx.mode)
    };

    let checker_config = updated_roots
        .first()
        .map(|r| basilisk_config::load_basilisk_config(r))
        .unwrap_or_default();
    let index = WorkspaceIndex::new(updated_roots, mode, checker_config);
    *server.index.write().await = Some(index);

    // Re-scan if in a whole-workspace analysis mode.
    if matches!(mode, AnalysisMode::WholeModule | AnalysisMode::CrossModule) {
        run_workspace_scan(server, mode).await;
    }
}

// Implements [LSPUV-WATCHERS] (uv.lock, .python-version, pyproject.toml,
// basilisk.json; the spec's `.venv/pyvenv.cfg` row is not watched).
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
        FileSystemWatcher {
            glob_pattern: GlobPattern::String("**/basilisk.json".into()),
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
        info!("registered config file watchers (uv.lock, .python-version, pyproject.toml, basilisk.json)");
    }
}

/// Scan the whole workspace and publish diagnostics for all files.
async fn run_workspace_scan(server: &LspServer, _mode: AnalysisMode) {
    let guard = server.index.read().await;
    let Some(index) = guard.as_ref() else { return };
    let scan_result = scan_resolve_and_check(server, index).await;
    drop(guard);

    // Respect the Type Checking toggle ([ANALYSIS-ENABLED]): suppress publication
    // when disabled instead of flooding the editor with scan diagnostics.
    for (uri, diags) in scan_result.diagnostics {
        server.publish_diagnostics_if_enabled(uri, diags).await;
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
    let roots = server.workspace_roots.read().await;
    let result = scan_resolve_and_check_with_roots(index, &roots);
    drop(roots);
    result
}

/// Core scan logic that takes roots directly instead of `&LspServer`.
///
/// This allows the scan to run in a spawned task with cloned data.
fn scan_resolve_and_check_with_roots(
    index: &WorkspaceIndex,
    roots: &[std::path::PathBuf],
) -> ScanResult {
    let (_results, file_count, _initial_error_count) = index.scan();

    // Resolve imports for all scanned files.
    let config = roots
        .first()
        .map(|r| crate::config::load_config(r))
        .unwrap_or_default();

    // Detect uv project, build package registry and discover workspace members.
    let registry = build_uv_registry(roots);
    let search_paths = crate::import_resolver::search_paths_from_config(roots, &config, registry);
    info!(
        site_packages = ?search_paths.site_packages,
        workspace_members = search_paths.workspace_members.len(),
        stub_paths = search_paths.stub_paths.len(),
        has_registry = search_paths.registry.is_some(),
        "built LSP import search paths"
    );
    // Cache the search paths so incremental single-file re-checks (didOpen /
    // didChange) resolve third-party imports identically to this scan, instead
    // of resurrecting false imports_unresolved. Implements [ANALYSIS-INCR-IMPORTS].
    index.set_search_paths(search_paths);

    // Re-analyse every scanned file through the salsa engine now that the
    // search paths are known. The initial scan() generates diagnostics before
    // workspace members are known, so imports_unresolved fires for imports that
    // are actually resolvable — the sweep replaces those results with
    // import-resolved (and, in crossModule mode, cross-module-populated)
    // diagnostics, and primes the engine so subsequent edits are incremental.
    // Implements [CHKARCH-INCREMENTAL-SALSA] adoption.
    let diagnostics = index.reresolve_imports_and_recheck();
    if matches!(index.mode, AnalysisMode::CrossModule) {
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
        server.client.publish_diagnostics(uri, vec![], None).await;
    }
}

/// Detect a uv project and build a [`PackageRegistry`] from its lock file.
///
/// Returns `None` if this is not a uv project, if there is no lock file, or
/// if the lock file fails to parse. Errors are logged but never fatal — the
/// LSP falls back to registry-free resolution.
fn build_uv_registry(roots: &[std::path::PathBuf]) -> Option<Arc<basilisk_uv::PackageRegistry>> {
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

    let roots = server.workspace_roots.read().await;
    let config = roots
        .first()
        .map(|r| crate::config::load_config(r))
        .unwrap_or_default();

    let registry = build_uv_registry(&roots);
    let search_paths = crate::import_resolver::search_paths_from_config(&roots, &config, registry);
    // Refresh the cached search paths so subsequent incremental re-checks pick
    // up the rebuilt registry / venv. Implements [ANALYSIS-INCR-IMPORTS].
    index.set_search_paths(search_paths);
    drop(roots);

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
        check_pytest_from_index(&client, &index, &enabled, &root).await;
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
                "Basilisk: pytest not found in uv.lock — use the quick fix to install it"
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
            super::publish_diagnostics_gated(client, enabled, uri, vec![diag]).await;
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

/// Handle the `shutdown` request: stop all debug and profiling sessions.
pub(super) async fn shutdown(server: &LspServer) -> LspResult<()> {
    server.debug_manager.stop_all().await;
    server.profiler_manager.stop_all().await;
    Ok(())
}
