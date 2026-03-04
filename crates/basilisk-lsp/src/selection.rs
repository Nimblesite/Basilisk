//! Selection Ranges handler (Smart Select).
//!
//! Implements `textDocument/selectionRange` — given cursor positions, returns
//! nested selection ranges that expand outward from the most specific scope
//! (e.g. parameter name) to the broadest (entire document).

use basilisk_resolver::{ResolvedModule, Span};
use tower_lsp::lsp_types::{Position, Range, SelectionRange};

use crate::util::{byte_offset_to_position, position_to_byte_offset};

/// Compute selection ranges for the given cursor positions.
///
/// Each position maps to a `SelectionRange` linked list: innermost first,
/// with `.parent` pointing to the next larger enclosing range. The outermost
/// range is always the entire document.
#[must_use]
pub fn selection_ranges(
    resolved: &ResolvedModule,
    source: &str,
    positions: &[Position],
) -> Vec<SelectionRange> {
    positions
        .iter()
        .map(|pos| {
            let offset = position_to_byte_offset(source, *pos);
            build_selection_range(resolved, source, offset)
        })
        .collect()
}

/// Build a nested `SelectionRange` chain for a single byte offset.
///
/// Collects all spans (from functions, classes, parameters, variables, attributes)
/// that contain the offset, sorts them from largest to smallest, then chains
/// them as a linked list from innermost to outermost. The outermost entry is
/// always the full document range.
fn build_selection_range(
    resolved: &ResolvedModule,
    source: &str,
    offset: usize,
) -> SelectionRange {
    let mut spans = collect_containing_spans(resolved, offset);

    // Sort by span size descending (largest first) so we build the chain
    // from outermost to innermost.
    spans.sort_by(|a, b| {
        let size_a = a.end.saturating_sub(a.start);
        let size_b = b.end.saturating_sub(b.start);
        size_b.cmp(&size_a)
    });

    // Deduplicate identical spans.
    spans.dedup();

    // Start with the whole-document range as the outermost parent.
    let doc_end = byte_offset_to_position(source, source.len());
    let mut current = SelectionRange {
        range: Range {
            start: Position::new(0, 0),
            end: doc_end,
        },
        parent: None,
    };

    // Wrap each span from outermost to innermost, making the previous
    // range the parent of the new one.
    for span in &spans {
        let range = span_to_range(source, *span);
        // Skip if this range is identical to the current (document) range
        // or any duplicate.
        if range == current.range {
            continue;
        }
        current = SelectionRange {
            range,
            parent: Some(Box::new(current)),
        };
    }

    current
}

/// Collect all spans from the resolved module that contain the given offset.
fn collect_containing_spans(resolved: &ResolvedModule, offset: usize) -> Vec<Span> {
    let mut spans = Vec::new();

    for func in &resolved.functions {
        // Function definition span (the entire `def ... :` block).
        if contains(func.def_span, offset) {
            spans.push(func.def_span);
        }
        // Function name span.
        if contains(func.name_span, offset) {
            spans.push(func.name_span);
        }
        // Parameters: check if cursor is inside a parameter.
        for param in &func.parameters {
            if contains(param.name_span, offset) {
                spans.push(param.name_span);
            }
            if let Some(ann_span) = param.annotation_span {
                if contains(ann_span, offset) {
                    spans.push(ann_span);
                }
            }
        }
        if let Some(ref va) = func.vararg {
            if contains(va.name_span, offset) {
                spans.push(va.name_span);
            }
        }
        if let Some(ref kw) = func.kwarg {
            if contains(kw.name_span, offset) {
                spans.push(kw.name_span);
            }
        }
        // Return annotation span.
        if let Some(ret_span) = func.return_annotation_span {
            if contains(ret_span, offset) {
                spans.push(ret_span);
            }
        }
    }

    for class in &resolved.classes {
        // Class definition span.
        if contains(class.def_span, offset) {
            spans.push(class.def_span);
        }
        // Class name span.
        if contains(class.name_span, offset) {
            spans.push(class.name_span);
        }
        // Attributes.
        for attr in &class.attributes {
            if contains(attr.name_span, offset) {
                spans.push(attr.name_span);
            }
            if let Some(ann_span) = attr.annotation_span {
                if contains(ann_span, offset) {
                    spans.push(ann_span);
                }
            }
        }
    }

    // Module-level variables.
    for var in &resolved.module_vars {
        if contains(var.name_span, offset) {
            spans.push(var.name_span);
        }
        if let Some(ann_span) = var.annotation_span {
            if contains(ann_span, offset) {
                spans.push(ann_span);
            }
        }
    }

    // Imports.
    for imp in &resolved.imports {
        if contains(imp.span, offset) {
            spans.push(imp.span);
        }
    }

    spans
}

/// Check whether a byte offset falls within a span.
fn contains(span: Span, offset: usize) -> bool {
    (span.start as usize) <= offset && offset < (span.end as usize)
}

/// Convert a resolver `Span` to an LSP `Range`.
fn span_to_range(source: &str, span: Span) -> Range {
    Range {
        start: byte_offset_to_position(source, span.start as usize),
        end: byte_offset_to_position(source, span.end as usize),
    }
}
