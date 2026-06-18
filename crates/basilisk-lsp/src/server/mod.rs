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
pub(super) mod commands;
pub(super) mod document;
pub(super) mod handlers;
pub(super) mod init;
pub(super) mod memory_handlers;
pub(super) mod profiler_handlers;
pub(super) mod refactor_commands;
pub(super) mod rule_override;
pub(super) mod stub_handlers;
pub(super) mod test_handlers;
pub(super) mod uv_handlers;

macro_rules! diaglog {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/basilisk-diag.log")
        {
            let _ = writeln!(f, $($arg)*);
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
    CompletionResponse, DidChangeConfigurationParams, DidChangeTextDocumentParams,
    DidChangeWatchedFilesParams, DidChangeWorkspaceFoldersParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentColorParams,
    DocumentFormattingParams, DocumentHighlight, DocumentHighlightParams, DocumentSymbolParams,
    DocumentSymbolResponse, ExecuteCommandParams, FoldingRange, FoldingRangeParams,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams, InitializeParams,
    InitializeResult, InitializedParams, InlayHint, InlayHintParams, Location, Position,
    PrepareRenameResponse, ReferenceParams, RenameFilesParams, RenameParams, SelectionRange,
    SelectionRangeParams, SemanticTokensParams, SemanticTokensResult, SignatureHelpParams,
    SymbolInformation, TextDocumentPositionParams, TextEdit, TypeHierarchyItem,
    TypeHierarchyPrepareParams, TypeHierarchySubtypesParams, TypeHierarchySupertypesParams, Url,
    WorkspaceEdit, WorkspaceSymbolParams,
};
use tower_lsp::{Client, LspService, Server};

use crate::workspace::WorkspaceIndex;

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
    /// Workspace root folders discovered during initialization.
    pub(super) workspace_roots: RwLock<Vec<std::path::PathBuf>>,
    /// Debug session manager — spawns debugpy and tracks active sessions.
    pub(super) debug_manager: crate::debug::DebugSessionManager,
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
            workspace_roots: RwLock::new(Vec::new()),
            debug_manager: crate::debug::DebugSessionManager::new(),
            profiler_manager: crate::profiler::ProfileSessionManager::new(),
            memory_manager: crate::profiler::memory::session::MemorySessionManager::new(),
            watcher_debounce: Mutex::new(None),
            module_changed_debounce: Mutex::new(None),
            test_config: RwLock::new(TestExplorerConfig::default()),
        }
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
/// # Errors
///
/// Returns an `io::Error` if the Tokio runtime fails to initialize.
pub fn run_server() -> std::io::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let (service, socket) = LspService::new(LspServer::new);
        Server::new(stdin, stdout, socket).serve(service).await;
    });
    Ok(())
}
