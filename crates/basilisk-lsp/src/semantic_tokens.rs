//! Semantic token computation for the Basilisk LSP server.
//!
//! Classifies symbol name spans from the resolved module into LSP semantic
//! token types (function, class, parameter, variable, property, method,
//! decorator, type, typeParameter) with modifiers (definition, readonly,
//! declaration, static, deprecated).

use basilisk_resolver::scope::{FunctionInfo, ParameterInfo, Span};
use basilisk_resolver::ResolvedModule;
use tower_lsp::lsp_types::{SemanticToken, SemanticTokenModifier, SemanticTokenType};

use crate::util::byte_offset_to_position;

/// Token type legend — order matters (index = `token_type` ID).
pub static TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::FUNCTION,       // 0
    SemanticTokenType::METHOD,         // 1
    SemanticTokenType::CLASS,          // 2
    SemanticTokenType::PARAMETER,      // 3
    SemanticTokenType::VARIABLE,       // 4
    SemanticTokenType::PROPERTY,       // 5
    SemanticTokenType::NAMESPACE,      // 6
    SemanticTokenType::DECORATOR,      // 7
    SemanticTokenType::TYPE,           // 8
    SemanticTokenType::TYPE_PARAMETER, // 9
];

/// Token modifier legend — order matters (bit position = index).
pub static TOKEN_MODIFIERS: &[SemanticTokenModifier] = &[
    SemanticTokenModifier::DEFINITION,  // bit 0
    SemanticTokenModifier::READONLY,    // bit 1
    SemanticTokenModifier::DECLARATION, // bit 2
    SemanticTokenModifier::STATIC,      // bit 3
    SemanticTokenModifier::DEPRECATED,  // bit 4
];

/// Index constants for token types.
const TT_FUNCTION: u32 = 0;
const TT_METHOD: u32 = 1;
const TT_CLASS: u32 = 2;
const TT_PARAMETER: u32 = 3;
const TT_VARIABLE: u32 = 4;
const TT_PROPERTY: u32 = 5;
const TT_NAMESPACE: u32 = 6;
const TT_DECORATOR: u32 = 7;
const TT_TYPE: u32 = 8;
const TT_TYPE_PARAMETER: u32 = 9;

/// Modifier bit flags.
const MOD_DEFINITION: u32 = 1 << 0;
const MOD_DECLARATION: u32 = 1 << 2;
const MOD_STATIC: u32 = 1 << 3;

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

/// Create a `RawToken` from a span, token type, and modifier bitset.
fn span_token(span: Span, token_type: u32, modifiers: u32) -> RawToken {
    RawToken {
        byte_offset: span.start,
        length: span.end.saturating_sub(span.start),
        token_type,
        modifiers,
    }
}

/// Emit a parameter token and its type annotation token (if present).
fn push_param_tokens(raw: &mut Vec<RawToken>, param: &ParameterInfo) {
    raw.push(span_token(param.name_span, TT_PARAMETER, MOD_DEFINITION));
    if let Some(ann_span) = param.annotation_span {
        raw.push(span_token(ann_span, TT_TYPE, 0));
    }
}

/// Check if a function has `@staticmethod` or `@classmethod` decorator.
fn has_static_decorator(decorators: &[String]) -> bool {
    decorators
        .iter()
        .any(|d| d == "staticmethod" || d == "classmethod")
}

/// Collect tokens for a single function or method definition.
fn collect_function_tokens(raw: &mut Vec<RawToken>, func: &FunctionInfo) {
    let tt = if func.class_name.is_some() {
        TT_METHOD
    } else {
        TT_FUNCTION
    };
    let mut name_mods = MOD_DEFINITION | MOD_DECLARATION;
    if has_static_decorator(&func.decorators) {
        name_mods |= MOD_STATIC;
    }
    raw.push(span_token(func.name_span, tt, name_mods));

    // Decorator tokens.
    for (_, span) in &func.decorator_spans {
        raw.push(span_token(*span, TT_DECORATOR, 0));
    }

    // Parameters.
    for param in &func.parameters {
        push_param_tokens(raw, param);
    }
    if let Some(ref va) = func.vararg {
        push_param_tokens(raw, va);
    }
    if let Some(ref kw) = func.kwarg {
        push_param_tokens(raw, kw);
    }

    // Return type annotation.
    if let Some(ret_span) = func.return_annotation_span {
        raw.push(span_token(ret_span, TT_TYPE, 0));
    }
}

/// Compute semantic tokens for a resolved module.
///
/// Returns delta-encoded tokens sorted by position, ready for the LSP response.
#[must_use]
pub fn semantic_tokens(resolved: &ResolvedModule, source: &str) -> Vec<SemanticToken> {
    let mut raw: Vec<RawToken> = Vec::new();

    for func in &resolved.functions {
        collect_function_tokens(&mut raw, func);
    }

    for class in &resolved.classes {
        raw.push(span_token(
            class.name_span,
            TT_CLASS,
            MOD_DEFINITION | MOD_DECLARATION,
        ));

        for (_, span) in &class.decorator_spans {
            raw.push(span_token(*span, TT_DECORATOR, 0));
        }
        for gp in &class.generic_params {
            raw.push(span_token(gp.span, TT_TYPE_PARAMETER, MOD_DEFINITION));
        }
        for attr in &class.attributes {
            raw.push(span_token(attr.name_span, TT_PROPERTY, MOD_DEFINITION));
        }
    }

    for var in &resolved.module_vars {
        raw.push(span_token(var.name_span, TT_VARIABLE, MOD_DEFINITION));
    }

    for imp in &resolved.imports {
        raw.push(span_token(imp.span, TT_NAMESPACE, 0));
    }

    raw.sort_by_key(|t| t.byte_offset);
    delta_encode(&raw, source)
}

/// Delta-encode raw tokens into LSP `SemanticToken` values.
fn delta_encode(raw: &[RawToken], source: &str) -> Vec<SemanticToken> {
    let mut prev_line = 0u32;
    let mut prev_start = 0u32;
    let mut tokens = Vec::with_capacity(raw.len());

    for rt in raw {
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
