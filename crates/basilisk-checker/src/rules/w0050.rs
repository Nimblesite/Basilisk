//! BSK-W0050: Redundant type annotation warning.
//!
//! Emits a warning when a type annotation is redundant because the inferred type
//! exactly matches the declared type. This is Basilisk's headline differentiator
//! from other type checkers.
//!
//! ```python
//! x: int = 42        # W0050 — annotation is redundant
//! y: str = "hello"   # W0050 — annotation is redundant
//! z: float = 42      # NO warning — annotation adds information (widening)
//! ```

use basilisk_resolver::ResolvedModule;
use crate::inference::infer_rhs;
use crate::types::InferredType;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-W0050",
    docs_url: "https://basilisk-lang.org/errors/BSK-W0050",
};

/// Emits BSK-W0050 for redundant type annotations.
pub(crate) struct RedundantAnnotationWarning;

impl Rule for RedundantAnnotationWarning {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Check module-level variables
        module
            .module_vars
            .iter()
            .filter(|var| var.has_annotation)
            .filter_map(|var| {
                let annotation_text = extract_annotation(&module.source, var.name_span)?;
                
                // Use inference system to get RHS type
                let inferred_type = infer_rhs(&var.rhs_kind);
                
                // Skip if inference failed
                if matches!(inferred_type, InferredType::Unknown) {
                    return None;
                }
                
                // Parse annotation text to InferredType using existing parser
                let declared_type = InferredType::from_annotation(annotation_text);
                
                // Check if annotation is redundant (base type match)
                if types_match_for_w0050(&inferred_type, &declared_type) {
                    Some((var.name_span, var.name.clone(), annotation_text.to_owned()))
                } else {
                    None
                }
            })
            .for_each(|(span, name, annotation)| {
                diagnostics.push(make_diagnostic_for_var(&name, &annotation, span, &module.path));
            });
        
        // Check class attributes (skip TypedDict/Protocol/NamedTuple classes)
        module
            .classes
            .iter()
            .filter(|class| {
                // Skip TypedDict, Protocol, and NamedTuple classes
                !class.bases.iter().any(|base| {
                    base.contains("TypedDict") || base.contains("Protocol") || base.contains("NamedTuple")
                })
            })
            .flat_map(|class| &class.attributes)
            .filter(|attr| attr.has_annotation && attr.has_value)
            .filter_map(|attr| {
                let annotation_text = extract_annotation(&module.source, attr.name_span)?;
                
                // Use inference system to get RHS type
                let inferred_type = infer_rhs(&attr.rhs_kind);
                
                // For class attributes with literal values, we can infer the type from the source
                let inferred_type = if matches!(inferred_type, InferredType::Unknown) {
                    // Try to infer from the source text
                    infer_type_from_source(&module.source, attr.name_span, &InferredType::from_annotation(annotation_text))
                } else {
                    inferred_type
                };
                
                // Skip if inference still failed
                if matches!(inferred_type, InferredType::Unknown) {
                    return None;
                }
                
                // Parse annotation text to InferredType using existing parser
                let declared_type = InferredType::from_annotation(annotation_text);
                
                // Check if annotation is redundant (base type match)
                if types_match_for_w0050(&inferred_type, &declared_type) {
                    Some((attr.name_span, attr.name.clone(), annotation_text.to_owned()))
                } else {
                    None
                }
            })
            .for_each(|(span, name, annotation)| {
                diagnostics.push(make_diagnostic_for_var(&name, &annotation, span, &module.path));
            });
    }
}

/// Extract the annotation text from the source line containing `name_span`.
///
/// Looks for `: <annotation>` on the same source line as the variable name,
/// stopping at the `=` sign that introduces the RHS.  Returns `None` if no
/// such pattern is found.
fn extract_annotation(source: &str, name_span: basilisk_resolver::Span) -> Option<&str> {
    // Find the byte offset of the start of the line containing the name.
    let start = name_span.start as usize;
    let line_start = source[..start].rfind('\n').map_or(0, |pos| pos + 1);
    let line_end = source[start..]
        .find('\n')
        .map_or(source.len(), |pos| start + pos);

    let line = source.get(line_start..line_end)?;

    // Position of the name within the line.
    let name_offset = start.checked_sub(line_start)?;

    // Find `: ` after the name position on this line.
    let colon_pos = line[name_offset..].find(": ")? + name_offset;
    let after_colon = colon_pos + 2; // skip ': '

    // Find `=` that ends the annotation (must be after the colon).
    let annotation_end = line[after_colon..]
        .find('=')
        .map_or(line.len(), |p| after_colon + p);

    let annotation = line.get(after_colon..annotation_end)?.trim();

    if annotation.is_empty() {
        None
    } else {
        Some(annotation)
    }
}

/// Check if types match for W0050 purposes (base type comparison)
fn types_match_for_w0050(inferred: &InferredType, declared: &InferredType) -> bool {
    use InferredType::{Bool, Bytes, Dict, Float, Int, List, Never, None_, Set, Str, Tuple};

    match (inferred, declared) {
        // Basic types - exact match
        (Int, Int) | (Str, Str) | (Float, Float) | (Bool, Bool) | (Bytes, Bytes) | (None_, None_) => true,
        // Empty containers: annotation adds element-type information, so it is NOT redundant
        (List(a), List(_)) if matches!(a.as_ref(), Never) => false,
        (Dict(ak, _), Dict(_, _)) if matches!(ak.as_ref(), Never) => false,
        (Set(a), Set(_)) if matches!(a.as_ref(), Never) => false,
        // Non-empty collection types - outer type match is sufficient for W0050
        (List(_), List(_)) | (Dict(..), Dict(..)) | (Set(_), Set(_)) | (Tuple(_), Tuple(_)) => true,
        // Default case - no match
        _ => false,
    }
}

/// Infer type from source text when resolver inference fails
fn infer_type_from_source(source: &str, name_span: basilisk_resolver::Span, declared_type: &InferredType) -> InferredType {
    // Extract the line containing the assignment
    let start = name_span.start as usize;
    let line_start = source[..start].rfind('\n').map_or(0, |pos| pos + 1);
    let line_end = source[start..]
        .find('\n')
        .map_or(source.len(), |pos| start + pos);

    let Some(line) = source.get(line_start..line_end) else {
        return InferredType::Unknown;
    };

    // Find the value after the '=' sign
    let Some(equals_pos) = line.find('=') else {
        return InferredType::Unknown;
    };

    let value_text = line[equals_pos + 1..].trim();
    
    // Simple literal detection
    if value_text.parse::<i64>().is_ok() {
        InferredType::Int
    } else if value_text.parse::<f64>().is_ok() {
        InferredType::Float
    } else if (value_text.starts_with('"') && value_text.ends_with('"')) || 
              (value_text.starts_with('\'') && value_text.ends_with('\'')) {
        InferredType::Str
    } else if value_text == "True" || value_text == "False" {
        InferredType::Bool
    } else if value_text == "None" {
        InferredType::None_
    } else if value_text.starts_with("b\"") && value_text.ends_with('"') {
        InferredType::Bytes
    } else {
        // Fall back to the declared type if we can't infer from the literal
        declared_type.clone()
    }
}

/// Create diagnostic for redundant annotation warning
fn make_diagnostic_for_var(
    name: &str,
    annotation: &str,
    span: basilisk_resolver::Span,
    path: &str,
) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Warning,
        message: format!(
            "Redundant type annotation: `{name}` is annotated `{annotation}` but the inferred type is identical"
        ),
        span,
        path: path.to_owned(),
        help: Some("Remove the redundant annotation to reduce noise".to_owned()),
        note: Some(
            "Basilisk warns about redundant annotations to encourage cleaner code".to_owned(),
        ),
    }
}
