//! Implements [LSPARCH-ARCH-MODSTRUCT]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-MODSTRUCT
//!
//! Language Server Protocol server implementation for Basilisk.
//!
//! Thin dispatcher that delegates to feature modules for each LSP request.

use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::task::AbortHandle;

pub(super) mod activity_panel;
pub(super) mod adoption;
mod command_configuration;
mod command_fixes;
pub(super) mod commands;
pub(super) mod document;
pub(super) mod handlers;
pub(super) mod init;
pub(super) mod memory_handlers;
pub(super) mod profiler_handlers;
pub(super) mod refactor_commands;
pub(super) mod resolved_env;
pub(super) mod stub_handlers;
pub(super) mod test_handlers;
pub(super) mod typeshed_document;
pub(super) mod typeshed_status;
pub(super) mod uv_handlers;

macro_rules! diaglog {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/basilisk-diag.log")
        {
            // Millisecond timestamp + pid, and a single write per line so
            // concurrent handlers (and concurrent server processes in tests)
            // cannot shred each other's lines mid-string.
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let line = format!($($arg)*);
            let _ = writeln!(f, "[{timestamp} p{}] {line}", std::process::id());
        }
    }};
}

pub(crate) use diaglog;

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    CodeActionParams, CodeActionResponse, CodeLens, CodeLensParams, ColorInformation,
    ColorPresentation, ColorPresentationParams, CompletionItem, CompletionParams,
    CompletionResponse, Diagnostic, DidChangeConfigurationParams, DidChangeTextDocumentParams,
    DidChangeWatchedFilesParams, DidChangeWorkspaceFoldersParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentColorParams,
    DocumentFormattingParams, DocumentHighlight, DocumentHighlightParams,
    DocumentRangeFormattingParams, DocumentSymbolParams, DocumentSymbolResponse,
    ExecuteCommandParams, FoldingRange, FoldingRangeParams, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverParams, InitializeParams, InitializeResult,
    InitializedParams, InlayHint, InlayHintParams, Location, Position, PrepareRenameResponse,
    ReferenceParams, RenameFilesParams, RenameParams, SelectionRange, SelectionRangeParams,
    SemanticTokensParams, SemanticTokensResult, SignatureHelpParams, SymbolInformation,
    TextDocumentPositionParams, TextEdit, TypeHierarchyItem, TypeHierarchyPrepareParams,
    TypeHierarchySubtypesParams, TypeHierarchySupertypesParams, Url, WorkspaceEdit,
    WorkspaceSymbolParams,
};
use tower_lsp::{Client, LspService, Server};

use crate::workspace::WorkspaceIndex;

// Implements [ANALYSIS-INCR-DEBOUNCE]
/// Debounce interval for file-watcher notifications (milliseconds).
pub(super) const FILE_WATCHER_DEBOUNCE_MS: u64 = 200;

/// The JSON-RPC error returned when a command needs a workspace root but none
/// is available. Shared by the uv and stub command handlers.
pub(super) fn no_workspace_root_error() -> tower_lsp::jsonrpc::Error {
    tower_lsp::jsonrpc::Error {
        code: tower_lsp::jsonrpc::ErrorCode::ServerError(-32010),
        message: "No workspace root available".into(),
        data: None,
    }
}

/// Debounce interval for `basilisk/moduleChanged` notifications (milliseconds).
pub(super) const MODULE_CHANGED_DEBOUNCE_MS: u64 = 300;

// Implements [LSPTEST-CONFIGURATION-SETTINGS]
/// Runtime test explorer configuration received from the client.
#[derive(Debug, Clone)]
pub(super) struct TestExplorerConfig {
    /// Whether test discovery is enabled.
    pub(super) enabled: bool,
    /// Test framework: `pytest`, `unittest`, or `auto`.
    pub(super) framework: String,
    /// Path to the pytest executable.
    pub(super) pytest_path: String,
    /// Additional test runner arguments.
    pub(super) args: Vec<String>,
    /// Re-discover tests on file save.
    pub(super) auto_discover_on_save: bool,
    /// Use `uv run` when a uv project is detected.
    pub(super) use_uv_run: bool,
}

impl Default for TestExplorerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            framework: "auto".to_owned(),
            pytest_path: "pytest".to_owned(),
            args: Vec::new(),
            auto_discover_on_save: true,
            use_uv_run: true,
        }
    }
}

/// The Basilisk LSP server.
pub struct LspServer {
    /// LSP client for sending notifications back to the editor.
    pub(super) client: Client,
    /// Workspace index (None until initialized).
    pub(super) index: Arc<RwLock<Option<WorkspaceIndex>>>,
    /// Workspace root folders discovered during initialization. Shared as an
    /// `Arc` so the server-owned configuration watcher ([LSPARCH-CONFIG])
    /// follows root changes after the handler returns.
    pub(super) workspace_roots: Arc<RwLock<Vec<std::path::PathBuf>>>,
    /// Root-keyed runtime Typeshed generations shared by analysis and editor
    /// surfaces. Acquisition populates each root before its analysis starts;
    /// refreshes replace one entry only after every activation gate passes.
    pub(super) typeshed_generations: Arc<
        RwLock<std::collections::BTreeMap<std::path::PathBuf, typeshed_status::TypeshedGeneration>>,
    >,
    /// Editor-selected Python binary. This is kept separately from project
    /// configuration because `initializationOptions.basilisk.python` has
    /// higher precedence and must survive every search-path rebuild.
    pub(super) python_interpreter: Arc<RwLock<Option<std::path::PathBuf>>>,
    // Implements [LSPDEBUG-WIRE] (DebugSessionManager added to LspServer)
    /// Debug session manager — spawns debugpy and tracks active sessions.
    pub(super) debug_manager: Arc<crate::debug::DebugSessionManager>,
    /// Profiler session manager — py-spy sampling, aggregation, export.
    pub(super) profiler_manager: crate::profiler::ProfileSessionManager,
    /// Memory session manager — drives the editor-couriered ingest round-trip
    /// (snapshot/diff/leak state) since the LSP holds no DAP connection.
    pub(super) memory_manager: crate::profiler::memory::session::MemorySessionManager,
    /// Debounced file-watcher task.
    pub(super) watcher_debounce: Mutex<Option<AbortHandle>>,
    /// Debounced module-changed notification task.
    pub(super) module_changed_debounce: Mutex<Option<AbortHandle>>,
    /// Test explorer configuration from the client.
    pub(super) test_config: RwLock<TestExplorerConfig>,
    // Implements [ANALYSIS-ENABLED] (the `basilisk.enabled` / "Type Checking"
    // toggle). The LSP is authoritative for diagnostics in the default mode, so
    // when the toggle is off the server clears published diagnostics and stops
    // publishing new ones — instead of leaving stale errors on screen.
    // GitHub #65 / #119. Shared as an `Arc` so debounced/spawned publish tasks
    // (file-watcher, startup scan) can read it after the handler returns.
    /// Whether type checking (diagnostic publication) is enabled.
    pub(super) type_checking_enabled: Arc<RwLock<bool>>,
    // Implements [LSPARCH-DIAGNOSTIC-SCOPE]: the LSP publishes the union of
    // both command scopes by default; the IDE-level initialization option
    // `basilisk.analyze = false` restricts publication to check scope
    // (`pep`-tagged rules only). Per-user editor ergonomics — project
    // configuration grades rules and never selects commands.
    /// Whether analyze-scope diagnostics are published (default true).
    pub(super) analyze_enabled: Arc<std::sync::atomic::AtomicBool>,
    // Implements [LSPFMT-CONFIG] (`basilisk.formatter`): when the client or
    // config selects `"none"`, formatting capabilities are not advertised and
    // the handlers answer `None` even if a client calls them anyway.
    /// Whether formatting (whole-document and range) is enabled.
    pub(super) formatting_enabled: std::sync::atomic::AtomicBool,
    // Implements [EXTACT-MODULES-HEADER-LOADING] (GitHub #144): clients must be
    // able to tell "not scanned yet" apart from "genuinely zero Python files",
    // so the server tracks whether a workspace scan has finished and stamps it
    // into every `basilisk.workspaceModules` rollup. Shared as an `Arc` so the
    // spawned scan task can flip it after the handler returns.
    /// Whether the initial workspace scan has completed.
    pub(super) initial_scan_complete: Arc<std::sync::atomic::AtomicBool>,
    /// Revision-checked configuration previews awaiting an explicit apply.
    /// Shared as an `Arc` so the server-owned configuration watcher
    /// ([LSPARCH-CONFIG]) runs the same refresh tail from its own task.
    pub(crate) configuration_editor: Arc<crate::configuration_editor::ConfigurationEditorState>,
    // Implements [LSPARCH-CONFIG]: the server watches the active
    // configuration source itself — reactivity never depends on the client
    // supporting `workspace/didChangeWatchedFiles` (Zed advertises no file
    // watchers at all; see docs/specs/ZED-SPEC.md).
    /// The server-owned configuration watcher task, aborted on shutdown.
    pub(super) config_watcher: Mutex<Option<AbortHandle>>,
}

impl std::fmt::Debug for LspServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspServer").finish_non_exhaustive()
    }
}

impl LspServer {
    /// Create a new LSP server instance.
    #[must_use]
    pub fn new(client: Client) -> Self {
        Self {
            client,
            index: Arc::new(RwLock::new(None)),
            workspace_roots: Arc::new(RwLock::new(Vec::new())),
            typeshed_generations: Arc::new(RwLock::new(std::collections::BTreeMap::new())),
            python_interpreter: Arc::new(RwLock::new(None)),
            debug_manager: Arc::new(crate::debug::DebugSessionManager::new()),
            profiler_manager: crate::profiler::ProfileSessionManager::new(),
            memory_manager: crate::profiler::memory::session::MemorySessionManager::new(),
            watcher_debounce: Mutex::new(None),
            module_changed_debounce: Mutex::new(None),
            test_config: RwLock::new(TestExplorerConfig::default()),
            // Type checking is on by default; the client opts out via
            // `basilisk.enabled = false`. Implements [ANALYSIS-ENABLED].
            type_checking_enabled: Arc::new(RwLock::new(true)),
            // Both scopes publish by default; the client opts out of analyze
            // scope via `basilisk.analyze = false`. [LSPARCH-DIAGNOSTIC-SCOPE]
            analyze_enabled: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            // Formatting is on by default (`basilisk.formatter = "ruff"`);
            // `initialize` flips this off for `"none"`. [LSPFMT-CONFIG]
            formatting_enabled: std::sync::atomic::AtomicBool::new(true),
            // No scan has run yet: zero-file rollups are NOT trustworthy until
            // the first scan completes. [EXTACT-MODULES-HEADER-LOADING], #144.
            initial_scan_complete: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            configuration_editor: Arc::new(
                crate::configuration_editor::ConfigurationEditorState::default(),
            ),
            config_watcher: Mutex::new(None),
        }
    }

    /// Cloneable handles for the shared configuration refresh tail, so the
    /// server-owned configuration watcher can run it from a spawned task.
    /// Implements [LSPARCH-CONFIG].
    pub(crate) fn refresh_handles(
        &self,
    ) -> crate::configuration_editor::ConfigurationRefreshHandles {
        crate::configuration_editor::ConfigurationRefreshHandles {
            index: Arc::clone(&self.index),
            client: self.client.clone(),
            type_checking_enabled: Arc::clone(&self.type_checking_enabled),
            analyze_enabled: Arc::clone(&self.analyze_enabled),
            workspace_roots: Arc::clone(&self.workspace_roots),
            python_interpreter: Arc::clone(&self.python_interpreter),
            typeshed_generations: Arc::clone(&self.typeshed_generations),
            initial_scan_complete: Arc::clone(&self.initial_scan_complete),
            configuration_editor: Arc::clone(&self.configuration_editor),
        }
    }

    /// Whether formatting is enabled ([LSPFMT-CONFIG]).
    pub(super) fn is_formatting_enabled(&self) -> bool {
        self.formatting_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// The workspace's formatter style options ([LSPFMT-ENGINE]), read from
    /// the first workspace root's `pyproject.toml`.
    pub(super) async fn format_style(&self) -> crate::config::FormatStyle {
        self.workspace_roots
            .read()
            .await
            .first()
            .map(|root| crate::config::load_format_style(root))
            .unwrap_or_default()
    }

    /// Whether type checking (diagnostic publication) is currently enabled.
    /// Implements [ANALYSIS-ENABLED].
    pub(super) async fn is_type_checking_enabled(&self) -> bool {
        *self.type_checking_enabled.read().await
    }

    /// Publish diagnostics only while type checking is enabled.
    ///
    /// Every live diagnostic-producing path routes through here so that the
    /// `basilisk.enabled` toggle is honoured uniformly. Clearing (publishing an
    /// empty set on disable) deliberately bypasses this gate via
    /// [`publish_diagnostics_gated`] with `force_clear`. Implements
    /// [ANALYSIS-ENABLED] / [ANALYSIS-PUBLISH].
    pub(super) async fn publish_diagnostics_if_enabled(&self, uri: Url, diags: Vec<Diagnostic>) {
        publish_diagnostics_gated(
            &self.client,
            &self.type_checking_enabled,
            &self.analyze_enabled,
            uri,
            diags,
        )
        .await;
    }

    /// Borrow the index and call `f` with it. Returns `None` if not yet
    /// initialized (before `initialized()` fires).
    pub(super) async fn with_index<T, F>(&self, f: F) -> Option<T>
    where
        F: FnOnce(&WorkspaceIndex) -> Option<T>,
    {
        let guard = self.index.read().await;
        guard.as_ref().and_then(f)
    }

    /// Get the text, resolved module, and diagnostics for a document.
    pub(super) async fn get_document_data(
        &self,
        uri: &Url,
    ) -> Option<(
        String,
        Arc<basilisk_resolver::ResolvedModule>,
        Vec<basilisk_checker::Diagnostic>,
    )> {
        self.with_index(|idx| idx.get_by_uri(uri)).await
    }

    /// Get the text and resolved module for a DISPLAY surface, falling back to
    /// the last revision that parsed. See [`WorkspaceIndex::get_for_display`]
    /// for why display surfaces read differently from diagnostics.
    /// Implements [ANALYSIS-INDEX-LASTGOOD].
    pub(super) async fn get_display_data(
        &self,
        uri: &Url,
    ) -> Option<(String, Arc<basilisk_resolver::ResolvedModule>)> {
        self.with_index(|idx| idx.get_for_display(uri)).await
    }

    /// Run a position-based handler: extract document data, compute byte offset,
    /// call `handler`, and wrap the result in `LspResult`.
    pub(super) async fn at_position<T>(
        &self,
        uri: Url,
        pos: Position,
        handler: impl FnOnce(
            &Arc<basilisk_resolver::ResolvedModule>,
            &str,
            usize,
            &Url,
            &[basilisk_checker::Diagnostic],
        ) -> Option<T>,
    ) -> LspResult<Option<T>> {
        let Some((text, resolved, diags)) = self.get_document_data(&uri).await else {
            return Ok(None);
        };
        let byte_offset = crate::util::position_to_byte_offset(&text, pos);
        Ok(handler(&resolved, &text, byte_offset, &uri, &diags))
    }
}

// Implements [ANALYSIS-ENABLED] — the single choke point for the
// `basilisk.enabled` gate, callable from spawned tasks that hold a cloned
// `Client` + flag `Arc` (file-watcher debounce, startup scan) without a
// `&LspServer`.
/// Publish `diags` for `uri` only while the `enabled` flag is set.
///
/// The disable path clears already-published diagnostics by publishing an empty
/// set; that clearing must happen regardless of the flag, so callers wanting to
/// clear use [`Client::publish_diagnostics`] directly rather than this gate.
pub(super) async fn publish_diagnostics_gated(
    client: &Client,
    enabled: &RwLock<bool>,
    analyze: &std::sync::atomic::AtomicBool,
    uri: Url,
    mut diags: Vec<Diagnostic>,
) {
    let is_enabled = *enabled.read().await;
    // Implements [LSPARCH-DIAGNOSTIC-SCOPE]: with the analyze opt-out set,
    // only check-scope (`pep`-tagged) diagnostics publish. The edge filter is
    // `is_pep_rule` — project configuration never selects scope.
    if !analyze.load(std::sync::atomic::Ordering::Relaxed) {
        diags.retain(is_check_scope);
    }
    diaglog!(
        "[DIAG] publish n={} enabled={is_enabled} uri={uri}",
        diags.len()
    );
    if is_enabled {
        client.publish_diagnostics(uri, diags, None).await;
    }
}

/// Whether a published diagnostic belongs to check scope.
///
/// Implements [LSPARCH-DIAGNOSTIC-SCOPE] / [CHKARCH-COMMANDS]: `pep`-tagged
/// rules are check scope. Diagnostics without a registry code (syntax errors,
/// tooling notices) are never analyze-scope rules and always publish.
fn is_check_scope(diagnostic: &Diagnostic) -> bool {
    match &diagnostic.code {
        Some(tower_lsp::lsp_types::NumberOrString::String(code)) => {
            basilisk_checker::is_pep_rule(code)
        }
        Some(tower_lsp::lsp_types::NumberOrString::Number(_)) | None => true,
    }
}

/// Return `None` if the collection is empty, `Some(v)` otherwise.
pub(super) fn none_if_empty<T>(items: Vec<T>) -> Option<Vec<T>> {
    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}

// ── tower-lsp LanguageServer trait implementation ─────────────────────────────

#[tower_lsp::async_trait]
impl tower_lsp::LanguageServer for LspServer {
    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
        init::initialize(self, params).await
    }

    async fn initialized(&self, _: InitializedParams) {
        init::initialized(self).await;
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        init::did_change_configuration(self, params).await;
    }

    async fn did_change_workspace_folders(&self, params: DidChangeWorkspaceFoldersParams) {
        init::did_change_workspace_folders(self, params).await;
    }

    async fn shutdown(&self) -> LspResult<()> {
        init::shutdown(self).await
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        document::did_open(self, params).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        document::did_change(self, params).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        document::did_save(self, params).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        document::did_close(self, params).await;
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        document::did_change_watched_files(self, params).await;
    }

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        handlers::hover(self, params).await
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        handlers::goto_definition(self, params).await
    }

    async fn goto_declaration(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        handlers::goto_declaration(self, params).await
    }

    async fn goto_type_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        handlers::goto_type_definition(self, params).await
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> LspResult<Option<DocumentSymbolResponse>> {
        handlers::document_symbol(self, params).await
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> LspResult<Option<Vec<SymbolInformation>>> {
        handlers::symbol(self, params).await
    }

    async fn signature_help(
        &self,
        params: SignatureHelpParams,
    ) -> LspResult<Option<tower_lsp::lsp_types::SignatureHelp>> {
        handlers::signature_help(self, params).await
    }

    async fn references(&self, params: ReferenceParams) -> LspResult<Option<Vec<Location>>> {
        handlers::references(self, params).await
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> LspResult<Option<Vec<DocumentHighlight>>> {
        handlers::document_highlight(self, params).await
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> LspResult<Option<PrepareRenameResponse>> {
        handlers::prepare_rename(self, params).await
    }

    async fn rename(&self, params: RenameParams) -> LspResult<Option<WorkspaceEdit>> {
        handlers::rename(self, params).await
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> LspResult<Option<Vec<InlayHint>>> {
        handlers::inlay_hint(self, params).await
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> LspResult<Option<SemanticTokensResult>> {
        handlers::semantic_tokens_full(self, params).await
    }

    async fn code_action(&self, params: CodeActionParams) -> LspResult<Option<CodeActionResponse>> {
        handlers::code_action(self, params).await
    }

    async fn execute_command(
        &self,
        params: ExecuteCommandParams,
    ) -> LspResult<Option<serde_json::Value>> {
        commands::dispatch_execute_command(self, params).await
    }

    async fn completion(&self, params: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        handlers::completion(self, params).await
    }

    async fn completion_resolve(&self, item: CompletionItem) -> LspResult<CompletionItem> {
        handlers::completion_resolve(self, item).await
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> LspResult<Option<Vec<TextEdit>>> {
        handlers::formatting(self, params).await
    }

    async fn range_formatting(
        &self,
        params: DocumentRangeFormattingParams,
    ) -> LspResult<Option<Vec<TextEdit>>> {
        handlers::range_formatting(self, params).await
    }

    async fn folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> LspResult<Option<Vec<FoldingRange>>> {
        handlers::folding_range(self, params).await
    }

    async fn selection_range(
        &self,
        params: SelectionRangeParams,
    ) -> LspResult<Option<Vec<SelectionRange>>> {
        handlers::selection_range(self, params).await
    }

    async fn prepare_call_hierarchy(
        &self,
        params: CallHierarchyPrepareParams,
    ) -> LspResult<Option<Vec<CallHierarchyItem>>> {
        handlers::prepare_call_hierarchy(self, params).await
    }

    async fn incoming_calls(
        &self,
        params: CallHierarchyIncomingCallsParams,
    ) -> LspResult<Option<Vec<CallHierarchyIncomingCall>>> {
        handlers::incoming_calls(self, params).await
    }

    async fn outgoing_calls(
        &self,
        params: CallHierarchyOutgoingCallsParams,
    ) -> LspResult<Option<Vec<CallHierarchyOutgoingCall>>> {
        handlers::outgoing_calls(self, params).await
    }

    async fn code_lens(&self, params: CodeLensParams) -> LspResult<Option<Vec<CodeLens>>> {
        handlers::code_lens(self, params).await
    }

    async fn prepare_type_hierarchy(
        &self,
        params: TypeHierarchyPrepareParams,
    ) -> LspResult<Option<Vec<TypeHierarchyItem>>> {
        handlers::prepare_type_hierarchy(self, params).await
    }

    async fn supertypes(
        &self,
        params: TypeHierarchySupertypesParams,
    ) -> LspResult<Option<Vec<TypeHierarchyItem>>> {
        handlers::supertypes(self, params).await
    }

    async fn subtypes(
        &self,
        params: TypeHierarchySubtypesParams,
    ) -> LspResult<Option<Vec<TypeHierarchyItem>>> {
        handlers::subtypes(self, params).await
    }

    async fn document_color(
        &self,
        params: DocumentColorParams,
    ) -> LspResult<Vec<ColorInformation>> {
        handlers::document_color(self, params).await
    }

    async fn color_presentation(
        &self,
        params: ColorPresentationParams,
    ) -> LspResult<Vec<ColorPresentation>> {
        handlers::color_presentation(self, params).await
    }

    async fn will_rename_files(
        &self,
        params: RenameFilesParams,
    ) -> LspResult<Option<WorkspaceEdit>> {
        handlers::will_rename_files(self, params).await
    }
}

// ── Server entry point ───────────────────────────────────────────────────────

/// Start the LSP server.
///
/// Runs on analysis-sized stacks ([LSPARCH-ARCH-STACK]): recursive AST
/// visitors overflow default thread stacks on deeply chained expressions in
/// generated code, aborting the whole server (GitHub #278).
///
/// # Errors
///
/// Returns an `io::Error` if the Tokio runtime fails to initialize.
pub fn run_server() -> std::io::Result<()> {
    crate::runtime::block_on_with_analysis_stack("basilisk-lsp-stdio", || async {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let (service, socket) = LspService::build(LspServer::new)
            .custom_method(
                basilisk_common::configuration_editor::SNAPSHOT,
                LspServer::configuration_snapshot,
            )
            .custom_method(
                basilisk_common::configuration_editor::PREVIEW,
                LspServer::preview_configuration_change,
            )
            .custom_method(
                basilisk_common::configuration_editor::APPLY,
                LspServer::apply_configuration_change,
            )
            .custom_method(
                basilisk_common::configuration_editor::OCCURRENCES,
                LspServer::rule_occurrences,
            )
            .custom_method(
                basilisk_common::configuration_editor::TYPESHED_ACTION,
                LspServer::typeshed_action,
            )
            .custom_method(
                basilisk_common::configuration_editor::TYPESHED_DOCUMENT,
                LspServer::typeshed_document,
            )
            .finish();
        Server::new(stdin, stdout, socket).serve(service).await;
        Ok(())
    })
}
