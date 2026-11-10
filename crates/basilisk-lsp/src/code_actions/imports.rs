//! Implements [LSPARCH-FEATURES-CODEACTIONS] and [LSPFMT-IMPORTS].
//! See docs/specs/LSP-FORMATTING-SPEC.md#LSPFMT-IMPORTS
//!
//! Import-related code actions.
//!
//! Provides: organize imports, expand wildcard imports, split multiple
//! imports, and add `__all__` declaration. The first three run **natively in
//! the binary** on the Ruff AST ([`crate::import_hygiene`]) — no `ruff`
//! subprocess, no PATH lookup, no silent no-op when ruff is absent (#261).

use std::collections::HashMap;

use tower_lsp::lsp_types::{CodeAction, CodeActionKind, Position, Range, TextEdit, Url};

use super::full_document_range;

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Build a [`CodeAction`] that replaces the entire document with `new_text`.
fn full_file_replacement_action(
    uri: &Url,
    source: &str,
    new_text: String,
    title: &str,
    kind: CodeActionKind,
    is_preferred: bool,
) -> CodeAction {
    let mut changes = HashMap::new();
    let _ = changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: full_document_range(source),
            new_text,
        }],
    );
    super::code_action_with_changes(title.to_owned(), kind, changes, is_preferred)
}

// ── Organize imports (native, isort semantics) ────────────────────────────────

/// Sort the document's leading import block with isort semantics and return a
/// full-file replacement [`CodeAction`], or `None` if the source is already
/// organized. Runs in-process on the Ruff AST ([LSPFMT-IMPORTS]).
pub(crate) fn organize_imports(uri: &Url, source: &str) -> Option<CodeAction> {
    let new_source = crate::import_hygiene::organize_source(source, None)?;
    Some(full_file_replacement_action(
        uri,
        source,
        new_source,
        "Organize imports",
        CodeActionKind::SOURCE_ORGANIZE_IMPORTS,
        true,
    ))
}

// ── Expand wildcard imports (native) ──────────────────────────────────────────

/// Replace `from X import *` with explicit imports of the names the file
/// uses, or `None` if there is no unambiguous wildcard to expand
/// ([LSPFMT-IMPORTS]).
pub(crate) fn expand_wildcard_imports(uri: &Url, source: &str) -> Option<CodeAction> {
    if !source.contains("import *") {
        return None;
    }
    let new_source = crate::import_hygiene::expand_wildcard_source(source)?;
    Some(full_file_replacement_action(
        uri,
        source,
        new_source,
        "Expand wildcard imports",
        CodeActionKind::QUICKFIX,
        false,
    ))
}

// ── Split multiple imports on one line (native) ───────────────────────────────

/// Split `import a, b` statements into one import per module (Ruff E401 fix
/// parity), or `None` if no statement needs splitting ([LSPFMT-IMPORTS]).
pub(crate) fn convert_import_style(uri: &Url, source: &str) -> Option<CodeAction> {
    if !source.contains("import ") {
        return None;
    }
    let new_source = crate::import_hygiene::split_multi_imports(source)?;
    Some(full_file_replacement_action(
        uri,
        source,
        new_source,
        "Split multiple imports on one line",
        CodeActionKind::QUICKFIX,
        false,
    ))
}

// ── Add __all__ declaration ───────────────────────────────────────────────────

/// Offer to add an `__all__` declaration listing all public names in the module.
/// Only offered when `__all__` is not already defined.
pub(crate) fn add_dunder_all(uri: &Url, source: &str) -> Option<CodeAction> {
    if source.contains("__all__") {
        return None;
    }

    let public_names = collect_public_names(source);
    if public_names.is_empty() {
        return None;
    }

    let names_str = public_names
        .iter()
        .map(|n| format!("    \"{n}\","))
        .collect::<Vec<_>>()
        .join("\n");
    let all_text = format!("__all__ = [\n{names_str}\n]\n\n");

    let insert_line = super::last_import_line(source);
    let insert_pos = Position {
        line: insert_line,
        character: 0,
    };

    let mut changes = HashMap::new();
    let _ = changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: Range {
                start: insert_pos,
                end: insert_pos,
            },
            new_text: all_text,
        }],
    );
    Some(super::code_action_with_changes(
        "Add __all__ declaration (basilisk)".to_owned(),
        CodeActionKind::SOURCE,
        changes,
        false,
    ))
}

/// Collect public (non-underscore-prefixed) top-level names from source text.
fn collect_public_names(source: &str) -> Vec<&str> {
    let mut public_names: Vec<&str> = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("def ") {
            if let Some(name) = rest.split('(').next() {
                let name = name.trim();
                if !name.starts_with('_') {
                    public_names.push(name);
                }
            }
        } else if let Some(rest) = trimmed.strip_prefix("class ") {
            if let Some(name) = rest.split(['(', ':']).next() {
                let name = name.trim();
                if !name.starts_with('_') {
                    public_names.push(name);
                }
            }
        } else if !trimmed.starts_with('#')
            && !trimmed.starts_with("import ")
            && !trimmed.starts_with("from ")
            && !trimmed.is_empty()
        {
            if let Some(name) = trimmed.split('=').next() {
                let name = name.split(':').next().unwrap_or("").trim();
                if !name.is_empty()
                    && !name.starts_with('_')
                    && name.chars().all(|c| c.is_alphanumeric() || c == '_')
                {
                    public_names.push(name);
                }
            }
        }
    }
    public_names
}
