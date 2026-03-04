//! Language Server Protocol server implementation for Basilisk.
//!
//! Thin dispatcher that delegates to feature modules for each LSP request.

use std::sync::Arc;

use dashmap::DashMap;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    CallHierarchyServerCapability, CodeActionParams, CodeActionProviderCapability,
    CodeActionResponse, CodeDescription, CompletionOptions, CompletionParams, CompletionResponse,
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentHighlight,
    DocumentHighlightParams, DocumentFormattingParams, DocumentSymbolParams,
    DocumentSymbolResponse, ExecuteCommandOptions, ExecuteCommandParams, FoldingRange,
    FoldingRangeParams, FoldingRangeProviderCapability, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverParams, HoverProviderCapability, InitializeParams,
    InitializeResult, InitializedParams, InlayHint, InlayHintParams, Location, MessageType,
    NumberOrString, OneOf, Position, PrepareRenameResponse, Range, ReferenceParams, RenameOptions,
    RenameParams, SelectionRange, SelectionRangeParams, SelectionRangeProviderCapability,
    SemanticTokens, SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions,
    SemanticTokensParams, SemanticTokensResult, SemanticTokensServerCapabilities,
    ServerCapabilities, ServerInfo, SignatureHelpOptions, SignatureHelpParams, SymbolInformation,
    TextDocumentPositionParams, TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit, Url,
    WorkDoneProgressOptions, WorkspaceEdit, WorkspaceSymbolParams,
};
use tower_lsp::{Client, LspService, Server};

use crate::util::{byte_offset_to_position, position_to_byte_offset};
use crate::{call_hierarchy, code_actions, completion, definition, folding, formatting, highlight, hover, inlay_hints, references, selection, signature, symbols};

/// Fallback docs URL used when a diagnostic code URL fails to parse.
const FALLBACK_DOCS_URL: &str = "https://basilisk-lang.org";

/// State for a single open document.
struct DocumentState {
    /// Current text content.
    text: String,
    /// Cached resolved module from the last parse/resolve cycle.
    resolved: Option<Arc<basilisk_resolver::ResolvedModule>>,
    /// Cached diagnostics from the last check cycle.
    diagnostics: Vec<basilisk_checker::Diagnostic>,
}

/// The Basilisk LSP server.
pub struct LspServer {
    /// LSP client for sending notifications back to the editor.
    client: Client,
    /// Map from document URI to its current state.
    documents: DashMap<Url, DocumentState>,
}

impl LspServer {
    /// Create a new LSP server instance.
    #[must_use]
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: DashMap::new(),
        }
    }

    /// Run the checker on a document, cache results, and publish diagnostics.
    async fn check_and_publish(&self, uri: Url, text: &str) {
        let file_path = uri.to_file_path().unwrap_or_default();
        let path_str = file_path.to_string_lossy().into_owned();

        let parsed = match basilisk_parser::parse_source(text.to_owned(), path_str) {
            Ok(p) => p,
            Err(e) => {
                let lsp_diag = parse_error_diagnostic(&e.to_string());
                self.documents.insert(
                    uri.clone(),
                    DocumentState {
                        text: text.to_owned(),
                        resolved: None,
                        diagnostics: vec![],
                    },
                );
                self.client
                    .publish_diagnostics(uri, vec![lsp_diag], None)
                    .await;
                return;
            }
        };

        let Ok(resolved) = basilisk_resolver::resolve(&parsed) else {
            self.documents.insert(
                uri.clone(),
                DocumentState {
                    text: text.to_owned(),
                    resolved: None,
                    diagnostics: vec![],
                },
            );
            self.client.publish_diagnostics(uri, vec![], None).await;
            return;
        };

        let checker_diags = basilisk_checker::check(&resolved);
        let lsp_diags: Vec<Diagnostic> = checker_diags
            .iter()
            .map(|d| bsk_to_lsp(d, text))
            .collect();

        self.documents.insert(
            uri.clone(),
            DocumentState {
                text: text.to_owned(),
                resolved: Some(Arc::new(resolved)),
                diagnostics: checker_diags,
            },
        );

        self.client.publish_diagnostics(uri, lsp_diags, None).await;
    }

    /// Get the text, resolved module, and diagnostics for a document.
    fn get_document_data(
        &self,
        uri: &Url,
    ) -> Option<(String, Arc<basilisk_resolver::ResolvedModule>, Vec<basilisk_checker::Diagnostic>)>
    {
        let entry = self.documents.get(uri)?;
        let text = entry.text.clone();
        let resolved = entry.resolved.clone()?;
        let diagnostics = entry.diagnostics.clone();
        Some((text, resolved, diagnostics))
    }
}

// ── Diagnostic conversion ────────────────────────────────────────────────────

/// Convert a Basilisk diagnostic to an LSP diagnostic.
fn bsk_to_lsp(d: &basilisk_checker::Diagnostic, text: &str) -> Diagnostic {
    let start = byte_offset_to_position(text, d.span.start as usize);
    let end = byte_offset_to_position(text, d.span.end as usize);
    let severity = match d.severity {
        basilisk_checker::Severity::Error | basilisk_checker::Severity::SafetyViolation => {
            DiagnosticSeverity::ERROR
        }
        basilisk_checker::Severity::Warning => DiagnosticSeverity::WARNING,
    };
    // FALLBACK_DOCS_URL is a compile-time constant that is always a valid URL.
    let Ok(fallback) = Url::parse(FALLBACK_DOCS_URL) else {
        return Diagnostic {
            range: Range { start, end },
            severity: Some(severity),
            code: Some(NumberOrString::String(d.code.code.to_owned())),
            source: Some("basilisk".to_owned()),
            message: d.message.clone(),
            ..Default::default()
        };
    };
    Diagnostic {
        range: Range { start, end },
        severity: Some(severity),
        code: Some(NumberOrString::String(d.code.code.to_owned())),
        code_description: Some(CodeDescription {
            href: Url::parse(d.code.docs_url).unwrap_or(fallback),
        }),
        source: Some("basilisk".to_owned()),
        message: d.message.clone(),
        ..Default::default()
    }
}

/// Create a diagnostic for a parse error.
fn parse_error_diagnostic(message: &str) -> Diagnostic {
    Diagnostic {
        range: Range {
            start: Position::new(0, 0),
            end: Position::new(0, 0),
        },
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String("BSK-PARSE".to_owned())),
        source: Some("basilisk".to_owned()),
        message: format!("Parse error: {message}"),
        ..Default::default()
    }
}

// ── tower-lsp LanguageServer trait implementation ─────────────────────────────

#[tower_lsp::async_trait]
impl tower_lsp::LanguageServer for LspServer {
    async fn initialize(&self, _params: InitializeParams) -> LspResult<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "basilisk".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                call_hierarchy_provider: Some(CallHierarchyServerCapability::Simple(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".to_owned()]),
                    ..Default::default()
                }),
                definition_provider: Some(OneOf::Left(true)),
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
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: SemanticTokensLegend {
                                token_types: crate::semantic_tokens::TOKEN_TYPES.to_vec(),
                                token_modifiers: crate::semantic_tokens::TOKEN_MODIFIERS.to_vec(),
                            },
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            range: None,
                            work_done_progress_options: WorkDoneProgressOptions::default(),
                        },
                    ),
                ),
                ..Default::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Basilisk LSP initialized")
            .await;
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }

    // ── Document lifecycle ───────────────────────────────────────────────────

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        self.check_and_publish(uri, &text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let Some(change) = params.content_changes.into_iter().next() else {
            return;
        };
        self.check_and_publish(uri, &change.text).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(entry) = self.documents.get(&uri) {
            let text = entry.text.clone();
            drop(entry);
            self.check_and_publish(uri, &text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.documents.remove(&uri);
        self.client.publish_diagnostics(uri, vec![], None).await;
    }

    // ── Hover ────────────────────────────────────────────────────────────────

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let Some((text, resolved, diags)) = self.get_document_data(&uri) else {
            return Ok(None);
        };
        let byte_offset = position_to_byte_offset(&text, pos);
        Ok(hover::hover_at(&resolved, &text, byte_offset, &diags))
    }

    // ── Go to Definition ─────────────────────────────────────────────────────

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let Some((text, resolved, _)) = self.get_document_data(&uri) else {
            return Ok(None);
        };
        let byte_offset = position_to_byte_offset(&text, pos);
        Ok(definition::goto_definition(&resolved, &text, byte_offset, &uri))
    }

    // ── Document Symbols ─────────────────────────────────────────────────────

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> LspResult<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let Some((text, resolved, _)) = self.get_document_data(&uri) else {
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
        let docs: Vec<_> = self
            .documents
            .iter()
            .filter_map(|entry| {
                let uri = entry.key().clone();
                let text = entry.text.clone();
                let resolved = entry.resolved.clone()?;
                Some((uri, resolved, text))
            })
            .collect();
        let syms = symbols::workspace_symbols(&docs, query);
        if syms.is_empty() {
            Ok(None)
        } else {
            Ok(Some(syms))
        }
    }

    // ── Signature Help ───────────────────────────────────────────────────────

    async fn signature_help(
        &self,
        params: SignatureHelpParams,
    ) -> LspResult<Option<tower_lsp::lsp_types::SignatureHelp>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let Some((text, resolved, _)) = self.get_document_data(&uri) else {
            return Ok(None);
        };
        let byte_offset = position_to_byte_offset(&text, pos);
        Ok(signature::signature_help_at(&resolved, &text, byte_offset))
    }

    // ── Find All References ──────────────────────────────────────────────────

    async fn references(
        &self,
        params: ReferenceParams,
    ) -> LspResult<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let include_decl = params.context.include_declaration;
        let Some((text, resolved, _)) = self.get_document_data(&uri) else {
            return Ok(None);
        };
        let byte_offset = position_to_byte_offset(&text, pos);
        let locs = references::find_references(&resolved, &text, byte_offset, &uri, include_decl);
        if locs.is_empty() {
            Ok(None)
        } else {
            Ok(Some(locs))
        }
    }

    // ── Document Highlight ────────────────────────────────────────────────

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> LspResult<Option<Vec<DocumentHighlight>>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let Some((text, resolved, _)) = self.get_document_data(&uri) else {
            return Ok(None);
        };
        let byte_offset = position_to_byte_offset(&text, pos);
        let highlights = highlight::document_highlights(&resolved, &text, byte_offset);
        if highlights.is_empty() {
            Ok(None)
        } else {
            Ok(Some(highlights))
        }
    }

    // ── Rename ───────────────────────────────────────────────────────────────

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> LspResult<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri;
        let pos = params.position;
        let Some((text, resolved, _)) = self.get_document_data(&uri) else {
            return Ok(None);
        };
        let byte_offset = position_to_byte_offset(&text, pos);
        Ok(references::prepare_rename(&resolved, &text, byte_offset))
    }

    async fn rename(&self, params: RenameParams) -> LspResult<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let new_name = params.new_name;
        let Some((text, resolved, _)) = self.get_document_data(&uri) else {
            return Ok(None);
        };
        let byte_offset = position_to_byte_offset(&text, pos);
        Ok(references::rename_symbol(
            &resolved,
            &text,
            byte_offset,
            &uri,
            &new_name,
        ))
    }

    // ── Inlay Hints ──────────────────────────────────────────────────────────

    async fn inlay_hint(
        &self,
        params: InlayHintParams,
    ) -> LspResult<Option<Vec<InlayHint>>> {
        let uri = params.text_document.uri;
        let Some((text, resolved, _)) = self.get_document_data(&uri) else {
            return Ok(None);
        };
        let hints = inlay_hints::inlay_hints(&resolved, &text);
        if hints.is_empty() {
            Ok(None)
        } else {
            Ok(Some(hints))
        }
    }

    // ── Semantic Tokens ──────────────────────────────────────────────────────

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> LspResult<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let Some((text, resolved, _)) = self.get_document_data(&uri) else {
            return Ok(None);
        };
        let tokens = crate::semantic_tokens::semantic_tokens(&resolved, &text);
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: tokens,
        })))
    }

    // ── Code Actions ─────────────────────────────────────────────────────────

    async fn code_action(
        &self,
        params: CodeActionParams,
    ) -> LspResult<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let source = self
            .documents
            .get(&uri)
            .map(|d| d.text.clone())
            .unwrap_or_default();
        let actions =
            code_actions::code_actions(&uri, &params.context.diagnostics, &source);
        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
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
            .documents
            .get(&uri)
            .map(|d| d.text.clone())
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

    async fn completion(
        &self,
        params: CompletionParams,
    ) -> LspResult<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let Some(entry) = self.documents.get(&uri) else {
            return Ok(None);
        };
        let text = entry.text.clone();
        drop(entry);

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

    // ── Document Formatting ─────────────────────────────────────────────────

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> LspResult<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let Some(entry) = self.documents.get(&uri) else {
            return Ok(None);
        };
        let text = entry.text.clone();
        drop(entry);
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
        let Some((text, resolved, _)) = self.get_document_data(&uri) else {
            return Ok(None);
        };
        let ranges = folding::folding_ranges(&resolved, &text);
        if ranges.is_empty() {
            Ok(None)
        } else {
            Ok(Some(ranges))
        }
    }

    // ── Selection Ranges ─────────────────────────────────────────────────

    async fn selection_range(
        &self,
        params: SelectionRangeParams,
    ) -> LspResult<Option<Vec<SelectionRange>>> {
        let uri = params.text_document.uri;
        let Some((text, resolved, _)) = self.get_document_data(&uri) else {
            return Ok(None);
        };
        let ranges = selection::selection_ranges(&resolved, &text, &params.positions);
        if ranges.is_empty() {
            Ok(None)
        } else {
            Ok(Some(ranges))
        }
    }

    // ── Call Hierarchy ──────────────────────────────────────────────────

    async fn prepare_call_hierarchy(
        &self,
        params: CallHierarchyPrepareParams,
    ) -> LspResult<Option<Vec<CallHierarchyItem>>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let Some((text, resolved, _)) = self.get_document_data(&uri) else {
            return Ok(None);
        };
        let byte_offset = position_to_byte_offset(&text, pos);
        let items = call_hierarchy::prepare(&resolved, &text, byte_offset, &uri);
        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(items))
        }
    }

    async fn incoming_calls(
        &self,
        params: CallHierarchyIncomingCallsParams,
    ) -> LspResult<Option<Vec<CallHierarchyIncomingCall>>> {
        let uri = params.item.uri.clone();
        let item_name = params.item.name.clone();
        let Some((text, resolved, _)) = self.get_document_data(&uri) else {
            return Ok(None);
        };
        let calls = call_hierarchy::incoming_calls(&resolved, &text, &item_name, &uri);
        if calls.is_empty() {
            Ok(None)
        } else {
            Ok(Some(calls))
        }
    }

    async fn outgoing_calls(
        &self,
        params: CallHierarchyOutgoingCallsParams,
    ) -> LspResult<Option<Vec<CallHierarchyOutgoingCall>>> {
        let uri = params.item.uri.clone();
        let item_name = params.item.name.clone();
        let Some((text, resolved, _)) = self.get_document_data(&uri) else {
            return Ok(None);
        };
        let calls = call_hierarchy::outgoing_calls(&resolved, &text, &item_name, &uri);
        if calls.is_empty() {
            Ok(None)
        } else {
            Ok(Some(calls))
        }
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
