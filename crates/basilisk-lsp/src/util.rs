//! Implements [LSPARCH-FEATURES-FINDSYM]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-FINDSYM
//! Shared utilities for LSP feature handlers.
//!
//! Position conversion, symbol lookup, and type signature formatting.

use std::fmt::Write as _;

use basilisk_resolver::{
    AttributeInfo, ClassInfo, FunctionInfo, ImportInfo, ImportKind, ParameterInfo, ResolvedModule,
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
    span.start_usize() <= offset && offset < span.end_usize()
}

/// Find the symbol whose `name_span` contains the given byte offset.
///
/// Searches functions (and their parameters and local variables), classes (and
/// their attributes), module variables, and imports. Returns the first match.
// Implements [LSPARCH-FEATURES-FINDSYM] — central symbol lookup reused by hover, goto-def, references, rename.
#[must_use]
pub fn find_symbol_at_offset(resolved: &ResolvedModule, offset: usize) -> Option<SymbolHit<'_>> {
    // Check function names, their parameters, and their local variables.
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
        // Function-local bindings: annotated (`x: T = ...`) and plain (`x = ...`).
        for var in func.local_vars.iter().chain(&func.local_unannotated_vars) {
            if in_span(offset, var.name_span) {
                return Some(SymbolHit::Variable(var));
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

/// Find a symbol definition by name. Searches functions, classes, module
/// variables, function-local variables, and function parameters.
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
    // Function-local variables (annotated and plain assignments) and parameters.
    // Searched last so module-level symbols keep precedence for a bare name
    // reference; lets a use of a local/param resolve to its in-function binding.
    for func in &resolved.functions {
        for var in func.local_vars.iter().chain(&func.local_unannotated_vars) {
            if var.name == name {
                return Some(SymbolHit::Variable(var));
            }
        }
        if let Some(hit) = find_parameter_by_name(func, name) {
            return Some(hit);
        }
    }
    // Class attributes, so a `self.attr` (or `obj.attr`) read resolves to the
    // attribute's declaration. Searched last — only matches a name that nothing
    // higher-precedence claimed — so existing resolutions are unchanged.
    for class in &resolved.classes {
        for attr in &class.attributes {
            if attr.name == name {
                return Some(SymbolHit::Attribute { class, attr });
            }
        }
    }
    None
}

/// Find a parameter (positional, `*args`, or `**kwargs`) of `func` by name.
fn find_parameter_by_name<'a>(func: &'a FunctionInfo, name: &str) -> Option<SymbolHit<'a>> {
    for param in &func.parameters {
        if param.name == name {
            return Some(SymbolHit::Parameter { func, param });
        }
    }
    if let Some(ref va) = func.vararg {
        if va.name == name {
            return Some(SymbolHit::Parameter { func, param: va });
        }
    }
    if let Some(ref kw) = func.kwarg {
        if kw.name == name {
            return Some(SymbolHit::Parameter { func, param: kw });
        }
    }
    None
}

/// Find the import that introduces `name` as a locally-bound symbol.
///
/// Matches `from m import name` / `from m import x as name` (the bound name lives
/// in `names`) and `import name` / `import m as name` (the module or its alias).
/// Star imports bind no specific name, so they never match. Shared by hover
/// (render the import declaration) and goto-definition (resolve the import's
/// target file) when the cross-module symbol table has not populated yet.
pub(crate) fn find_import_by_bound_name<'a>(
    resolved: &'a ResolvedModule,
    name: &str,
) -> Option<&'a ImportInfo> {
    resolved.imports.iter().find(|imp| match imp.kind {
        ImportKind::Star => false,
        ImportKind::From => imp.names.iter().any(|n| n == name),
        ImportKind::Plain => imp.names.iter().any(|n| n == name) || imp.module == name,
    })
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
    let current_byte = *bytes.get(offset)?;
    if !is_ident_char(current_byte) {
        return None;
    }
    let mut start = offset;
    while start > 0 && bytes.get(start - 1).copied().is_some_and(is_ident_char) {
        start -= 1;
    }
    let mut end = offset;
    while bytes.get(end).copied().is_some_and(is_ident_char) {
        end += 1;
    }
    source.get(start..end).map(str::to_owned)
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

// ── Type signature formatting ────────────────────────────────────────────────

/// Format a hover markdown string for a symbol hit.
// Implements [LSPARCH-FEATURES-FINDSYM] — `format_type_signature` builds hover markdown for any symbol kind.
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
        } else if !is_implicit_receiver(func, idx, param) {
            // #253: an unannotated parameter renders its inferred type —
            // `Unknown` until parameter inference exists — never blank.
            sig.push_str(": Unknown");
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
        ReturnAnnotationKind::Missing => {
            // #253: no annotation — infer from the body's `return` statements.
            let inferred = infer_return_type_display(func);
            if !inferred.is_empty() {
                let _ = write!(sig, " -> {inferred}");
            }
        }
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

/// `true` for the implicit `self`/`cls` receiver of a method — it carries an
/// implicit type, so it never renders an `Unknown` annotation.
fn is_implicit_receiver(func: &FunctionInfo, idx: usize, param: &ParameterInfo) -> bool {
    idx == 0 && func.class_name.is_some() && (param.name == "self" || param.name == "cls")
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
        let inferred = rhs_type_display(&var.rhs_kind);
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
        let inferred = rhs_type_display(&attr.rhs_kind);
        if !inferred.is_empty() {
            let _ = write!(sig, ": {inferred}");
        }
    }
    sig
}

fn format_import_signature(imp: &ImportInfo) -> String {
    // Route on `kind`, not `names.is_empty()`: a plain `import X as Y` carries its
    // alias `Y` in `names`, so a names-based check would mislabel it a `from`-import.
    match imp.kind {
        ImportKind::From => {
            format!(
                "(import) from {} import {}",
                imp.module,
                imp.names.join(", ")
            )
        }
        ImportKind::Star => format!("(module) from {} import *", imp.module),
        ImportKind::Plain => match imp.names.first() {
            Some(alias) => format!("(module) import {} as {alias}", imp.module),
            None => format!("(module) import {}", imp.module),
        },
    }
}

/// Extract annotation text from the source using a span.
fn annotation_text(span: Option<Span>, source: &str) -> Option<String> {
    let span = span?;
    let text = span.slice_source(source)?;
    Some(text.trim().to_owned())
}

/// Simple type-name display for an inferred `RhsKind` (shared by inlay hints).
pub(crate) fn rhs_type_display(rhs: &basilisk_resolver::RhsKind) -> &'static str {
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
        RhsKind::KnownCall(result) => rhs_type_display(result),
        _ => "",
    }
}

/// Infer a display type for a function's return from its `return` statements.
///
/// Shared by hover (#253) and inlay hints. Returns an empty string when the
/// type cannot be determined.
pub(crate) fn infer_return_type_display(func: &basilisk_resolver::FunctionInfo) -> &'static str {
    if func.return_stmts.is_empty() {
        return "None";
    }

    // Collect the display names for every return statement.
    let mut common_type: Option<&'static str> = None;
    for ret in &func.return_stmts {
        let display = rhs_type_display(&ret.rhs_kind);
        // If any return has an uninferrable type, bail out.
        if display.is_empty() {
            return "";
        }
        match common_type {
            None => common_type = Some(display),
            Some(prev) if prev == display => {}
            Some(_) => return "", // mixed return types — cannot infer
        }
    }

    common_type.unwrap_or("None")
}

// ── Position conversion ──────────────────────────────────────────────────────

/// Convert a byte offset to an LSP position (UTF-16 code units).
#[must_use]
pub fn byte_offset_to_position(text: &str, byte_offset: usize) -> Position {
    let clamped = byte_offset.min(text.len());
    let before = text.get(..clamped).unwrap_or(text);
    let line = u32::try_from(before.chars().filter(|&c| c == '\n').count()).unwrap_or(u32::MAX);
    let last_nl = before.rfind('\n').map_or(0, |p| p + 1);
    let after_nl = before.get(last_nl..).unwrap_or("");
    let character = after_nl
        .chars()
        .map(|c| if u32::from(c) > 0xFFFF { 2u32 } else { 1u32 })
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
            char_cu += if u32::from(ch) > 0xFFFF { 2 } else { 1 };
        }
        byte_pos = byte_idx + ch.len_utf8();
    }
    byte_pos
}

/// Convert a `Span` to an LSP `Range`.
#[must_use]
pub fn span_to_range(text: &str, span: Span) -> tower_lsp::lsp_types::Range {
    tower_lsp::lsp_types::Range {
        start: byte_offset_to_position(text, span.start_usize()),
        end: byte_offset_to_position(text, span.end_usize()),
    }
}

/// Get the symbol name at a byte offset, either from the symbol table or from
/// the identifier under the cursor.
pub(crate) fn symbol_name_at(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
) -> Option<String> {
    if let Some(hit) = find_symbol_at_offset(resolved, byte_offset) {
        return Some(symbol_hit_name(&hit).to_owned());
    }
    identifier_at_offset(source, byte_offset)
}

/// The declared name of a resolved symbol hit.
fn symbol_hit_name<'a>(hit: &'a SymbolHit<'a>) -> &'a str {
    match hit {
        SymbolHit::Function(f) => &f.name,
        SymbolHit::Class(c) => &c.name,
        SymbolHit::Variable(v) => &v.name,
        SymbolHit::Parameter { param, .. } => &param.name,
        SymbolHit::Attribute { attr, .. } => &attr.name,
        SymbolHit::Import(i) => &i.module,
    }
}

/// The source span of a resolved symbol hit's defining name.
pub(crate) fn definition_span(hit: &SymbolHit<'_>) -> Span {
    match hit {
        SymbolHit::Function(f) => f.name_span,
        SymbolHit::Class(c) => c.name_span,
        SymbolHit::Variable(v) => v.name_span,
        SymbolHit::Parameter { param, .. } => param.name_span,
        SymbolHit::Attribute { attr, .. } => attr.name_span,
        SymbolHit::Import(i) => i.span,
    }
}

/// The LSP range covering a resolved symbol hit's defining name.
pub(crate) fn definition_range(hit: &SymbolHit<'_>, source: &str) -> tower_lsp::lsp_types::Range {
    span_to_range(source, definition_span(hit))
}
