//! Navigation-related LSP handlers: go-to-definition, declaration, type-definition,
//! references, highlights, rename, symbols, call hierarchy, and type hierarchy.

use std::collections::HashSet;

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    DocumentHighlight, DocumentHighlightParams, DocumentSymbolParams, DocumentSymbolResponse,
    GotoDefinitionParams, GotoDefinitionResponse, Location, PrepareRenameResponse, ReferenceParams,
    RenameParams, SymbolInformation, TextDocumentPositionParams, TypeHierarchyItem,
    TypeHierarchyPrepareParams, TypeHierarchySubtypesParams, TypeHierarchySupertypesParams, Url,
    WorkspaceEdit, WorkspaceSymbolParams,
};

use crate::{
    call_hierarchy, declaration, definition, highlight, references, symbols, type_definition,
    type_hierarchy,
};

use crate::server::{none_if_empty, LspServer};

/// Handle `textDocument/definition`.
///
/// Tries single-file definition first. If no local definition is found,
/// falls back to cross-file lookup via `imported_symbols` populated by
/// cross-module analysis.
pub(in crate::server) async fn goto_definition(
    server: &LspServer,
    params: GotoDefinitionParams,
) -> LspResult<Option<GotoDefinitionResponse>> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;
    Ok(server
        .with_index(|idx| {
            let (text, resolved, _) = idx.get_by_uri(&uri)?;
            let byte_offset = crate::util::position_to_byte_offset(&text, pos);

            // Try single-file definition first.
            if let Some(resp) = definition::goto_definition(&resolved, &text, byte_offset, &uri) {
                return Some(resp);
            }

            // Cross-file: extract identifier and check imported symbols.
            let name = crate::util::identifier_at_offset(&text, byte_offset)?;
            let ext_sym = resolved.imported_symbols.get(&name)?;

            // Follow re-export chain: if the symbol in the target file is itself
            // imported from another file, follow through to the actual definition.
            let (final_path, final_span) =
                follow_reexport_chain(idx, &ext_sym.source_path, &name, ext_sym.source_span);

            let target_entry = idx.files.get(&final_path)?;
            let range = crate::util::span_to_range(&target_entry.text, final_span);
            let target_uri = Url::from_file_path(&final_path).ok()?;

            Some(GotoDefinitionResponse::Scalar(Location {
                uri: target_uri,
                range,
            }))
        })
        .await)
}

/// Follow re-export chains to find the actual definition site.
///
/// If the symbol at `source_path` is itself an import from another file,
/// follows the chain until a non-imported definition is found. Prevents
/// infinite loops with a depth limit.
fn follow_reexport_chain(
    idx: &crate::workspace::WorkspaceIndex,
    source_path: &std::path::Path,
    name: &str,
    source_span: basilisk_resolver::Span,
) -> (std::path::PathBuf, basilisk_resolver::Span) {
    let mut current_path = source_path.to_path_buf();
    let mut current_span = source_span;

    // Follow up to 10 levels of re-exports to avoid infinite loops.
    for _ in 0..10 {
        let Some(entry) = idx.files.get(&current_path) else {
            break;
        };
        let Some(resolved) = &entry.resolved else {
            break;
        };

        // Check if this symbol is imported from yet another file.
        if let Some(ext_sym) = resolved.imported_symbols.get(name) {
            current_path = ext_sym.source_path.clone();
            current_span = ext_sym.source_span;
        } else {
            // Symbol is defined here (not imported) — this is the final definition.
            break;
        }
    }

    (current_path, current_span)
}

/// Handle `textDocument/declaration`.
pub(in crate::server) async fn goto_declaration(
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
pub(in crate::server) async fn goto_type_definition(
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
pub(in crate::server) async fn document_symbol(
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
pub(in crate::server) async fn symbol(
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

/// Handle `textDocument/references`.
///
/// Finds single-file references first, then searches cross-file via the
/// import graph — checking all importers of the current file for usage of
/// the symbol, and the source file if the symbol is imported.
pub(in crate::server) async fn references(
    server: &LspServer,
    params: ReferenceParams,
) -> LspResult<Option<Vec<Location>>> {
    let uri = params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;
    let include_decl = params.context.include_declaration;
    Ok(server
        .with_index(|idx| {
            let (text, resolved, _) = idx.get_by_uri(&uri)?;
            let byte_offset = crate::util::position_to_byte_offset(&text, pos);

            // Single-file references.
            let mut locations =
                references::find_references(&resolved, &text, byte_offset, &uri, include_decl);

            // Extract the symbol name for cross-file search.
            let name = crate::util::identifier_at_offset(&text, byte_offset)?;
            let current_path = uri.to_file_path().ok()?;

            // Track seen locations to avoid O(n) dedup with contains().
            let mut seen: HashSet<(Url, u32, u32)> = locations
                .iter()
                .map(|loc| {
                    (
                        loc.uri.clone(),
                        loc.range.start.line,
                        loc.range.start.character,
                    )
                })
                .collect();

            // Cross-file: search importers of this file for the symbol.
            if let Ok(graph) = idx.import_graph.lock() {
                for importer_path in graph.importers_of(&current_path) {
                    if let Some(entry) = idx.files.get(&importer_path) {
                        // Only search if the importer has this symbol in imported_symbols.
                        if let Some(ref res) = entry.resolved {
                            if res.imported_symbols.contains_key(&name) {
                                if let Some(importer_uri) =
                                    crate::workspace_scan::path_to_uri(&importer_path)
                                {
                                    let ranges =
                                        references::find_identifier_occurrences(&entry.text, &name);
                                    for range in ranges {
                                        let key = (
                                            importer_uri.clone(),
                                            range.start.line,
                                            range.start.character,
                                        );
                                        if seen.insert(key) {
                                            locations.push(Location {
                                                uri: importer_uri.clone(),
                                                range,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // If the symbol is imported, also search the source file for its definition.
            if let Some(ext_sym) = resolved.imported_symbols.get(&name) {
                if ext_sym.source_path != current_path {
                    if let Some(entry) = idx.files.get(&ext_sym.source_path) {
                        if let Some(source_uri) =
                            crate::workspace_scan::path_to_uri(&ext_sym.source_path)
                        {
                            if include_decl {
                                let def_range =
                                    crate::util::span_to_range(&entry.text, ext_sym.source_span);
                                let key = (
                                    source_uri.clone(),
                                    def_range.start.line,
                                    def_range.start.character,
                                );
                                if seen.insert(key) {
                                    locations.push(Location {
                                        uri: source_uri.clone(),
                                        range: def_range,
                                    });
                                }
                            }
                            // Also find usage references in the source file.
                            let ranges =
                                references::find_identifier_occurrences(&entry.text, &name);
                            for range in ranges {
                                let key =
                                    (source_uri.clone(), range.start.line, range.start.character);
                                if seen.insert(key) {
                                    locations.push(Location {
                                        uri: source_uri.clone(),
                                        range,
                                    });
                                }
                            }
                        }
                    }
                }
            }

            none_if_empty(locations)
        })
        .await)
}

/// Handle `textDocument/documentHighlight`.
pub(in crate::server) async fn document_highlight(
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
pub(in crate::server) async fn prepare_rename(
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
///
/// Renames the symbol at the cursor across all files: definition site,
/// import sites, and usage sites in importers.
pub(in crate::server) async fn rename(
    server: &LspServer,
    params: RenameParams,
) -> LspResult<Option<WorkspaceEdit>> {
    let uri = params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;
    let new_name = params.new_name;
    Ok(server
        .with_index(|idx| {
            let (text, resolved, _) = idx.get_by_uri(&uri)?;
            let byte_offset = crate::util::position_to_byte_offset(&text, pos);

            // Single-file rename edits.
            let local_edit =
                references::rename_symbol(&resolved, &text, byte_offset, &uri, &new_name)?;
            let mut all_changes: std::collections::HashMap<
                Url,
                Vec<tower_lsp::lsp_types::TextEdit>,
            > = local_edit.changes.unwrap_or_default();

            // Extract the old symbol name for cross-file search.
            let name = crate::util::identifier_at_offset(&text, byte_offset)?;
            let current_path = uri.to_file_path().ok()?;

            // Cross-file: rename in all importers of this file.
            if let Ok(graph) = idx.import_graph.lock() {
                for importer_path in graph.importers_of(&current_path) {
                    if let Some(entry) = idx.files.get(&importer_path) {
                        if let Some(ref res) = entry.resolved {
                            if res.imported_symbols.contains_key(&name) {
                                if let Some(importer_uri) =
                                    crate::workspace_scan::path_to_uri(&importer_path)
                                {
                                    let edits: Vec<tower_lsp::lsp_types::TextEdit> =
                                        references::find_identifier_occurrences(&entry.text, &name)
                                            .into_iter()
                                            .map(|range| tower_lsp::lsp_types::TextEdit {
                                                range,
                                                new_text: new_name.clone(),
                                            })
                                            .collect();
                                    if !edits.is_empty() {
                                        all_changes.entry(importer_uri).or_default().extend(edits);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // If the symbol is imported, also rename at the source definition.
            if let Some(ext_sym) = resolved.imported_symbols.get(&name) {
                if ext_sym.source_path != current_path {
                    if let Some(entry) = idx.files.get(&ext_sym.source_path) {
                        if let Some(source_uri) =
                            crate::workspace_scan::path_to_uri(&ext_sym.source_path)
                        {
                            let edits: Vec<tower_lsp::lsp_types::TextEdit> =
                                references::find_identifier_occurrences(&entry.text, &name)
                                    .into_iter()
                                    .map(|range| tower_lsp::lsp_types::TextEdit {
                                        range,
                                        new_text: new_name.clone(),
                                    })
                                    .collect();
                            if !edits.is_empty() {
                                all_changes.entry(source_uri).or_default().extend(edits);
                            }
                        }
                    }
                }
            }

            if all_changes.is_empty() {
                return None;
            }

            Some(WorkspaceEdit {
                changes: Some(all_changes),
                ..Default::default()
            })
        })
        .await)
}

/// Handle `callHierarchy/prepare`.
pub(in crate::server) async fn prepare_call_hierarchy(
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
pub(in crate::server) async fn incoming_calls(
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
pub(in crate::server) async fn outgoing_calls(
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

/// Handle `typeHierarchy/prepare`.
pub(in crate::server) async fn prepare_type_hierarchy(
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
pub(in crate::server) async fn supertypes(
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
pub(in crate::server) async fn subtypes(
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
