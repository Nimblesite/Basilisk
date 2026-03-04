//! Semantic token computation for the Basilisk LSP server.
//!
//! Classifies symbol name spans from the resolved module into LSP semantic
//! token types (function, class, parameter, variable, property, method).

use basilisk_resolver::ResolvedModule;
use tower_lsp::lsp_types::{SemanticToken, SemanticTokenModifier, SemanticTokenType};

use crate::util::byte_offset_to_position;

/// Token type legend — order matters (index = `token_type` ID).
pub static TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::FUNCTION,   // 0
    SemanticTokenType::METHOD,     // 1
    SemanticTokenType::CLASS,      // 2
    SemanticTokenType::PARAMETER,  // 3
    SemanticTokenType::VARIABLE,   // 4
    SemanticTokenType::PROPERTY,   // 5
    SemanticTokenType::NAMESPACE,  // 6
];

/// Token modifier legend — order matters (bit position = index).
pub static TOKEN_MODIFIERS: &[SemanticTokenModifier] = &[
    SemanticTokenModifier::DEFINITION, // bit 0
    SemanticTokenModifier::READONLY,   // bit 1
];

/// Index constants for token types.
const TT_FUNCTION: u32 = 0;
const TT_METHOD: u32 = 1;
const TT_CLASS: u32 = 2;
const TT_PARAMETER: u32 = 3;
const TT_VARIABLE: u32 = 4;
const TT_PROPERTY: u32 = 5;
const TT_NAMESPACE: u32 = 6;

/// Modifier bit flags.
const MOD_DEFINITION: u32 = 1 << 0;

/// A raw token before delta encoding.
struct RawToken {
    /// Byte offset in source.
    byte_offset: u32,
    /// Length in bytes.
    length: u32,
    /// Token type index.
    token_type: u32,
    /// Modifier bitset.
    modifiers: u32,
}

/// Compute semantic tokens for a resolved module.
///
/// Returns delta-encoded tokens sorted by position, ready for the LSP response.
#[must_use]
pub fn semantic_tokens(resolved: &ResolvedModule, source: &str) -> Vec<SemanticToken> {
    let mut raw: Vec<RawToken> = Vec::new();

    // Functions and methods.
    for func in &resolved.functions {
        let tt = if func.class_name.is_some() {
            TT_METHOD
        } else {
            TT_FUNCTION
        };
        raw.push(RawToken {
            byte_offset: func.name_span.start,
            length: func.name_span.end.saturating_sub(func.name_span.start),
            token_type: tt,
            modifiers: MOD_DEFINITION,
        });

        // Parameters.
        for param in &func.parameters {
            raw.push(RawToken {
                byte_offset: param.name_span.start,
                length: param.name_span.end.saturating_sub(param.name_span.start),
                token_type: TT_PARAMETER,
                modifiers: MOD_DEFINITION,
            });
        }
        if let Some(ref va) = func.vararg {
            raw.push(RawToken {
                byte_offset: va.name_span.start,
                length: va.name_span.end.saturating_sub(va.name_span.start),
                token_type: TT_PARAMETER,
                modifiers: MOD_DEFINITION,
            });
        }
        if let Some(ref kw) = func.kwarg {
            raw.push(RawToken {
                byte_offset: kw.name_span.start,
                length: kw.name_span.end.saturating_sub(kw.name_span.start),
                token_type: TT_PARAMETER,
                modifiers: MOD_DEFINITION,
            });
        }
    }

    // Classes.
    for class in &resolved.classes {
        raw.push(RawToken {
            byte_offset: class.name_span.start,
            length: class.name_span.end.saturating_sub(class.name_span.start),
            token_type: TT_CLASS,
            modifiers: MOD_DEFINITION,
        });

        // Attributes (properties).
        for attr in &class.attributes {
            raw.push(RawToken {
                byte_offset: attr.name_span.start,
                length: attr.name_span.end.saturating_sub(attr.name_span.start),
                token_type: TT_PROPERTY,
                modifiers: MOD_DEFINITION,
            });
        }
    }

    // Module variables.
    for var in &resolved.module_vars {
        raw.push(RawToken {
            byte_offset: var.name_span.start,
            length: var.name_span.end.saturating_sub(var.name_span.start),
            token_type: TT_VARIABLE,
            modifiers: MOD_DEFINITION,
        });
    }

    // Imports (as namespace tokens).
    for imp in &resolved.imports {
        raw.push(RawToken {
            byte_offset: imp.span.start,
            length: imp.span.end.saturating_sub(imp.span.start),
            token_type: TT_NAMESPACE,
            modifiers: 0,
        });
    }

    // Sort by byte offset for delta encoding.
    raw.sort_by_key(|t| t.byte_offset);

    // Delta-encode positions.
    let mut prev_line = 0u32;
    let mut prev_start = 0u32;
    let mut tokens = Vec::with_capacity(raw.len());

    for rt in &raw {
        let pos = byte_offset_to_position(source, rt.byte_offset as usize);
        let delta_line = pos.line.saturating_sub(prev_line);
        let delta_start = if delta_line == 0 {
            pos.character.saturating_sub(prev_start)
        } else {
            pos.character
        };

        tokens.push(SemanticToken {
            delta_line,
            delta_start,
            length: rt.length,
            token_type: rt.token_type,
            token_modifiers_bitset: rt.modifiers,
        });

        prev_line = pos.line;
        prev_start = pos.character;
    }

    tokens
}
