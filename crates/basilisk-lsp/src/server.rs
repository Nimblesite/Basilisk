//! Language Server Protocol server implementation for Basilisk.
//!
//! This module provides a full LSP server using tower-lsp that can be
//! started via the `basilisk lsp` CLI subcommand.

use std::fmt::Write as _;

use dashmap::DashMap;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams,
    CodeActionProviderCapability, CodeActionResponse, CodeDescription, Diagnostic,
    DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, Hover, HoverContents, HoverParams,
    HoverProviderCapability, InitializeParams, InitializeResult, InitializedParams, MarkupContent,
    MarkupKind, MessageType, NumberOrString, Position, Range, ServerCapabilities, ServerInfo,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit, Url, WorkspaceEdit,
};
use tower_lsp::{Client, LspService, Server};

/// Fallback docs URL used when a diagnostic code URL fails to parse.
const FALLBACK_DOCS_URL: &str = "https://basilisk-lang.org";

/// State for a single open document.
struct DocumentState {
    /// Current text content.
    text: String,
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

    /// Run the checker on a document and publish diagnostics.
    async fn check_and_publish(&self, uri: Url, text: &str) {
        let diagnostics = run_checker(&uri, text);
        self.client.publish_diagnostics(uri, diagnostics, None).await;
    }
}

/// Run the full Basilisk checker pipeline on in-memory text.
fn run_checker(uri: &Url, text: &str) -> Vec<Diagnostic> {
    let file_path = uri.to_file_path().unwrap_or_default();
    let path_str = file_path.to_string_lossy().into_owned();

    let parsed = match basilisk_parser::parse_source(text.to_owned(), path_str) {
        Ok(p) => p,
        Err(e) => return vec![parse_error_diagnostic(&e.to_string())],
    };
    let Ok(resolved) = basilisk_resolver::resolve(&parsed) else {
        return vec![];
    };
    basilisk_checker::check(&resolved)
        .into_iter()
        .map(|d| bsk_to_lsp(d, text))
        .collect()
}

/// Convert a Basilisk diagnostic to an LSP diagnostic.
fn bsk_to_lsp(d: basilisk_checker::Diagnostic, text: &str) -> Diagnostic {
    let start = byte_offset_to_position(text, d.span.start as usize);
    let end = byte_offset_to_position(text, d.span.end as usize);
    let severity = match d.severity {
        basilisk_checker::Severity::Error | basilisk_checker::Severity::SafetyViolation => {
            DiagnosticSeverity::ERROR
        }
        basilisk_checker::Severity::Warning => DiagnosticSeverity::WARNING,
    };
    let fallback = Url::parse(FALLBACK_DOCS_URL).unwrap_or_else(|_| {
        // SAFETY: FALLBACK_DOCS_URL is a valid constant literal; this branch is unreachable.
        Url::parse("http://localhost").unwrap_or_else(|_| unreachable!("localhost is a valid URL"))
    });
    Diagnostic {
        range: Range { start, end },
        severity: Some(severity),
        code: Some(NumberOrString::String(d.code.code.to_owned())),
        code_description: Some(CodeDescription {
            href: Url::parse(d.code.docs_url).unwrap_or(fallback),
        }),
        source: Some("basilisk".to_owned()),
        message: d.message,
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

/// Convert a byte offset to an LSP position (UTF-16 code units).
fn byte_offset_to_position(text: &str, byte_offset: usize) -> Position {
    let clamped = byte_offset.min(text.len());
    let before = &text[..clamped];
    let line = u32::try_from(before.chars().filter(|&c| c == '\n').count()).unwrap_or(u32::MAX);
    let last_nl = before.rfind('\n').map_or(0, |p| p + 1);
    let character = before[last_nl..]
        .chars()
        .map(|c| if c as u32 > 0xFFFF { 2u32 } else { 1u32 })
        .sum::<u32>();
    Position { line, character }
}

/// Convert an LSP position to a byte offset.
fn position_to_byte_offset(text: &str, pos: Position) -> usize {
    let mut line = 0u32;
    let mut char_cu = 0u32; // UTF-16 code units on current line
    let mut byte_pos = 0usize;

    for (byte_idx, ch) in text.char_indices() {
        if line == pos.line && char_cu == pos.character {
            return byte_idx;
        }
        if ch == '\n' {
            line += 1;
            char_cu = 0;
            if line > pos.line {
                return byte_idx;
            }
        } else {
            char_cu += if ch as u32 > 0xFFFF { 2 } else { 1 };
        }
        byte_pos = byte_idx + ch.len_utf8();
    }
    byte_pos
}

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
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
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

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        self.documents
            .insert(uri.clone(), DocumentState { text: text.clone() });
        self.check_and_publish(uri, &text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let Some(change) = params.content_changes.into_iter().next() else { return; };
        let text = change.text;
        self.documents
            .insert(uri.clone(), DocumentState { text: text.clone() });
        self.check_and_publish(uri, &text).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(entry) = self.documents.get(&uri) {
            self.check_and_publish(uri, &entry.text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.documents.remove(&uri);
        self.client.publish_diagnostics(uri, vec![], None).await;
    }

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let Some(entry) = self.documents.get(&uri) else { return Ok(None); };
        let text = entry.text.clone();
        drop(entry); // release DashMap lock before await

        let byte_offset = position_to_byte_offset(&text, pos);
        let file_path = uri.to_file_path().unwrap_or_default();
        let path_str = file_path.to_string_lossy().into_owned();

        let Ok(parsed) = basilisk_parser::parse_source(text.clone(), path_str) else {
            return Ok(None);
        };
        let Ok(resolved) = basilisk_resolver::resolve(&parsed) else {
            return Ok(None);
        };
        let diags = basilisk_checker::check(&resolved);

        let hit = diags.into_iter().find(|d| {
            (d.span.start as usize) <= byte_offset && byte_offset < (d.span.end as usize)
        });

        let Some(d) = hit else { return Ok(None); };
        let mut md = format!("**{}** — {}", d.code.code, d.message);
        if let Some(help) = d.help {
            let _ = write!(md, "\n\n_{help}_");
        }

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: md,
            }),
            range: None,
        }))
    }

    async fn code_action(&self, params: CodeActionParams) -> LspResult<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let diags = params.context.diagnostics;
        let Some(entry) = self.documents.get(&uri) else { return Ok(None); };
        drop(entry);

        let mut actions: Vec<CodeActionOrCommand> = vec![];
        for diag in &diags {
            let Some(NumberOrString::String(code)) = &diag.code else { continue; };
            let action = match code.as_str() {
                "BSK-E0001" => Some(fix_missing_param_annotation(&uri, diag)),
                "BSK-E0002" => Some(fix_missing_return_annotation(&uri, diag)),
                _ => None,
            };
            if let Some(a) = action {
                actions.push(CodeActionOrCommand::CodeAction(a));
            }
        }
        Ok(if actions.is_empty() { None } else { Some(actions) })
    }
}

/// Create a code action for missing parameter annotation (BSK-E0001).
fn fix_missing_param_annotation(uri: &Url, diag: &Diagnostic) -> CodeAction {
    let insert_pos = diag.range.end;
    let mut changes = std::collections::HashMap::new();
    changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: Range {
                start: insert_pos,
                end: insert_pos,
            },
            new_text: ": Any".to_owned(),
        }],
    );
    CodeAction {
        title: "Add `: Any` annotation (basilisk)".to_owned(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(true),
        ..Default::default()
    }
}

/// Create a code action for missing return annotation (BSK-E0002).
fn fix_missing_return_annotation(uri: &Url, diag: &Diagnostic) -> CodeAction {
    let insert_pos = diag.range.start;
    let mut changes = std::collections::HashMap::new();
    changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: Range {
                start: insert_pos,
                end: insert_pos,
            },
            new_text: "-> None ".to_owned(),
        }],
    );
    CodeAction {
        title: "Add `-> None` return type (basilisk)".to_owned(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(true),
        ..Default::default()
    }
}

/// Start the LSP server.
///
/// Returns an error if the Tokio runtime cannot be created.
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
