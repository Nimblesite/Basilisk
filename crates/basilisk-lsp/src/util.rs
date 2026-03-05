//! Shared utilities for LSP feature handlers.
//!
//! Position conversion, symbol lookup, and type signature formatting.

use std::fmt::Write as _;

use basilisk_resolver::{
    AttributeInfo, ClassInfo, FunctionInfo, ImportInfo, ParameterInfo, ResolvedModule,
    ReturnAnnotationKind, Span, VariableInfo,
};
use tower_lsp::lsp_types::Position;

// ── Symbol lookup ────────────────────────────────────────────────────────────

/// A symbol found at a byte offset in the source.
#[derive(Debug)]
pub enum SymbolHit<'a> {
    /// A function or method definition.
    Function(&'a FunctionInfo),
    /// A class definition.
    Class(&'a ClassInfo),
    /// A module-level variable.
    Variable(&'a VariableInfo),
    /// A parameter within a function.
    Parameter {
        /// The enclosing function.
        func: &'a FunctionInfo,
        /// The parameter itself.
        param: &'a ParameterInfo,
    },
    /// A class attribute.
    Attribute {
        /// The enclosing class.
        class: &'a ClassInfo,
        /// The attribute itself.
        attr: &'a AttributeInfo,
    },
    /// An import statement.
    Import(&'a ImportInfo),
}

/// Check if a byte offset falls within a span.
fn in_span(offset: usize, span: Span) -> bool {
    (span.start as usize) <= offset && offset < (span.end as usize)
}

/// Find the symbol whose `name_span` contains the given byte offset.
///
/// Searches functions (and their parameters), classes (and their attributes),
/// module variables, and imports. Returns the first match.
#[must_use]
pub fn find_symbol_at_offset(resolved: &ResolvedModule, offset: usize) -> Option<SymbolHit<'_>> {
    // Check function names and their parameters.
    for func in &resolved.functions {
        if in_span(offset, func.name_span) {
            return Some(SymbolHit::Function(func));
        }
        for param in &func.parameters {
            if in_span(offset, param.name_span) {
                return Some(SymbolHit::Parameter { func, param });
            }
        }
        if let Some(ref va) = func.vararg {
            if in_span(offset, va.name_span) {
                return Some(SymbolHit::Parameter { func, param: va });
            }
        }
        if let Some(ref kw) = func.kwarg {
            if in_span(offset, kw.name_span) {
                return Some(SymbolHit::Parameter { func, param: kw });
            }
        }
    }

    // Check class names and their attributes.
    for class in &resolved.classes {
        if in_span(offset, class.name_span) {
            return Some(SymbolHit::Class(class));
        }
        for attr in &class.attributes {
            if in_span(offset, attr.name_span) {
                return Some(SymbolHit::Attribute { class, attr });
            }
        }
    }

    // Module-level variables.
    for var in &resolved.module_vars {
        if in_span(offset, var.name_span) {
            return Some(SymbolHit::Variable(var));
        }
    }

    // Imports.
    for imp in &resolved.imports {
        if in_span(offset, imp.span) {
            return Some(SymbolHit::Import(imp));
        }
    }

    None
}

/// Find a symbol definition by name. Searches functions, classes, variables.
///
/// Returns the definition `SymbolHit` for the first match.
#[must_use]
pub fn find_definition_by_name<'a>(
    resolved: &'a ResolvedModule,
    name: &str,
) -> Option<SymbolHit<'a>> {
    // Functions (including methods).
    for func in &resolved.functions {
        if func.name == name {
            return Some(SymbolHit::Function(func));
        }
    }
    // Classes.
    for class in &resolved.classes {
        if class.name == name {
            return Some(SymbolHit::Class(class));
        }
    }
    // Module variables.
    for var in &resolved.module_vars {
        if var.name == name {
            return Some(SymbolHit::Variable(var));
        }
    }
    None
}

/// Extract the identifier at a byte offset from source text.
///
/// Expands left and right from the offset to capture the full identifier.
#[must_use]
pub fn identifier_at_offset(source: &str, offset: usize) -> Option<String> {
    let bytes = source.as_bytes();
    if offset >= bytes.len() {
        return None;
    }
    // Must be on an identifier character.
    if !is_ident_char(bytes[offset]) {
        return None;
    }
    let mut start = offset;
    while start > 0 && is_ident_char(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = offset;
    while end < bytes.len() && is_ident_char(bytes[end]) {
        end += 1;
    }
    Some(source[start..end].to_owned())
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

// ── Type signature formatting ────────────────────────────────────────────────

/// Format a hover markdown string for a symbol hit.
#[must_use]
pub fn format_type_signature(hit: &SymbolHit<'_>, source: &str) -> String {
    match hit {
        SymbolHit::Function(func) => format_function_signature(func, source),
        SymbolHit::Class(class) => format_class_signature(class),
        SymbolHit::Variable(var) => format_variable_signature(var, source),
        SymbolHit::Parameter { param, .. } => format_parameter_signature(param, source),
        SymbolHit::Attribute { class, attr } => format_attribute_signature(class, attr, source),
        SymbolHit::Import(imp) => format_import_signature(imp),
    }
}

fn format_function_signature(func: &FunctionInfo, source: &str) -> String {
    let kind = if func.class_name.is_some() {
        "method"
    } else {
        "function"
    };
    let mut sig = String::new();
    let _ = write!(sig, "({kind}) def ");
    if let Some(ref class_name) = func.class_name {
        let _ = write!(sig, "{class_name}.");
    }
    let _ = write!(sig, "{}(", func.name);

    for (idx, param) in func.parameters.iter().enumerate() {
        if idx > 0 {
            sig.push_str(", ");
        }
        sig.push_str(&param.name);
        if let Some(ann) = annotation_text(param.annotation_span, source) {
            let _ = write!(sig, ": {ann}");
        }
    }
    if let Some(ref va) = func.vararg {
        if !func.parameters.is_empty() {
            sig.push_str(", ");
        }
        let _ = write!(sig, "*{}", va.name);
        if let Some(ann) = annotation_text(va.annotation_span, source) {
            let _ = write!(sig, ": {ann}");
        }
    }
    if let Some(ref kw) = func.kwarg {
        if !func.parameters.is_empty() || func.vararg.is_some() {
            sig.push_str(", ");
        }
        let _ = write!(sig, "**{}", kw.name);
        if let Some(ann) = annotation_text(kw.annotation_span, source) {
            let _ = write!(sig, ": {ann}");
        }
    }
    sig.push(')');

    match func.return_annotation {
        ReturnAnnotationKind::Missing => {}
        ReturnAnnotationKind::NoneType => sig.push_str(" -> None"),
        ReturnAnnotationKind::Any => sig.push_str(" -> Any"),
        _ => {
            if let Some(ann) = annotation_text(func.return_annotation_span, source) {
                let _ = write!(sig, " -> {ann}");
            }
        }
    }

    sig
}

fn format_class_signature(class: &ClassInfo) -> String {
    let mut sig = format!("(class) {}", class.name);
    if !class.bases.is_empty() {
        let _ = write!(sig, "({})", class.bases.join(", "));
    }
    sig
}

fn format_variable_signature(var: &VariableInfo, source: &str) -> String {
    let mut sig = format!("(variable) {}", var.name);
    if let Some(ann) = annotation_text(var.annotation_span, source) {
        let _ = write!(sig, ": {ann}");
    } else {
        let inferred = infer_rhs_display(&var.rhs_kind);
        if !inferred.is_empty() {
            let _ = write!(sig, ": {inferred}");
        }
    }
    sig
}

fn format_parameter_signature(param: &ParameterInfo, source: &str) -> String {
    let mut sig = format!("(parameter) {}", param.name);
    if let Some(ann) = annotation_text(param.annotation_span, source) {
        let _ = write!(sig, ": {ann}");
    }
    sig
}

fn format_attribute_signature(class: &ClassInfo, attr: &AttributeInfo, source: &str) -> String {
    let mut sig = format!("(property) {}.{}", class.name, attr.name);
    if let Some(ann) = annotation_text(attr.annotation_span, source) {
        let _ = write!(sig, ": {ann}");
    } else {
        let inferred = infer_rhs_display(&attr.rhs_kind);
        if !inferred.is_empty() {
            let _ = write!(sig, ": {inferred}");
        }
    }
    sig
}

fn format_import_signature(imp: &ImportInfo) -> String {
    if imp.names.is_empty() {
        format!("(module) import {}", imp.module)
    } else {
        format!(
            "(import) from {} import {}",
            imp.module,
            imp.names.join(", ")
        )
    }
}

/// Extract annotation text from the source using a span.
fn annotation_text(span: Option<Span>, source: &str) -> Option<String> {
    let span = span?;
    let text = source.get(span.start as usize..span.end as usize)?;
    Some(text.trim().to_owned())
}

/// Simple display for inferred `RhsKind` types.
fn infer_rhs_display(rhs: &basilisk_resolver::RhsKind) -> &'static str {
    use basilisk_resolver::RhsKind;
    match rhs {
        RhsKind::IntLiteral => "int",
        RhsKind::FloatLiteral => "float",
        RhsKind::StrLiteral => "str",
        RhsKind::BoolLiteral => "bool",
        RhsKind::BytesLiteral => "bytes",
        RhsKind::NoneValue => "None",
        RhsKind::EmptyList | RhsKind::List(_) => "list",
        RhsKind::EmptyDict | RhsKind::Dict(_) => "dict",
        RhsKind::Set(_) => "set",
        RhsKind::Tuple(_) => "tuple",
        _ => "",
    }
}

// ── Position conversion ──────────────────────────────────────────────────────

/// Convert a byte offset to an LSP position (UTF-16 code units).
#[must_use]
pub fn byte_offset_to_position(text: &str, byte_offset: usize) -> Position {
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
#[must_use]
pub fn position_to_byte_offset(text: &str, pos: Position) -> usize {
    let mut line = 0u32;
    let mut char_cu = 0u32;
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

/// Convert a `Span` to an LSP `Range`.
#[must_use]
pub fn span_to_range(text: &str, span: Span) -> tower_lsp::lsp_types::Range {
    tower_lsp::lsp_types::Range {
        start: byte_offset_to_position(text, span.start as usize),
        end: byte_offset_to_position(text, span.end as usize),
    }
}
