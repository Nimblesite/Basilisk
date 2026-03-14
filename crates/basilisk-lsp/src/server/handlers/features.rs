//! Feature-related LSP handlers: hover, signature help, completion, formatting,
//! code actions, inlay hints, semantic tokens, folding, selection ranges,
//! code lens, and document color.

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    CodeActionParams, CodeActionResponse, CodeLens, CodeLensParams, ColorInformation,
    ColorPresentation, ColorPresentationParams, CompletionItem, CompletionParams,
    CompletionResponse, DocumentColorParams, DocumentFormattingParams, FoldingRange,
    FoldingRangeParams, Hover, HoverParams, InlayHint, InlayHintParams, SelectionRange,
    SelectionRangeParams, SemanticTokens, SemanticTokensParams, SemanticTokensResult,
    SignatureHelpParams, TextEdit,
};

use crate::{
    auto_import, code_actions, code_lens, completion, folding, formatting, hover, inlay_hints,
    selection, signature,
};

use crate::server::{none_if_empty, LspServer};

/// Handle `textDocument/hover`.
pub(in crate::server) async fn hover(
    server: &LspServer,
    params: HoverParams,
) -> LspResult<Option<Hover>> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;
    server
        .at_position(uri, pos, |resolved, text, offset, _, diags| {
            hover::hover_at(resolved, text, offset, diags)
        })
        .await
}

/// Handle `textDocument/signatureHelp`.
pub(in crate::server) async fn signature_help(
    server: &LspServer,
    params: SignatureHelpParams,
) -> LspResult<Option<tower_lsp::lsp_types::SignatureHelp>> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;
    server
        .at_position(uri, pos, |resolved, text, offset, _, _| {
            signature::signature_help_at(resolved, text, offset)
        })
        .await
}

/// Handle `textDocument/inlayHint`.
pub(in crate::server) async fn inlay_hint(
    server: &LspServer,
    params: InlayHintParams,
) -> LspResult<Option<Vec<InlayHint>>> {
    let uri = params.text_document.uri;
    let Some((text, resolved, _)) = server.get_document_data(&uri).await else {
        return Ok(None);
    };
    Ok(none_if_empty(inlay_hints::inlay_hints(&resolved, &text)))
}

/// Handle `textDocument/semanticTokens/full`.
pub(in crate::server) async fn semantic_tokens_full(
    server: &LspServer,
    params: SemanticTokensParams,
) -> LspResult<Option<SemanticTokensResult>> {
    let uri = params.text_document.uri;
    let Some((text, resolved, _)) = server.get_document_data(&uri).await else {
        return Ok(None);
    };
    let tokens = crate::semantic_tokens::semantic_tokens(&resolved, &text);
    Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
        result_id: None,
        data: tokens,
    })))
}

/// Handle `textDocument/codeAction`.
pub(in crate::server) async fn code_action(
    server: &LspServer,
    params: CodeActionParams,
) -> LspResult<Option<CodeActionResponse>> {
    let uri = params.text_document.uri;
    let source = server
        .with_index(|idx| idx.get_text(&uri))
        .await
        .unwrap_or_default();
    Ok(none_if_empty(code_actions::code_actions(
        &uri,
        &params.context.diagnostics,
        &source,
    )))
}

/// Handle `textDocument/completion`.
pub(in crate::server) async fn completion(
    server: &LspServer,
    params: CompletionParams,
) -> LspResult<Option<CompletionResponse>> {
    let uri = params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;
    let Some(text) = server.with_index(|idx| idx.get_text(&uri)).await else {
        return Ok(None);
    };

    let byte_offset = crate::util::position_to_byte_offset(&text, pos);
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

    let mut items = completion::complete(&resolved, &text, byte_offset);

    // Add auto-import suggestions for unknown symbols from the workspace index.
    let prefix = extract_completion_prefix(&text, byte_offset);
    if !prefix.is_empty() {
        let auto_imports = server
            .with_index(|idx| {
                let symbol_index = auto_import::build_symbol_index(idx);
                Some(build_auto_import_items(
                    &symbol_index,
                    &prefix,
                    &text,
                    &file_path,
                ))
            })
            .await;
        if let Some(mut ai_items) = auto_imports {
            items.append(&mut ai_items);
        }
    }

    if items.is_empty() {
        Ok(None)
    } else {
        Ok(Some(CompletionResponse::Array(items)))
    }
}

/// Extract the identifier prefix at the cursor position for auto-import matching.
fn extract_completion_prefix(text: &str, byte_offset: usize) -> String {
    let before = text.get(..byte_offset).unwrap_or("");
    let start = before
        .rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map_or(0, |pos| pos + 1);
    before.get(start..byte_offset).unwrap_or("").to_owned()
}

/// Build auto-import completion items from the workspace symbol index.
fn build_auto_import_items(
    symbol_index: &auto_import::SymbolIndex,
    prefix: &str,
    source: &str,
    current_file: &std::path::Path,
) -> Vec<CompletionItem> {
    use tower_lsp::lsp_types::{
        CompletionItemKind, CompletionItemLabelDetails, InsertTextFormat, Range,
    };

    let candidates = auto_import::suggest_imports(symbol_index, prefix);
    let insert_offset = auto_import::find_import_insertion_offset(source);
    let insert_pos = byte_offset_to_position(source, insert_offset);

    candidates
        .into_iter()
        .filter(|sym| sym.file_path != current_file)
        .take(10)
        .map(|sym| {
            let import_text = auto_import::generate_import_text(sym);
            let kind = match sym.kind {
                auto_import::SymbolKind::Function => CompletionItemKind::FUNCTION,
                auto_import::SymbolKind::Class => CompletionItemKind::CLASS,
                auto_import::SymbolKind::Variable => CompletionItemKind::VARIABLE,
            };
            CompletionItem {
                label: sym.name.clone(),
                kind: Some(kind),
                label_details: Some(CompletionItemLabelDetails {
                    detail: Some(format!(" (auto-import from {})", sym.module_path)),
                    description: None,
                }),
                insert_text: Some(sym.name.clone()),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                additional_text_edits: Some(vec![TextEdit {
                    range: Range::new(insert_pos, insert_pos),
                    new_text: import_text,
                }]),
                sort_text: Some(format!("zz_{}", sym.name)),
                ..Default::default()
            }
        })
        .collect()
}

/// Convert a byte offset to an LSP `Position`.
fn byte_offset_to_position(source: &str, offset: usize) -> tower_lsp::lsp_types::Position {
    let clamped = offset.min(source.len());
    let before = source.get(..clamped).unwrap_or(source);
    let line = before.chars().filter(|&c| c == '\n').count() as u32;
    let col = before.rfind('\n').map_or(clamped, |pos| clamped - pos - 1) as u32;
    tower_lsp::lsp_types::Position::new(line, col)
}

/// Handle `completionItem/resolve`: lazily load documentation for a completion item.
///
/// Called when the user selects a completion item, allowing us to lazily load
/// documentation (docstrings) that wasn't included in the initial completion list.
pub(in crate::server) async fn completion_resolve(
    server: &LspServer,
    item: CompletionItem,
) -> LspResult<CompletionItem> {
    // Get the document text to resolve the module.
    // We need to find which document this completion belongs to.
    let (text, path_str) = server
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

/// Handle `textDocument/formatting`.
pub(in crate::server) async fn formatting(
    server: &LspServer,
    params: DocumentFormattingParams,
) -> LspResult<Option<Vec<TextEdit>>> {
    let uri = params.text_document.uri;
    let Some(text) = server.with_index(|idx| idx.get_text(&uri)).await else {
        return Ok(None);
    };
    let file_path = uri.to_file_path().unwrap_or_default();
    let path_str = file_path.to_string_lossy().into_owned();
    Ok(formatting::format_document(&text, &path_str))
}

/// Handle `textDocument/foldingRange`.
pub(in crate::server) async fn folding_range(
    server: &LspServer,
    params: FoldingRangeParams,
) -> LspResult<Option<Vec<FoldingRange>>> {
    let uri = params.text_document.uri;
    let Some((text, resolved, _)) = server.get_document_data(&uri).await else {
        return Ok(None);
    };
    Ok(none_if_empty(folding::folding_ranges(&resolved, &text)))
}

/// Handle `textDocument/selectionRange`.
pub(in crate::server) async fn selection_range(
    server: &LspServer,
    params: SelectionRangeParams,
) -> LspResult<Option<Vec<SelectionRange>>> {
    let uri = params.text_document.uri;
    let Some((text, resolved, _)) = server.get_document_data(&uri).await else {
        return Ok(None);
    };
    Ok(none_if_empty(selection::selection_ranges(
        &resolved,
        &text,
        &params.positions,
    )))
}

/// Handle `textDocument/codeLens`.
pub(in crate::server) async fn code_lens(
    server: &LspServer,
    params: CodeLensParams,
) -> LspResult<Option<Vec<CodeLens>>> {
    let uri = params.text_document.uri;
    let Some((text, resolved, _)) = server.get_document_data(&uri).await else {
        return Ok(None);
    };
    Ok(none_if_empty(code_lens::code_lenses(&resolved, &text)))
}

/// Handle `textDocument/documentColor`.
pub(in crate::server) async fn document_color(
    server: &LspServer,
    params: DocumentColorParams,
) -> LspResult<Vec<ColorInformation>> {
    let uri = params.text_document.uri;
    let source = server
        .with_index(|idx| idx.get_text(&uri))
        .await
        .unwrap_or_default();
    Ok(crate::color::document_colors(&source))
}

/// Handle `textDocument/colorPresentation`.
pub(in crate::server) async fn color_presentation(
    _server: &LspServer,
    params: ColorPresentationParams,
) -> LspResult<Vec<ColorPresentation>> {
    Ok(crate::color::color_presentations(
        &params.color,
        &params.range,
    ))
}
