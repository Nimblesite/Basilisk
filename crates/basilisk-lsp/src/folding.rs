//! Folding Ranges handler.
//!
//! Computes foldable regions for functions, classes, and import blocks.

use basilisk_resolver::ResolvedModule;
use tower_lsp::lsp_types::{FoldingRange, FoldingRangeKind};

use crate::util::byte_offset_to_position;

/// Compute folding ranges for a resolved module.
///
/// Returns foldable regions for:
/// - Multi-line function definitions
/// - Multi-line class definitions
/// - Consecutive import blocks (2+ imports on adjacent lines)
#[must_use]
pub fn folding_ranges(resolved: &ResolvedModule, source: &str) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();

    // Function definitions
    for func in &resolved.functions {
        let start = byte_offset_to_position(source, func.def_span.start_usize());
        let end = byte_offset_to_position(source, func.def_span.end_usize());
        if start.line < end.line {
            ranges.push(FoldingRange {
                start_line: start.line,
                start_character: Some(start.character),
                end_line: end.line,
                end_character: Some(end.character),
                kind: Some(FoldingRangeKind::Region),
                collapsed_text: Some(format!("def {}(...)", func.name)),
            });
        }
    }

    // Class definitions
    for class in &resolved.classes {
        let start = byte_offset_to_position(source, class.def_span.start_usize());
        let end = byte_offset_to_position(source, class.def_span.end_usize());
        if start.line < end.line {
            ranges.push(FoldingRange {
                start_line: start.line,
                start_character: Some(start.character),
                end_line: end.line,
                end_character: Some(end.character),
                kind: Some(FoldingRangeKind::Region),
                collapsed_text: Some(format!("class {}(...)", class.name)),
            });
        }
    }

    // Import blocks -- group consecutive import lines
    add_import_folding(resolved, source, &mut ranges);

    ranges
}

/// Group consecutive import statements into foldable blocks.
///
/// Imports are sorted by start offset and grouped when the gap between
/// consecutive imports is at most one line. Only groups of 2+ imports
/// produce a folding range.
fn add_import_folding(resolved: &ResolvedModule, source: &str, ranges: &mut Vec<FoldingRange>) {
    if resolved.imports.is_empty() {
        return;
    }

    // Collect (start_line, end_line, start_offset, end_offset) for each import
    let mut import_lines: Vec<(u32, u32, u32, u32)> = resolved
        .imports
        .iter()
        .map(|imp| {
            let start = byte_offset_to_position(source, imp.span.start_usize());
            let end = byte_offset_to_position(source, imp.span.end_usize());
            (start.line, end.line, imp.span.start, imp.span.end)
        })
        .collect();

    // Sort by start offset
    import_lines.sort_by_key(|entry| entry.2);

    let Some(first) = import_lines.first() else {
        return;
    };

    // Group consecutive imports (gap <= 1 line)
    let mut group_start_line = first.0;
    let mut group_end_line = first.1;
    let mut group_count = 1usize;

    for entry in import_lines.get(1..).unwrap_or_default() {
        let (start_line, end_line, _, _) = *entry;
        // If this import starts within 1 line of the previous group end, extend
        if start_line <= group_end_line + 2 {
            if end_line > group_end_line {
                group_end_line = end_line;
            }
            group_count += 1;
        } else {
            // Emit previous group if it has 2+ imports
            if group_count >= 2 && group_start_line < group_end_line {
                ranges.push(FoldingRange {
                    start_line: group_start_line,
                    start_character: Some(0),
                    end_line: group_end_line,
                    end_character: None,
                    kind: Some(FoldingRangeKind::Imports),
                    collapsed_text: Some("imports ...".to_owned()),
                });
            }
            // Start new group
            group_start_line = start_line;
            group_end_line = end_line;
            group_count = 1;
        }
    }

    // Emit last group
    if group_count >= 2 && group_start_line < group_end_line {
        ranges.push(FoldingRange {
            start_line: group_start_line,
            start_character: Some(0),
            end_line: group_end_line,
            end_character: None,
            kind: Some(FoldingRangeKind::Imports),
            collapsed_text: Some("imports ...".to_owned()),
        });
    }
}
