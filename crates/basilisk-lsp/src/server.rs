//! Language Server Protocol server implementation for Basilisk.
//!
//! Thin dispatcher that delegates to feature modules for each LSP request.

use std::sync::Arc;

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    CallHierarchyServerCapability, CodeActionKind, CodeActionOptions, CodeActionParams,
    CodeActionProviderCapability, CodeActionResponse, CodeLens, CodeLensOptions, CodeLensParams,
    ColorInformation, ColorPresentation, ColorPresentationParams, ColorProviderCapability,
    CompletionItem, CompletionOptions, CompletionParams, CompletionResponse, DeclarationCapability,
    DidChangeTextDocumentParams, DidChangeWatchedFilesParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentColorParams,
    DocumentFormattingParams, DocumentHighlight, DocumentHighlightParams, DocumentSymbolParams,
    DocumentSymbolResponse, ExecuteCommandOptions, ExecuteCommandParams, FileChangeType,
    FileOperationFilter, FileOperationPattern, FileOperationPatternKind,
    FileOperationRegistrationOptions, FoldingRange, FoldingRangeParams,
    FoldingRangeProviderCapability, GotoDefinitionParams, GotoDefinitionResponse, Hover,
    HoverParams, HoverProviderCapability, InitializeParams, InitializeResult, InitializedParams,
    InlayHint, InlayHintParams, Location, MessageType, OneOf, Position, PrepareRenameResponse,
    ReferenceParams, RenameOptions, RenameParams, SelectionRange, SelectionRangeParams,
    SelectionRangeProviderCapability, SemanticTokens, SemanticTokensFullOptions,
    SemanticTokensLegend, SemanticTokensOptions, SemanticTokensParams, SemanticTokensResult,
    SemanticTokensServerCapabilities, ServerCapabilities, ServerInfo, SignatureHelpOptions,
    SignatureHelpParams, SymbolInformation, TextDocumentPositionParams,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit, TypeDefinitionProviderCapability,
    TypeHierarchyItem, TypeHierarchyPrepareParams, TypeHierarchySubtypesParams,
    TypeHierarchySupertypesParams, Url, WorkDoneProgressOptions, WorkspaceEdit,
    WorkspaceFoldersServerCapabilities, WorkspaceServerCapabilities,
    WorkspaceFileOperationsServerCapabilities, WorkspaceSymbolParams,
};
use tower_lsp::{Client, LspService, Server};

use crate::config::AnalysisMode;
use crate::util::position_to_byte_offset;
use crate::workspace::{WorkspaceIndex, resolve_analysis_mode};
use crate::{
    call_hierarchy, code_actions, code_lens, completion, declaration, definition, folding,
    formatting, highlight, hover, inlay_hints, references, selection, signature, symbols,
    type_definition, type_hierarchy,
};

/// Debounce delay for file-watcher events (milliseconds).
const FILE_WATCHER_DEBOUNCE_MS: u64 = 150;

/// The Basilisk LSP server.
pub struct LspServer {
    /// LSP client for sending notifications back to the editor.
    client: Client,
    /// Workspace index — the single source of truth for all file analysis state.
    ///
    /// Wrapped in `Arc` so the debounce task can share ownership without
    /// holding a reference into `self`.
    index: Arc<tokio::sync::RwLock<Option<WorkspaceIndex>>>,
    /// Abort handle for the pending file-watcher debounce task (if any).
    watcher_debounce: tokio::sync::Mutex<Option<tokio::task::AbortHandle>>,
}

impl LspServer {
    /// Create a new LSP server instance.
    #[must_use]
    pub fn new(client: Client) -> Self {
        Self {
            client,
            index: Arc::new(tokio::sync::RwLock::new(None)),
            watcher_debounce: tokio::sync::Mutex::new(None),
        }
    }

    /// Borrow the index and call `f` with it. Returns `None` if not yet
    /// initialized (before `initialized()` fires).
    async fn with_index<T, F>(&self, f: F) -> Option<T>
    where
        F: FnOnce(&WorkspaceIndex) -> Option<T>,
    {
        let guard = self.index.read().await;
        guard.as_ref().and_then(f)
    }

    /// Get the text, resolved module, and diagnostics for a document.
    async fn get_document_data(
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
    async fn at_position<T>(
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
        let byte_offset = position_to_byte_offset(&text, pos);
        Ok(handler(&resolved, &text, byte_offset, &uri, &diags))
    }
}

// ── tower-lsp LanguageServer trait implementation ─────────────────────────────

#[tower_lsp::async_trait]
impl tower_lsp::LanguageServer for LspServer {
    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
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
        let mode = resolve_analysis_mode(params.initialization_options.as_ref(), &roots);

        // Build the workspace index now so `initialized()` can scan immediately.
        let index = WorkspaceIndex::new(roots, mode);
        *self.index.write().await = Some(index);

        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "basilisk".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
            capabilities: build_server_capabilities(mode),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Basilisk LSP initialized")
            .await;

        let guard = self.index.read().await;
        let Some(index) = guard.as_ref() else { return };

        match index.mode {
            AnalysisMode::OpenFilesOnly => {
                self.client
                    .log_message(
                        MessageType::INFO,
                        "Basilisk: analysisMode=openFilesOnly — skipping workspace scan",
                    )
                    .await;
            }
            AnalysisMode::WholeModule | AnalysisMode::CrossModule => {
                self.client
                    .log_message(
                        MessageType::INFO,
                        "Basilisk: scanning workspace files...",
                    )
                    .await;
                let (results, file_count, error_count) = index.scan();
                drop(guard);
                for (uri, diags) in results {
                    self.client.publish_diagnostics(uri, diags, None).await;
                }
                self.client
                    .log_message(
                        MessageType::INFO,
                        format!(
                            "Basilisk: workspace scan complete — {file_count} files, {error_count} error(s)"
                        ),
                    )
                    .await;
                return;
            }
        }
        drop(guard);
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }

    // ── Document lifecycle ───────────────────────────────────────────────────

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        let version = params.text_document.version;
        let guard = self.index.read().await;
        let Some(index) = guard.as_ref() else { return };
        let diags = index.set_open(&uri, &text, version);
        drop(guard);
        self.client.publish_diagnostics(uri, diags, None).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;

        // Get current text for incremental edits.
        let guard = self.index.read().await;
        let Some(index) = guard.as_ref() else { return };
        let mut text = index.get_text(&uri).unwrap_or_default();
        drop(guard);

        // Apply incremental changes.
        for change in params.content_changes {
            if let Some(range) = change.range {
                let start = position_to_byte_offset(&text, range.start);
                let end = position_to_byte_offset(&text, range.end);
                text.replace_range(start..end, &change.text);
            } else {
                text = change.text;
            }
        }

        let guard = self.index.read().await;
        let Some(index) = guard.as_ref() else { return };
        let diags = index.set_open(&uri, &text, version);
        drop(guard);
        self.client.publish_diagnostics(uri, diags, None).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        // Re-run the pipeline on the cached in-memory text (already up-to-date).
        let guard = self.index.read().await;
        let Some(index) = guard.as_ref() else { return };
        let Some(text) = index.get_text(&uri) else {
            return;
        };
        let diags = index.set_open(&uri, &text, 0);
        drop(guard);
        self.client.publish_diagnostics(uri, diags, None).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        let guard = self.index.read().await;
        let Some(index) = guard.as_ref() else {
            self.client.publish_diagnostics(uri, vec![], None).await;
            return;
        };
        // In wholeModule/crossModule: re-analyse from disk and keep diagnostics.
        // In openFilesOnly: clear diagnostics (file is no longer open).
        match index.mode {
            AnalysisMode::OpenFilesOnly => {
                drop(guard);
                self.client.publish_diagnostics(uri, vec![], None).await;
            }
            AnalysisMode::WholeModule | AnalysisMode::CrossModule => {
                let (publish_uri, diags) = index.set_closed(&uri);
                drop(guard);
                self.client.publish_diagnostics(publish_uri, diags, None).await;
            }
        }
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        // Quick mode check — bail early for openFilesOnly without taking the write lock.
        {
            let guard = self.index.read().await;
            let Some(index) = guard.as_ref() else { return };
            if index.mode == AnalysisMode::OpenFilesOnly {
                return; // File-watcher events are irrelevant in openFilesOnly mode.
            }
        }

        // Classify the incoming changes, filtering to Python files only.
        let mut reload_targets: Vec<Url> = Vec::new();
        let mut delete_targets: Vec<Url> = Vec::new();

        for change in &params.changes {
            let uri = &change.uri;
            let path = uri.to_file_path().unwrap_or_default();
            if !path
                .extension()
                .is_some_and(|ext| ext == "py" || ext == "pyi")
            {
                continue;
            }
            match change.typ {
                FileChangeType::CREATED | FileChangeType::CHANGED => {
                    reload_targets.push(uri.clone());
                }
                FileChangeType::DELETED => {
                    delete_targets.push(uri.clone());
                }
                _ => {}
            }
        }

        if reload_targets.is_empty() && delete_targets.is_empty() {
            return;
        }

        // Debounce: abort any pending watcher task and replace with a new one
        // that fires after FILE_WATCHER_DEBOUNCE_MS milliseconds.
        let index_lock = Arc::clone(&self.index);
        let client = self.client.clone();

        let task = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(FILE_WATCHER_DEBOUNCE_MS)).await;

            let guard = index_lock.read().await;
            let Some(index) = guard.as_ref() else { return };

            let reload_results: Vec<_> = reload_targets
                .iter()
                .filter_map(|uri| index.reload_from_disk(uri))
                .collect();

            drop(guard);

            for (uri, diags) in reload_results {
                client.publish_diagnostics(uri, diags, None).await;
            }
            for uri in delete_targets {
                client.publish_diagnostics(uri, vec![], None).await;
            }
        });

        let abort_handle = task.abort_handle();
        let mut debounce = self.watcher_debounce.lock().await;
        if let Some(old) = debounce.take() {
            old.abort();
        }
        *debounce = Some(abort_handle);
    }

    // ── Hover ────────────────────────────────────────────────────────────────

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        self.at_position(uri, pos, |resolved, text, offset, _, diags| {
            hover::hover_at(resolved, text, offset, diags)
        })
        .await
    }

    // ── Go to Definition ─────────────────────────────────────────────────────

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        self.at_position(uri, pos, |resolved, text, offset, uri, _| {
            definition::goto_definition(resolved, text, offset, uri)
        })
        .await
    }

    // ── Go to Declaration ────────────────────────────────────────────────────

    async fn goto_declaration(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        self.at_position(uri, pos, |resolved, text, offset, uri, _| {
            declaration::goto_declaration(resolved, text, offset, uri)
        })
        .await
    }

    // ── Go to Type Definition ───────────────────────────────────────────────

    async fn goto_type_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        self.at_position(uri, pos, |resolved, text, offset, uri, _| {
            type_definition::goto_type_definition(resolved, text, offset, uri)
        })
        .await
    }

    // ── Document Symbols ─────────────────────────────────────────────────────

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> LspResult<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let Some((text, resolved, _)) = self.get_document_data(&uri).await else {
            return Ok(None);
        };
        let syms = symbols::document_symbols(&resolved, &text);
        if syms.is_empty() {
            Ok(None)
        } else {
            Ok(Some(DocumentSymbolResponse::Nested(syms)))
        }
    }

    // ── Workspace Symbols ─────────────────────────────────────────────────────

    #[allow(deprecated)]
    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> LspResult<Option<Vec<SymbolInformation>>> {
        let query = &params.query;
        let docs = self
            .with_index(|idx| Some(idx.all_resolved()))
            .await
            .unwrap_or_default();
        Ok(none_if_empty(symbols::workspace_symbols(&docs, query)))
    }

    // ── Signature Help ───────────────────────────────────────────────────────

    async fn signature_help(
        &self,
        params: SignatureHelpParams,
    ) -> LspResult<Option<tower_lsp::lsp_types::SignatureHelp>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        self.at_position(uri, pos, |resolved, text, offset, _, _| {
            signature::signature_help_at(resolved, text, offset)
        })
        .await
    }

    // ── Find All References ──────────────────────────────────────────────────

    async fn references(&self, params: ReferenceParams) -> LspResult<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let include_decl = params.context.include_declaration;
        self.at_position(uri, pos, |resolved, text, offset, uri, _| {
            none_if_empty(references::find_references(resolved, text, offset, uri, include_decl))
        })
        .await
    }

    // ── Document Highlight ────────────────────────────────────────────────

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> LspResult<Option<Vec<DocumentHighlight>>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        self.at_position(uri, pos, |resolved, text, offset, _, _| {
            none_if_empty(highlight::document_highlights(resolved, text, offset))
        })
        .await
    }

    // ── Rename ───────────────────────────────────────────────────────────────

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> LspResult<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri;
        let pos = params.position;
        self.at_position(uri, pos, |resolved, text, offset, _, _| {
            references::prepare_rename(resolved, text, offset)
        })
        .await
    }

    async fn rename(&self, params: RenameParams) -> LspResult<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let new_name = params.new_name;
        self.at_position(uri, pos, |resolved, text, offset, uri, _| {
            references::rename_symbol(resolved, text, offset, uri, &new_name)
        })
        .await
    }

    // ── Inlay Hints ──────────────────────────────────────────────────────────

    async fn inlay_hint(&self, params: InlayHintParams) -> LspResult<Option<Vec<InlayHint>>> {
        let uri = params.text_document.uri;
        let Some((text, resolved, _)) = self.get_document_data(&uri).await else {
            return Ok(None);
        };
        Ok(none_if_empty(inlay_hints::inlay_hints(&resolved, &text)))
    }

    // ── Semantic Tokens ──────────────────────────────────────────────────────

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> LspResult<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let Some((text, resolved, _)) = self.get_document_data(&uri).await else {
            return Ok(None);
        };
        let tokens = crate::semantic_tokens::semantic_tokens(&resolved, &text);
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: tokens,
        })))
    }

    // ── Code Actions ─────────────────────────────────────────────────────────

    async fn code_action(&self, params: CodeActionParams) -> LspResult<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let source = self
            .with_index(|idx| idx.get_text(&uri))
            .await
            .unwrap_or_default();
        Ok(none_if_empty(code_actions::code_actions(
            &uri,
            &params.context.diagnostics,
            &source,
        )))
    }

    // ── Execute Command ──────────────────────────────────────────────────────

    async fn execute_command(
        &self,
        params: ExecuteCommandParams,
    ) -> LspResult<Option<serde_json::Value>> {
        if params.command != "basilisk.organizeImports" {
            return Ok(None);
        }

        let Some(uri_value) = params.arguments.first() else {
            return Ok(None);
        };
        let Some(uri_str) = uri_value.as_str() else {
            return Ok(None);
        };
        let Ok(uri) = Url::parse(uri_str) else {
            return Ok(None);
        };

        let source = self
            .with_index(|idx| idx.get_text(&uri))
            .await
            .unwrap_or_default();

        if source.is_empty() {
            return Ok(None);
        }

        let Some(action) = code_actions::organize_imports(&uri, &source) else {
            return Ok(None);
        };

        if let Some(edit) = action.edit {
            let _ = self.client.apply_edit(edit).await;
        }

        Ok(None)
    }

    // ── Completion ───────────────────────────────────────────────────────────

    async fn completion(&self, params: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let Some(text) = self.with_index(|idx| idx.get_text(&uri)).await else {
            return Ok(None);
        };

        let byte_offset = position_to_byte_offset(&text, pos);
        let file_path = uri.to_file_path().unwrap_or_default();
        let path_str = file_path.to_string_lossy().into_owned();

        // For completion we re-resolve (possibly with a patched cursor line)
        // because the cached resolve may be stale due to incomplete expressions.
        let resolved = completion::try_resolve(&text, &path_str).or_else(|| {
            let patched = completion::patch_cursor_line(&text, pos.line);
            completion::try_resolve(&patched, &path_str)
        });

        let Some(resolved) = resolved else {
            return Ok(None);
        };

        let items = completion::complete(&resolved, &text, byte_offset);
        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(CompletionResponse::Array(items)))
        }
    }

    /// Resolve additional documentation for a completion item.
    ///
    /// This is called when the user selects a completion item, allowing us to
    /// lazily load documentation (docstrings) that wasn't included in the
    /// initial completion list.
    async fn completion_resolve(&self, item: CompletionItem) -> LspResult<CompletionItem> {
        // Get the document text to resolve the module
        // We need to find which document this completion belongs to
        let (text, path_str) = self
            .with_index(|idx| {
                idx.files.iter().next().map(|entry| {
                    let path = entry.key().clone();
                    let text = entry
                        .resolved
                        .as_ref()
                        .map(|r| r.source.clone())
                        .unwrap_or_default();
                    let path_str = path.to_string_lossy().into_owned();
                    (text, path_str)
                })
            })
            .await
            .unwrap_or_default();

        Ok(completion::resolve_completion_item(item, &text, &path_str))
    }

    // ── Document Formatting ─────────────────────────────────────────────────

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> LspResult<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let Some(text) = self.with_index(|idx| idx.get_text(&uri)).await else {
            return Ok(None);
        };
        let file_path = uri.to_file_path().unwrap_or_default();
        let path_str = file_path.to_string_lossy().into_owned();
        Ok(formatting::format_document(&text, &path_str))
    }

    // ── Folding Ranges ────────────────────────────────────────────────────

    async fn folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> LspResult<Option<Vec<FoldingRange>>> {
        let uri = params.text_document.uri;
        let Some((text, resolved, _)) = self.get_document_data(&uri).await else {
            return Ok(None);
        };
        Ok(none_if_empty(folding::folding_ranges(&resolved, &text)))
    }

    // ── Selection Ranges ─────────────────────────────────────────────────

    async fn selection_range(
        &self,
        params: SelectionRangeParams,
    ) -> LspResult<Option<Vec<SelectionRange>>> {
        let uri = params.text_document.uri;
        let Some((text, resolved, _)) = self.get_document_data(&uri).await else {
            return Ok(None);
        };
        Ok(none_if_empty(selection::selection_ranges(
            &resolved,
            &text,
            &params.positions,
        )))
    }

    // ── Call Hierarchy ──────────────────────────────────────────────────

    async fn prepare_call_hierarchy(
        &self,
        params: CallHierarchyPrepareParams,
    ) -> LspResult<Option<Vec<CallHierarchyItem>>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        self.at_position(uri, pos, |resolved, text, offset, uri, _| {
            none_if_empty(call_hierarchy::prepare(resolved, text, offset, uri))
        })
        .await
    }

    async fn incoming_calls(
        &self,
        params: CallHierarchyIncomingCallsParams,
    ) -> LspResult<Option<Vec<CallHierarchyIncomingCall>>> {
        let uri = params.item.uri.clone();
        let item_name = params.item.name.clone();
        let Some((text, resolved, _)) = self.get_document_data(&uri).await else {
            return Ok(None);
        };
        Ok(none_if_empty(call_hierarchy::incoming_calls(
            &resolved, &text, &item_name, &uri,
        )))
    }

    async fn outgoing_calls(
        &self,
        params: CallHierarchyOutgoingCallsParams,
    ) -> LspResult<Option<Vec<CallHierarchyOutgoingCall>>> {
        let uri = params.item.uri.clone();
        let item_name = params.item.name.clone();
        let Some((text, resolved, _)) = self.get_document_data(&uri).await else {
            return Ok(None);
        };
        Ok(none_if_empty(call_hierarchy::outgoing_calls(
            &resolved, &text, &item_name, &uri,
        )))
    }

    // ── Code Lens ─────────────────────────────────────────────────────────

    async fn code_lens(&self, params: CodeLensParams) -> LspResult<Option<Vec<CodeLens>>> {
        let uri = params.text_document.uri;
        let Some((text, resolved, _)) = self.get_document_data(&uri).await else {
            return Ok(None);
        };
        Ok(none_if_empty(code_lens::code_lenses(&resolved, &text)))
    }

    // ── Type Hierarchy ──────────────────────────────────────────────────

    async fn prepare_type_hierarchy(
        &self,
        params: TypeHierarchyPrepareParams,
    ) -> LspResult<Option<Vec<TypeHierarchyItem>>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        self.at_position(uri, pos, |resolved, text, offset, uri, _| {
            none_if_empty(type_hierarchy::prepare(resolved, text, offset, uri))
        })
        .await
    }

    async fn supertypes(
        &self,
        params: TypeHierarchySupertypesParams,
    ) -> LspResult<Option<Vec<TypeHierarchyItem>>> {
        let uri = params.item.uri.clone();
        let class_name = params
            .item
            .data
            .as_ref()
            .and_then(|d| d.as_str())
            .unwrap_or(&params.item.name);
        let Some((text, resolved, _)) = self.get_document_data(&uri).await else {
            return Ok(None);
        };
        Ok(none_if_empty(type_hierarchy::supertypes(
            &resolved, &text, class_name, &uri,
        )))
    }

    async fn subtypes(
        &self,
        params: TypeHierarchySubtypesParams,
    ) -> LspResult<Option<Vec<TypeHierarchyItem>>> {
        let uri = params.item.uri.clone();
        let class_name = params
            .item
            .data
            .as_ref()
            .and_then(|d| d.as_str())
            .unwrap_or(&params.item.name);
        let Some((text, resolved, _)) = self.get_document_data(&uri).await else {
            return Ok(None);
        };
        Ok(none_if_empty(type_hierarchy::subtypes(
            &resolved, &text, class_name, &uri,
        )))
    }

    // ── Document Color ──────────────────────────────────────────────────

    async fn document_color(
        &self,
        params: DocumentColorParams,
    ) -> LspResult<Vec<ColorInformation>> {
        let uri = params.text_document.uri;
        let source = self
            .with_index(|idx| idx.get_text(&uri))
            .await
            .unwrap_or_default();
        Ok(crate::color::document_colors(&source))
    }

    async fn color_presentation(
        &self,
        params: ColorPresentationParams,
    ) -> LspResult<Vec<ColorPresentation>> {
        Ok(crate::color::color_presentations(
            &params.color,
            &params.range,
        ))
    }
}

/// Return `None` if the collection is empty, `Some(v)` otherwise.
fn none_if_empty<T>(items: Vec<T>) -> Option<Vec<T>> {
    if items.is_empty() { None } else { Some(items) }
}

// ── Server entry point ───────────────────────────────────────────────────────

/// Build the full `ServerCapabilities` for the `initialize` response.
///
/// Extracted to keep `initialize` under the 100-line limit.
fn build_server_capabilities(mode: AnalysisMode) -> ServerCapabilities {
    // Advertise workspace.fileOperations when the server tracks closed files.
    // openFilesOnly mode ignores file-system events, so skip those capabilities.
    let workspace_capabilities = if mode == AnalysisMode::OpenFilesOnly {
        None
    } else {
        let py_filter = python_file_operation_filter();
        Some(WorkspaceServerCapabilities {
            workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                supported: Some(true),
                change_notifications: Some(OneOf::Left(true)),
            }),
            file_operations: Some(WorkspaceFileOperationsServerCapabilities {
                did_create: Some(py_filter.clone()),
                did_rename: Some(py_filter.clone()),
                did_delete: Some(py_filter),
                ..Default::default()
            }),
        })
    };

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
            commands: vec!["basilisk.organizeImports".to_owned()],
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
        workspace: workspace_capabilities,
        ..Default::default()
    }
}

/// Build a `FileOperationRegistrationOptions` that matches `.py` and `.pyi` files.
///
/// Used in `build_server_capabilities` to tell the client which file operations
/// the server cares about (only in wholeModule / crossModule modes).
fn python_file_operation_filter() -> FileOperationRegistrationOptions {
    let make_filter = |glob: &str| FileOperationFilter {
        scheme: Some("file".to_owned()),
        pattern: FileOperationPattern {
            glob: glob.to_owned(),
            matches: Some(FileOperationPatternKind::File),
            options: None,
        },
    };
    FileOperationRegistrationOptions {
        filters: vec![make_filter("**/*.py"), make_filter("**/*.pyi")],
    }
}

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
