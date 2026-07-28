//! Implements [LSPARCH-FEATURES-CODELENS]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-CODELENS
//!
//! Code Lens handler: reference counts above functions and classes.

use basilisk_resolver::ResolvedModule;
use tower_lsp::lsp_types::{CodeLens, Command};

use crate::references::find_identifier_occurrences;
use crate::util::span_to_range;

/// Format the reference count title for a code lens.
fn reference_title(count: usize) -> String {
    if count == 1 {
        "1 reference".to_owned()
    } else {
        format!("{count} references")
    }
}

/// Compute code lenses for a resolved module.
///
/// Produces one lens per top-level function (excluding methods) and one per
/// class, each showing a reference count.
#[must_use]
pub fn code_lenses(resolved: &ResolvedModule, source: &str) -> Vec<CodeLens> {
    let mut lenses = Vec::new();
    let mask = crate::source_mask::SourceMask::build(source);

    // Top-level functions (not methods).
    for func in &resolved.functions {
        if func.class_name.is_some() {
            continue;
        }
        let refs = find_identifier_occurrences(source, &func.name, &mask);
        // Subtract the definition itself.
        let count = refs.len().saturating_sub(1);
        let range = span_to_range(source, func.name_span);
        lenses.push(CodeLens {
            range,
            command: Some(Command {
                title: reference_title(count),
                command: String::new(),
                arguments: None,
            }),
            data: None,
        });
    }

    // Classes.
    for class in &resolved.classes {
        let refs = find_identifier_occurrences(source, &class.name, &mask);
        let count = refs.len().saturating_sub(1);
        let range = span_to_range(source, class.name_span);
        lenses.push(CodeLens {
            range,
            command: Some(Command {
                title: reference_title(count),
                command: String::new(),
                arguments: None,
            }),
            data: None,
        });
    }

    lenses
}
