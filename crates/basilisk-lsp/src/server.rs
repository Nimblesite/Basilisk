//! Language Server Protocol server implementation for Basilisk.
//!
//! This module provides a full LSP server using tower-lsp that can be
//! started via the `basilisk lsp` CLI subcommand.

use std::fmt::Write as _;

use dashmap::DashMap;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams,
    CodeActionProviderCapability, CodeActionResponse, CodeDescription, CompletionItem,
    CompletionItemKind, CompletionOptions, CompletionParams, CompletionResponse, Diagnostic,
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
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".to_owned()]),
                    ..Default::default()
                }),
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

        let prefix = extract_prefix(&text, byte_offset);
        let is_dot = is_dot_completion(&text, byte_offset);

        // Try parsing the document as-is; if it fails (incomplete expression at
        // cursor), remove the cursor line and retry so we still get the resolved
        // symbol table for completion.
        let resolved = try_resolve(&text, &path_str)
            .or_else(|| {
                let patched = patch_cursor_line(&text, pos.line);
                try_resolve(&patched, &path_str)
            });

        let Some(resolved) = resolved else {
            return Ok(None);
        };

        let items = if is_dot {
            dot_completions(&resolved, &text, byte_offset)
        } else {
            symbol_completions(&resolved, &prefix)
        };

        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(CompletionResponse::Array(items)))
        }
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

/// Try to parse and resolve a source string, returning `None` on failure.
fn try_resolve(text: &str, path: &str) -> Option<basilisk_resolver::ResolvedModule> {
    let parsed = basilisk_parser::parse_source(text.to_owned(), path.to_owned()).ok()?;
    basilisk_resolver::resolve(&parsed).ok()
}

/// Replace the line at `line_number` (0-based) with `pass` (preserving indentation).
///
/// This keeps the file structurally valid when the cursor line has an
/// incomplete expression like `self.` or `obj.`.
fn patch_cursor_line(text: &str, line_number: u32) -> String {
    text.lines()
        .enumerate()
        .map(|(idx, line)| {
            if idx == line_number as usize {
                let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
                format!("{indent}pass")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extract the identifier prefix before the cursor position.
fn extract_prefix(text: &str, byte_offset: usize) -> String {
    let before = &text[..byte_offset.min(text.len())];
    before
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

/// Check if the cursor is immediately after a `.` (dot completion context).
fn is_dot_completion(text: &str, byte_offset: usize) -> bool {
    let before = &text[..byte_offset.min(text.len())];
    // Skip any partial identifier the user has typed after the dot
    let stripped = before.trim_end_matches(|c: char| c.is_alphanumeric() || c == '_');
    stripped.ends_with('.')
}

/// Extract the receiver name before a dot (e.g. `self` from `self.`).
fn dot_receiver(text: &str, byte_offset: usize) -> Option<String> {
    let before = &text[..byte_offset.min(text.len())];
    let stripped = before.trim_end_matches(|c: char| c.is_alphanumeric() || c == '_');
    let before_dot = stripped.strip_suffix('.')?;
    let name: String = before_dot
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if name.is_empty() { None } else { Some(name) }
}

/// Find which class contains the method at a byte offset (for `self.` completion).
fn enclosing_class(
    resolved: &basilisk_resolver::ResolvedModule,
    offset: usize,
) -> Option<&basilisk_resolver::scope::ClassInfo> {
    // Find a method whose body contains this offset, then look up its class
    let func = resolved.functions.iter().find(|f| {
        f.class_name.is_some()
            && (f.def_span.start as usize) <= offset
    });
    if let Some(func) = func {
        if let Some(class_name) = &func.class_name {
            return resolved.classes.iter().find(|c| &c.name == class_name);
        }
    }
    None
}

/// Emit completion items for all attributes and methods of a class.
fn class_member_items(
    class: &basilisk_resolver::scope::ClassInfo,
    prefix: &str,
) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    for attr in &class.attributes {
        if !prefix.is_empty() && !attr.name.starts_with(prefix) {
            continue;
        }
        items.push(CompletionItem {
            label: attr.name.clone(),
            kind: Some(CompletionItemKind::FIELD),
            detail: Some(format!("{}.{}", class.name, attr.name)),
            ..Default::default()
        });
    }
    for method_name in &class.method_names {
        if !prefix.is_empty() && !method_name.starts_with(prefix) {
            continue;
        }
        items.push(CompletionItem {
            label: method_name.clone(),
            kind: Some(CompletionItemKind::METHOD),
            detail: Some(format!("{}.{}", class.name, method_name)),
            ..Default::default()
        });
    }
    items
}

/// Provide dot-completion items (class attributes and methods).
fn dot_completions(
    resolved: &basilisk_resolver::ResolvedModule,
    text: &str,
    byte_offset: usize,
) -> Vec<CompletionItem> {
    let receiver = dot_receiver(text, byte_offset);
    let prefix = extract_prefix(text, byte_offset);

    if receiver.as_deref() == Some("self") {
        // `self.` — find the enclosing class
        enclosing_class(resolved, byte_offset)
            .map(|c| class_member_items(c, &prefix))
            .unwrap_or_default()
    } else if let Some(ref recv_name) = receiver {
        // `ClassName.` — look up the class by name
        resolved
            .classes
            .iter()
            .find(|c| &c.name == recv_name)
            .map(|c| class_member_items(c, &prefix))
            .unwrap_or_default()
    } else {
        vec![]
    }
}

/// Provide non-dot completion items: functions, classes, variables, imports, builtins.
fn symbol_completions(
    resolved: &basilisk_resolver::ResolvedModule,
    prefix: &str,
) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Functions
    for func in &resolved.functions {
        if func.class_name.is_some() {
            continue; // Skip methods — they come via dot-completion
        }
        if !prefix.is_empty() && !func.name.starts_with(prefix) {
            continue;
        }
        if seen.insert(func.name.clone()) {
            let mut detail = String::from("(");
            for (idx, param) in func.parameters.iter().enumerate() {
                if idx > 0 {
                    detail.push_str(", ");
                }
                detail.push_str(&param.name);
            }
            detail.push(')');
            items.push(CompletionItem {
                label: func.name.clone(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some(detail),
                ..Default::default()
            });
        }
    }

    // Classes
    for class in &resolved.classes {
        if !prefix.is_empty() && !class.name.starts_with(prefix) {
            continue;
        }
        if seen.insert(class.name.clone()) {
            items.push(CompletionItem {
                label: class.name.clone(),
                kind: Some(CompletionItemKind::CLASS),
                detail: Some("class".to_owned()),
                ..Default::default()
            });
        }
    }

    // Module variables
    for var in &resolved.module_vars {
        if !prefix.is_empty() && !var.name.starts_with(prefix) {
            continue;
        }
        if seen.insert(var.name.clone()) {
            items.push(CompletionItem {
                label: var.name.clone(),
                kind: Some(CompletionItemKind::VARIABLE),
                detail: Some("variable".to_owned()),
                ..Default::default()
            });
        }
    }

    // Imported names
    for imp in &resolved.imports {
        match imp.kind {
            basilisk_resolver::scope::ImportKind::Plain => {
                // `import os` → complete `os`
                let name = imp.module.split('.').next().unwrap_or(&imp.module);
                if !prefix.is_empty() && !name.starts_with(prefix) {
                    continue;
                }
                if seen.insert(name.to_owned()) {
                    items.push(CompletionItem {
                        label: name.to_owned(),
                        kind: Some(CompletionItemKind::MODULE),
                        detail: Some(format!("import {}", imp.module)),
                        ..Default::default()
                    });
                }
            }
            basilisk_resolver::scope::ImportKind::From => {
                // `from X import a, b` → complete `a`, `b`
                for name in &imp.names {
                    if !prefix.is_empty() && !name.starts_with(prefix) {
                        continue;
                    }
                    if seen.insert(name.clone()) {
                        items.push(CompletionItem {
                            label: name.clone(),
                            kind: Some(CompletionItemKind::VARIABLE),
                            detail: Some(format!("from {} import", imp.module)),
                            ..Default::default()
                        });
                    }
                }
            }
            basilisk_resolver::scope::ImportKind::Star => {}
        }
    }

    // Python builtins
    add_builtin_completions(&mut items, &mut seen, prefix);

    items
}

/// Add common Python built-in names as completion items.
fn add_builtin_completions(
    items: &mut Vec<CompletionItem>,
    seen: &mut std::collections::HashSet<String>,
    prefix: &str,
) {
    const BUILTIN_FUNCTIONS: &[&str] = &[
        "abs", "all", "any", "bin", "bool", "breakpoint", "bytearray", "bytes",
        "callable", "chr", "classmethod", "compile", "complex", "delattr", "dict",
        "dir", "divmod", "enumerate", "eval", "exec", "filter", "float", "format",
        "frozenset", "getattr", "globals", "hasattr", "hash", "help", "hex", "id",
        "input", "int", "isinstance", "issubclass", "iter", "len", "list", "locals",
        "map", "max", "memoryview", "min", "next", "object", "oct", "open", "ord",
        "pow", "print", "property", "range", "repr", "reversed", "round", "set",
        "setattr", "slice", "sorted", "staticmethod", "str", "sum", "super",
        "tuple", "type", "vars", "zip",
    ];
    const BUILTIN_CONSTANTS: &[&str] = &["True", "False", "None", "NotImplemented", "Ellipsis"];
    const BUILTIN_EXCEPTIONS: &[&str] = &[
        "Exception", "BaseException", "ValueError", "TypeError", "KeyError",
        "IndexError", "AttributeError", "ImportError", "OSError", "RuntimeError",
        "StopIteration", "ArithmeticError", "LookupError", "SyntaxError",
        "NameError", "FileNotFoundError", "NotImplementedError", "OverflowError",
        "ZeroDivisionError", "RecursionError", "PermissionError", "TimeoutError",
    ];

    for &name in BUILTIN_FUNCTIONS {
        if !prefix.is_empty() && !name.starts_with(prefix) {
            continue;
        }
        if seen.insert(name.to_owned()) {
            items.push(CompletionItem {
                label: name.to_owned(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some("built-in".to_owned()),
                ..Default::default()
            });
        }
    }
    for &name in BUILTIN_CONSTANTS {
        if !prefix.is_empty() && !name.starts_with(prefix) {
            continue;
        }
        if seen.insert(name.to_owned()) {
            items.push(CompletionItem {
                label: name.to_owned(),
                kind: Some(CompletionItemKind::CONSTANT),
                detail: Some("built-in".to_owned()),
                ..Default::default()
            });
        }
    }
    for &name in BUILTIN_EXCEPTIONS {
        if !prefix.is_empty() && !name.starts_with(prefix) {
            continue;
        }
        if seen.insert(name.to_owned()) {
            items.push(CompletionItem {
                label: name.to_owned(),
                kind: Some(CompletionItemKind::CLASS),
                detail: Some("built-in exception".to_owned()),
                ..Default::default()
            });
        }
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
