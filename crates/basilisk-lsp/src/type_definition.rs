//! Go to Type Definition handler.
//!
//! Navigates from a symbol to the definition of its *type*:
//! - Variable with annotation `x: MyClass` → jumps to `class MyClass`
//! - Parameter with annotation `param: Foo` → jumps to `class Foo`
//! - Attribute with annotation `attr: Bar` → jumps to `class Bar`
//!
//! Falls back to `goto_definition` when no type annotation is present
//! or the annotated type is not a user-defined class in the same file.

use basilisk_resolver::ResolvedModule;
use tower_lsp::lsp_types::{GotoDefinitionResponse, Location, Url};

use crate::util::{
    find_definition_by_name, find_symbol_at_offset, identifier_at_offset, span_to_range, SymbolHit,
};

/// Compute go-to-type-definition for a byte offset.
///
/// Resolves the type annotation of the symbol under the cursor and
/// jumps to the class definition if it exists in the same file.
#[must_use]
pub fn goto_type_definition(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
    uri: &Url,
) -> Option<GotoDefinitionResponse> {
    let hit = find_symbol_at_offset(resolved, byte_offset).or_else(|| {
        let name = identifier_at_offset(source, byte_offset)?;
        find_definition_by_name(resolved, &name)
    })?;

    let annotation_span = match &hit {
        SymbolHit::Variable(v) => v.annotation_span,
        SymbolHit::Parameter { param, .. } => param.annotation_span,
        SymbolHit::Attribute { attr, .. } => attr.annotation_span,
        // For functions, classes, imports — type definition is the symbol itself.
        _ => return None,
    };

    let span = annotation_span?;
    let type_text = source.get(span.start as usize..span.end as usize)?;
    let type_name = extract_base_type(type_text.trim());

    // Find the class definition for this type name.
    let class = resolved.classes.iter().find(|c| c.name == type_name)?;
    let range = span_to_range(source, class.name_span);

    Some(GotoDefinitionResponse::Scalar(Location {
        uri: uri.clone(),
        range,
    }))
}

/// Extract the base type name from an annotation string.
///
/// Strips generic parameters, `Optional[...]`, `list[...]`, etc.
/// - `MyClass` → `MyClass`
/// - `Optional[MyClass]` → `MyClass`
/// - `list[MyClass]` → `MyClass`
/// - `MyClass | None` → `MyClass`
fn extract_base_type(annotation: &str) -> &str {
    // Handle `Optional[X]` or `X | None` patterns.
    let trimmed = annotation.trim();

    // Strip `Optional[...]` wrapper.
    if let Some(inner) = trimmed
        .strip_prefix("Optional[")
        .and_then(|s| s.strip_suffix(']'))
    {
        return extract_base_type(inner.trim());
    }

    // Handle union `X | None` — take the non-None part.
    if trimmed.contains('|') {
        for part in trimmed.split('|') {
            let part = part.trim();
            if part != "None" {
                return extract_base_type(part);
            }
        }
    }

    // Strip generic parameters `X[...]` → `X`.
    if let Some(bracket_pos) = trimmed.find('[') {
        return trimmed[..bracket_pos].trim();
    }

    trimmed
}
