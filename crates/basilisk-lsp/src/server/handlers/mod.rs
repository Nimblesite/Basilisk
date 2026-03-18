//! LSP request handlers — split into navigation and feature submodules.
//!
//! Navigation: go-to-definition, declaration, type-definition, references,
//! highlights, rename, symbols, call hierarchy, type hierarchy.
//!
//! Features: hover, signature help, completion, formatting, code actions,
//! inlay hints, semantic tokens, folding, selection ranges, code lens, colors.

mod features;
mod navigation;

pub(super) use features::{
    code_action, code_lens, color_presentation, completion, completion_resolve, document_color,
    folding_range, formatting, hover, inlay_hint, selection_range, semantic_tokens_full,
    signature_help,
};

pub(super) use navigation::{
    document_highlight, document_symbol, goto_declaration, goto_definition, goto_type_definition,
    incoming_calls, outgoing_calls, prepare_call_hierarchy, prepare_rename, prepare_type_hierarchy,
    references, rename, subtypes, supertypes, symbol,
};
