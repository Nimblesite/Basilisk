//! Language Server Protocol server implementation for Basilisk.
//!
//! Thin dispatcher that delegates to feature modules for each LSP request.

use std::collections::HashMap;
use std::sync::Arc;

use dashmap::DashMap;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    CallHierarchyServerCapability, CodeActionKind, CodeActionOptions, CodeActionParams,
    CodeActionProviderCapability, CodeActionResponse, CodeDescription, CodeLens, CodeLensOptions,
    CodeLensParams, ColorInformation, ColorPresentation, ColorPresentationParams,
    ColorProviderCapability, CompletionItem, CompletionOptions, CompletionParams,
    CompletionResponse, DeclarationCapability, Diagnostic, DiagnosticSeverity,
    DidChangeTextDocumentParams, DidChangeWatchedFilesParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentColorParams,
    DocumentFormattingParams, DocumentHighlight, DocumentHighlightParams, DocumentSymbolParams,
    DocumentSymbolResponse, ExecuteCommandOptions, ExecuteCommandParams, FileChangeType,
    FoldingRange, FoldingRangeParams, FoldingRangeProviderCapability, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverParams, HoverProviderCapability, InitializeParams,
    InitializeResult, InitializedParams, InlayHint, InlayHintParams, Location, MessageType,
    NumberOrString, OneOf, Position, PrepareRenameResponse, Range, ReferenceParams, RenameOptions,
    RenameParams, SelectionRange, SelectionRangeParams, SelectionRangeProviderCapability,
    SemanticTokens, SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions,
    SemanticTokensParams, SemanticTokensResult, SemanticTokensServerCapabilities,
    ServerCapabilities, ServerInfo, SignatureHelpOptions, SignatureHelpParams, SymbolInformation,
    TextDocumentPositionParams, TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit,
    TypeDefinitionProviderCapability, TypeHierarchyItem, TypeHierarchyPrepareParams,
    TypeHierarchySubtypesParams, TypeHierarchySupertypesParams, Url, WorkDoneProgressOptions,
    WorkspaceEdit, WorkspaceSymbolParams,
};
use tower_lsp::{Client, LspService, Server};

use crate::util::{byte_offset_to_position, position_to_byte_offset};
use crate::{
    call_hierarchy, code_actions, code_lens, completion, declaration, definition, folding,
    formatting, highlight, hover, inlay_hints, references, selection, signature, symbols,
    type_definition, type_hierarchy,
};

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
    /// Workspace root folders discovered during initialization.
    workspace_roots: tokio::sync::RwLock<Vec<std::path::PathBuf>>,
}

impl LspServer {
    /// Create a new LSP server instance.
    #[must_use]
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: DashMap::new(),
            workspace_roots: tokio::sync::RwLock::new(Vec::new()),
        }
    }

    /// Scan workspace directories for all `.py` files and check each one.
    /// When both `foo.py` and `foo.pyi` exist, only the `.pyi` file is checked.
    async fn scan_workspace(&self) {
        let roots = self.workspace_roots.read().await;
        let mut py_files: Vec<std::path::PathBuf> = Vec::new();

        for root in roots.iter() {
            let cfg = crate::config::load_config(root);
            collect_python_files(root, &mut py_files, &cfg.exclude, root);
        }
        drop(roots);

        // Group by stem (filename without extension) and prefer .pyi over .py
        let mut by_stem: HashMap<String, std::path::PathBuf> = HashMap::new();
        for path in py_files {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                let ext = path.extension().and_then(|s| s.to_str());
                let stem_key = stem.to_string();
                // If we already have an entry, check if we should replace it
                match by_stem.get(&stem_key) {
                    Some(existing) => {
                        let existing_ext = existing.extension().and_then(|s| s.to_str());
                        // Prefer .pyi over .py
                        if existing_ext == Some("py") && ext == Some("pyi") {
                            by_stem.insert(stem_key, path);
                        }
                        // else keep existing (either .pyi or other)
                    }
                    None => {
                        by_stem.insert(stem_key, path);
                    }
                }
            }
        }

        let deduped_files: Vec<std::path::PathBuf> = by_stem.into_values().collect();
        let file_count = deduped_files.len();
        self.client
            .log_message(
                MessageType::INFO,
                format!(
                    "Basilisk: scanning {file_count} Python files (after .pyi/.py deduplication)"
                ),
            )
            .await;

        for path in &deduped_files {
            let Ok(text) = std::fs::read_to_string(path) else {
                continue;
            };
            let Some(uri) = path_to_uri(path) else {
                continue;
            };
            // Don't overwrite files the user already has open.
            if self.documents.contains_key(&uri) {
                continue;
            }
            self.check_and_publish(uri, &text).await;
        }

        self.client
            .log_message(
                MessageType::INFO,
                format!("Basilisk: workspace scan complete ({file_count} files)"),
            )
            .await;
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
        let lsp_diags: Vec<Diagnostic> =
            checker_diags.iter().map(|d| bsk_to_lsp(d, text)).collect();

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
    ) -> Option<(
        String,
        Arc<basilisk_resolver::ResolvedModule>,
        Vec<basilisk_checker::Diagnostic>,
    )> {
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
        basilisk_checker::Severity::Info => DiagnosticSeverity::INFORMATION,
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
    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
        // Capture workspace roots for later scanning.
        let mut roots = self.workspace_roots.write().await;
        if let Some(folders) = &params.workspace_folders {
            for folder in folders {
                if let Ok(path) = folder.uri.to_file_path() {
                    roots.push(path);
                }
            }
        }
        // Fallback to deprecated root_uri/root_path.
        if roots.is_empty() {
            if let Some(ref root_uri) = params.root_uri {
                if let Ok(path) = root_uri.to_file_path() {
                    roots.push(path);
                }
            }
        }
        drop(roots);

        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "basilisk".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
            capabilities: ServerCapabilities {
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
                code_action_provider: Some(CodeActionProviderCapability::Options(
                    CodeActionOptions {
                        code_action_kinds: Some(vec![
                            CodeActionKind::QUICKFIX,
                            CodeActionKind::SOURCE_ORGANIZE_IMPORTS,
                            CodeActionKind::REFACTOR,
                        ]),
                        ..Default::default()
                    },
                )),
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
                color_provider: Some(ColorProviderCapability::Simple(true)),
                ..Default::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Basilisk LSP initialized")
            .await;

        // Scan the workspace for all Python files and publish diagnostics.
        self.scan_workspace().await;
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

        // Get the current text as a starting point for incremental edits.
        let mut text = self
            .documents
            .get(&uri)
            .map(|e| e.text.clone())
            .unwrap_or_default();

        // Apply each incremental change in order.
        for change in params.content_changes {
            if let Some(range) = change.range {
                // Incremental update: replace the specified range.
                let start = position_to_byte_offset(&text, range.start);
                let end = position_to_byte_offset(&text, range.end);
                text.replace_range(start..end, &change.text);
            } else {
                // Full replacement (fallback).
                text = change.text;
            }
        }

        self.check_and_publish(uri, &text).await;
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

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        for change in &params.changes {
            let uri = &change.uri;
            let path = uri.to_file_path().unwrap_or_default();
            let is_python = path
                .extension()
                .is_some_and(|ext| ext == "py" || ext == "pyi");
            if !is_python {
                continue;
            }
            match change.typ {
                FileChangeType::CREATED | FileChangeType::CHANGED => {
                    // Re-read from disk and check (unless the file is open in the editor).
                    if self.documents.contains_key(uri) {
                        continue; // Editor has the live version.
                    }
                    if let Ok(text) = std::fs::read_to_string(&path) {
                        self.check_and_publish(uri.clone(), &text).await;
                    }
                }
                FileChangeType::DELETED => {
                    // Clear diagnostics for deleted files.
                    self.documents.remove(uri);
                    self.client
                        .publish_diagnostics(uri.clone(), vec![], None)
                        .await;
                }
                _ => {}
            }
        }
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
        Ok(definition::goto_definition(
            &resolved,
            &text,
            byte_offset,
            &uri,
        ))
    }

    // ── Go to Declaration ────────────────────────────────────────────────────

    async fn goto_declaration(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let Some((text, resolved, _)) = self.get_document_data(&uri) else {
            return Ok(None);
        };
        let byte_offset = position_to_byte_offset(&text, pos);
        Ok(declaration::goto_declaration(
            &resolved,
            &text,
            byte_offset,
            &uri,
        ))
    }

    // ── Go to Type Definition ───────────────────────────────────────────────

    async fn goto_type_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let Some((text, resolved, _)) = self.get_document_data(&uri) else {
            return Ok(None);
        };
        let byte_offset = position_to_byte_offset(&text, pos);
        Ok(type_definition::goto_type_definition(
            &resolved,
            &text,
            byte_offset,
            &uri,
        ))
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

    async fn references(&self, params: ReferenceParams) -> LspResult<Option<Vec<Location>>> {
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

    async fn inlay_hint(&self, params: InlayHintParams) -> LspResult<Option<Vec<InlayHint>>> {
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

    async fn code_action(&self, params: CodeActionParams) -> LspResult<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let source = self
            .documents
            .get(&uri)
            .map(|d| d.text.clone())
            .unwrap_or_default();
        let actions = code_actions::code_actions(&uri, &params.context.diagnostics, &source);
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

    async fn completion(&self, params: CompletionParams) -> LspResult<Option<CompletionResponse>> {
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

    /// Resolve additional documentation for a completion item.
    ///
    /// This is called when the user selects a completion item, allowing us to
    /// lazily load documentation (docstrings) that wasn't included in the
    /// initial completion list.
    async fn completion_resolve(&self, item: CompletionItem) -> LspResult<CompletionItem> {
        // Get the document text to resolve the module
        // We need to find which document this completion belongs to
        let (text, path_str) =
            self.documents
                .iter()
                .next()
                .map_or((String::new(), String::new()), |entry| {
                    let text = entry.text.clone();
                    let uri = entry.key().clone();
                    let file_path = uri.to_file_path().unwrap_or_default();
                    let path_str = file_path.to_string_lossy().into_owned();
                    (text, path_str)
                });

        Ok(completion::resolve_completion_item(item, &text, &path_str))
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

    // ── Code Lens ─────────────────────────────────────────────────────────

    async fn code_lens(&self, params: CodeLensParams) -> LspResult<Option<Vec<CodeLens>>> {
        let uri = params.text_document.uri;
        let Some((text, resolved, _)) = self.get_document_data(&uri) else {
            return Ok(None);
        };
        let lenses = code_lens::code_lenses(&resolved, &text);
        if lenses.is_empty() {
            Ok(None)
        } else {
            Ok(Some(lenses))
        }
    }

    // ── Type Hierarchy ──────────────────────────────────────────────────

    async fn prepare_type_hierarchy(
        &self,
        params: TypeHierarchyPrepareParams,
    ) -> LspResult<Option<Vec<TypeHierarchyItem>>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let Some((text, resolved, _)) = self.get_document_data(&uri) else {
            return Ok(None);
        };
        let byte_offset = position_to_byte_offset(&text, pos);
        let items = type_hierarchy::prepare(&resolved, &text, byte_offset, &uri);
        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(items))
        }
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
        let Some((text, resolved, _)) = self.get_document_data(&uri) else {
            return Ok(None);
        };
        let items = type_hierarchy::supertypes(&resolved, &text, class_name, &uri);
        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(items))
        }
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
        let Some((text, resolved, _)) = self.get_document_data(&uri) else {
            return Ok(None);
        };
        let items = type_hierarchy::subtypes(&resolved, &text, class_name, &uri);
        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(items))
        }
    }

    // ── Document Color ──────────────────────────────────────────────────

    async fn document_color(
        &self,
        params: DocumentColorParams,
    ) -> LspResult<Vec<ColorInformation>> {
        let uri = params.text_document.uri;
        let source = self
            .documents
            .get(&uri)
            .map(|d| d.text.clone())
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

// ── Workspace scanning helpers ────────────────────────────────────────────────

/// Recursively collect all `.py` files under `dir`, skipping hidden dirs,
/// common non-source directories, and user-configured exclude paths.
fn collect_python_files(
    dir: &std::path::Path,
    out: &mut Vec<std::path::PathBuf>,
    exclude: &[std::path::PathBuf],
    workspace_root: &std::path::Path,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            // Skip hidden dirs and common non-source directories.
            if name_str.starts_with('.')
                || name_str == "__pycache__"
                || name_str == "node_modules"
                || name_str == "venv"
                || name_str == ".tox"
                || name_str == ".mypy_cache"
                || name_str == ".ruff_cache"
            {
                continue;
            }
            // Skip user-configured exclude paths.
            if is_excluded(&path, exclude, workspace_root) {
                continue;
            }
            collect_python_files(&path, out, exclude, workspace_root);
        } else if path
            .extension()
            .is_some_and(|ext| ext == "py" || ext == "pyi")
        {
            out.push(path);
        }
    }
}

/// Check if a path matches any of the configured exclude patterns.
fn is_excluded(
    path: &std::path::Path,
    exclude: &[std::path::PathBuf],
    workspace_root: &std::path::Path,
) -> bool {
    let relative = path.strip_prefix(workspace_root).unwrap_or(path);
    exclude.iter().any(|exc| {
        // Match if the relative path starts with the exclude pattern.
        relative.starts_with(exc)
    })
}

/// Convert a filesystem path to an LSP `Url`.
fn path_to_uri(path: &std::path::Path) -> Option<Url> {
    Url::from_file_path(path).ok()
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
