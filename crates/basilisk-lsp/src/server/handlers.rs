//! LSP feature request handlers for the Basilisk LSP server.
//!
//! Covers hover, go-to-definition/declaration/type-definition, symbols,
//! signature help, references, highlights, rename, inlay hints, semantic tokens,
//! code actions, completion, formatting, folding, selection ranges, call
//! hierarchy, type hierarchy, code lens, and document color.

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    CodeActionParams, CodeActionResponse, CodeLens, CodeLensParams, ColorInformation,
    ColorPresentation, ColorPresentationParams, CompletionItem, CompletionParams,
    CompletionResponse, DocumentColorParams, DocumentFormattingParams, DocumentHighlight,
    DocumentHighlightParams, DocumentSymbolParams, DocumentSymbolResponse, FoldingRange,
    FoldingRangeParams, GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams,
    InlayHint, InlayHintParams, Location, PrepareRenameResponse, ReferenceParams, RenameParams,
    SemanticTokens, SemanticTokensParams, SemanticTokensResult, SelectionRange,
    SelectionRangeParams, SignatureHelpParams, SymbolInformation, TextDocumentPositionParams,
    TextEdit, TypeHierarchyItem, TypeHierarchyPrepareParams, TypeHierarchySubtypesParams,
    TypeHierarchySupertypesParams, WorkspaceSymbolParams, WorkspaceEdit,
};

use crate::{
    call_hierarchy, code_actions, code_lens, completion, declaration, definition, folding,
    formatting, highlight, hover, inlay_hints, references, selection, signature, symbols,
    type_definition, type_hierarchy,
};

use super::{none_if_empty, LspServer};

/// Handle `textDocument/hover`.
pub(super) async fn hover(server: &LspServer, params: HoverParams) -> LspResult<Option<Hover>> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;
    server
        .at_position(uri, pos, |resolved, text, offset, _, diags| {
            hover::hover_at(resolved, text, offset, diags)
        })
        .await
}

/// Handle `textDocument/definition`.
pub(super) async fn goto_definition(
    server: &LspServer,
    params: GotoDefinitionParams,
) -> LspResult<Option<GotoDefinitionResponse>> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;
    server
        .at_position(uri, pos, |resolved, text, offset, uri, _| {
            definition::goto_definition(resolved, text, offset, uri)
        })
        .await
}

/// Handle `textDocument/declaration`.
pub(super) async fn goto_declaration(
    server: &LspServer,
    params: GotoDefinitionParams,
) -> LspResult<Option<GotoDefinitionResponse>> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;
    server
        .at_position(uri, pos, |resolved, text, offset, uri, _| {
            declaration::goto_declaration(resolved, text, offset, uri)
        })
        .await
}

/// Handle `textDocument/typeDefinition`.
pub(super) async fn goto_type_definition(
    server: &LspServer,
    params: GotoDefinitionParams,
) -> LspResult<Option<GotoDefinitionResponse>> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;
    server
        .at_position(uri, pos, |resolved, text, offset, uri, _| {
            type_definition::goto_type_definition(resolved, text, offset, uri)
        })
        .await
}

/// Handle `textDocument/documentSymbol`.
pub(super) async fn document_symbol(
    server: &LspServer,
    params: DocumentSymbolParams,
) -> LspResult<Option<DocumentSymbolResponse>> {
    let uri = params.text_document.uri;
    let Some((text, resolved, _)) = server.get_document_data(&uri).await else {
        return Ok(None);
    };
    let syms = symbols::document_symbols(&resolved, &text);
    if syms.is_empty() {
        Ok(None)
    } else {
        Ok(Some(DocumentSymbolResponse::Nested(syms)))
    }
}

/// Handle `workspace/symbol`.
pub(super) async fn symbol(
    server: &LspServer,
    params: WorkspaceSymbolParams,
) -> LspResult<Option<Vec<SymbolInformation>>> {
    let query = &params.query;
    let docs = server
        .with_index(|idx| Some(idx.all_resolved()))
        .await
        .unwrap_or_default();
    Ok(none_if_empty(symbols::workspace_symbols(&docs, query)))
}

/// Handle `textDocument/signatureHelp`.
pub(super) async fn signature_help(
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

/// Handle `textDocument/references`.
pub(super) async fn references(
    server: &LspServer,
    params: ReferenceParams,
) -> LspResult<Option<Vec<Location>>> {
    let uri = params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;
    let include_decl = params.context.include_declaration;
    server
        .at_position(uri, pos, |resolved, text, offset, uri, _| {
            none_if_empty(references::find_references(
                resolved,
                text,
                offset,
                uri,
                include_decl,
            ))
        })
        .await
}

/// Handle `textDocument/documentHighlight`.
pub(super) async fn document_highlight(
    server: &LspServer,
    params: DocumentHighlightParams,
) -> LspResult<Option<Vec<DocumentHighlight>>> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;
    server
        .at_position(uri, pos, |resolved, text, offset, _, _| {
            none_if_empty(highlight::document_highlights(resolved, text, offset))
        })
        .await
}

/// Handle `textDocument/prepareRename`.
pub(super) async fn prepare_rename(
    server: &LspServer,
    params: TextDocumentPositionParams,
) -> LspResult<Option<PrepareRenameResponse>> {
    let uri = params.text_document.uri;
    let pos = params.position;
    server
        .at_position(uri, pos, |resolved, text, offset, _, _| {
            references::prepare_rename(resolved, text, offset)
        })
        .await
}

/// Handle `textDocument/rename`.
pub(super) async fn rename(
    server: &LspServer,
    params: RenameParams,
) -> LspResult<Option<WorkspaceEdit>> {
    let uri = params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;
    let new_name = params.new_name;
    server
        .at_position(uri, pos, |resolved, text, offset, uri, _| {
            references::rename_symbol(resolved, text, offset, uri, &new_name)
        })
        .await
}

/// Handle `textDocument/inlayHint`.
pub(super) async fn inlay_hint(
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
pub(super) async fn semantic_tokens_full(
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
pub(super) async fn code_action(
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
pub(super) async fn completion(
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

    let items = completion::complete(&resolved, &text, byte_offset);
    if items.is_empty() {
        Ok(None)
    } else {
        Ok(Some(CompletionResponse::Array(items)))
    }
}

/// Handle `completionItem/resolve`: lazily load documentation for a completion item.
///
/// Called when the user selects a completion item, allowing us to lazily load
/// documentation (docstrings) that wasn't included in the initial completion list.
pub(super) async fn completion_resolve(
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
pub(super) async fn formatting(
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
pub(super) async fn folding_range(
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
pub(super) async fn selection_range(
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

/// Handle `callHierarchy/prepare`.
pub(super) async fn prepare_call_hierarchy(
    server: &LspServer,
    params: CallHierarchyPrepareParams,
) -> LspResult<Option<Vec<CallHierarchyItem>>> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;
    server
        .at_position(uri, pos, |resolved, text, offset, uri, _| {
            none_if_empty(call_hierarchy::prepare(resolved, text, offset, uri))
        })
        .await
}

/// Handle `callHierarchy/incomingCalls`.
pub(super) async fn incoming_calls(
    server: &LspServer,
    params: CallHierarchyIncomingCallsParams,
) -> LspResult<Option<Vec<CallHierarchyIncomingCall>>> {
    let uri = params.item.uri.clone();
    let item_name = params.item.name.clone();
    let Some((text, resolved, _)) = server.get_document_data(&uri).await else {
        return Ok(None);
    };
    Ok(none_if_empty(call_hierarchy::incoming_calls(
        &resolved, &text, &item_name, &uri,
    )))
}

/// Handle `callHierarchy/outgoingCalls`.
pub(super) async fn outgoing_calls(
    server: &LspServer,
    params: CallHierarchyOutgoingCallsParams,
) -> LspResult<Option<Vec<CallHierarchyOutgoingCall>>> {
    let uri = params.item.uri.clone();
    let item_name = params.item.name.clone();
    let Some((text, resolved, _)) = server.get_document_data(&uri).await else {
        return Ok(None);
    };
    Ok(none_if_empty(call_hierarchy::outgoing_calls(
        &resolved, &text, &item_name, &uri,
    )))
}

/// Handle `textDocument/codeLens`.
pub(super) async fn code_lens(
    server: &LspServer,
    params: CodeLensParams,
) -> LspResult<Option<Vec<CodeLens>>> {
    let uri = params.text_document.uri;
    let Some((text, resolved, _)) = server.get_document_data(&uri).await else {
        return Ok(None);
    };
    Ok(none_if_empty(code_lens::code_lenses(&resolved, &text)))
}

/// Handle `typeHierarchy/prepare`.
pub(super) async fn prepare_type_hierarchy(
    server: &LspServer,
    params: TypeHierarchyPrepareParams,
) -> LspResult<Option<Vec<TypeHierarchyItem>>> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;
    server
        .at_position(uri, pos, |resolved, text, offset, uri, _| {
            none_if_empty(type_hierarchy::prepare(resolved, text, offset, uri))
        })
        .await
}

/// Handle `typeHierarchy/supertypes`.
pub(super) async fn supertypes(
    server: &LspServer,
    params: TypeHierarchySupertypesParams,
) -> LspResult<Option<Vec<TypeHierarchyItem>>> {
    let uri = params.item.uri.clone();
    let class_name = params
        .item
        .data
        .as_ref()
        .and_then(|d| d.as_str())
        .unwrap_or(&params.item.name);
    let Some((text, resolved, _)) = server.get_document_data(&uri).await else {
        return Ok(None);
    };
    Ok(none_if_empty(type_hierarchy::supertypes(
        &resolved, &text, class_name, &uri,
    )))
}

/// Handle `typeHierarchy/subtypes`.
pub(super) async fn subtypes(
    server: &LspServer,
    params: TypeHierarchySubtypesParams,
) -> LspResult<Option<Vec<TypeHierarchyItem>>> {
    let uri = params.item.uri.clone();
    let class_name = params
        .item
        .data
        .as_ref()
        .and_then(|d| d.as_str())
        .unwrap_or(&params.item.name);
    let Some((text, resolved, _)) = server.get_document_data(&uri).await else {
        return Ok(None);
    };
    Ok(none_if_empty(type_hierarchy::subtypes(
        &resolved, &text, class_name, &uri,
    )))
}

/// Handle `textDocument/documentColor`.
pub(super) async fn document_color(
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
pub(super) async fn color_presentation(
    _server: &LspServer,
    params: ColorPresentationParams,
) -> LspResult<Vec<ColorPresentation>> {
    Ok(crate::color::color_presentations(
        &params.color,
        &params.range,
    ))
}
