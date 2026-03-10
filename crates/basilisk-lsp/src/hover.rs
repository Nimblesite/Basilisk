//! Hover handler: type-aware hover with diagnostic info.
//!
//! Shows type signatures for symbols at definition sites, reference sites
//! (call sites, variable uses), and dot-access sites (`self.attr`).

use std::fmt::Write as _;

use basilisk_resolver::ResolvedModule;
use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind};

use crate::util::{
    find_definition_by_name, find_symbol_at_offset, format_type_signature, identifier_at_offset,
    SymbolHit,
};

/// Compute hover information at a byte offset.
///
/// Searches definition sites first, then tries name-based lookup for
/// reference sites (call sites, variable uses). Also shows any diagnostics
/// covering the cursor position.
#[must_use]
pub fn hover_at(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
    diagnostics: &[basilisk_checker::Diagnostic],
) -> Option<Hover> {
    let mut sections: Vec<String> = Vec::new();

    // 1. Definition site: cursor directly on a symbol's name_span.
    let hit = find_symbol_at_offset(resolved, byte_offset);

    // 2. Reference site: cursor on an identifier, look up by name.
    let hit = hit.or_else(|| {
        let name = identifier_at_offset(source, byte_offset)?;
        find_definition_by_name(resolved, &name)
    });

    if let Some(ref hit) = hit {
        let sig = format_type_signature(hit, source);
        sections.push(format!("```python\n{sig}\n```"));

        // Show docstring if available.
        let docstring = match hit {
            SymbolHit::Function(f) => f.docstring.as_deref(),
            SymbolHit::Class(c) => c.docstring.as_deref(),
            _ => None,
        };
        if let Some(ds) = docstring {
            sections.push(ds.to_owned());
        }
    }

    // Diagnostic info at this position.
    for d in diagnostics {
        if (d.span.start as usize) <= byte_offset && byte_offset < (d.span.end as usize) {
            let mut diag_md = format!("**{}** — {}", d.code.code, d.message);
            if let Some(ref help) = d.help {
                let _ = write!(diag_md, "\n\n_{help}_");
            }
            sections.push(diag_md);
        }
    }

    if sections.is_empty() {
        return None;
    }

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: sections.join("\n\n---\n\n"),
        }),
        range: None,
    })
}
