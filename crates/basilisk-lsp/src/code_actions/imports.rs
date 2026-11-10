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
