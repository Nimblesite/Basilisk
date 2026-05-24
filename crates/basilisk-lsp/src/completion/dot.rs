//! Implements [LSPARCH-FEATURES-COMPLETION]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-COMPLETION
//!
//! Dot-completion provider.
//!
//! Handles `expr.<cursor>` and `self.<cursor>` patterns by inspecting class
//! attributes and methods from the resolved module.

use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind};

use super::prefix::{dot_receiver, extract_prefix};

/// Return completion items for a dot-access expression at `byte_offset`.
pub(super) fn dot_completions(
    resolved: &basilisk_resolver::ResolvedModule,
    text: &str,
    byte_offset: usize,
) -> Vec<CompletionItem> {
    let receiver = dot_receiver(text, byte_offset);
    let prefix = extract_prefix(text, byte_offset);

    if receiver.as_deref() == Some("self") {
        enclosing_class(resolved, byte_offset)
            .map(|c| class_member_items(c, &prefix))
            .unwrap_or_default()
    } else if let Some(ref recv_name) = receiver {
        resolved
            .classes
            .iter()
            .find(|c| &c.name == recv_name)
            .map(|c| class_member_items(c, &prefix))
            .unwrap_or_default()
    } else {
        vec![]
    }
}

/// Find the innermost class that encloses `offset`.
fn enclosing_class(
    resolved: &basilisk_resolver::ResolvedModule,
    offset: usize,
) -> Option<&basilisk_resolver::scope::ClassInfo> {
    let func = resolved
        .functions
        .iter()
        .find(|f| f.class_name.is_some() && f.def_span.start_usize() <= offset)?;
    let class_name = func.class_name.as_ref()?;
    resolved.classes.iter().find(|c| &c.name == class_name)
}

/// Build completion items for all attributes and methods of `class`.
fn class_member_items(
    class: &basilisk_resolver::scope::ClassInfo,
    prefix: &str,
) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    for attr in &class.attributes {
        if !prefix.is_empty() && !attr.name.starts_with(prefix) {
            continue;
        }
        items.push(CompletionItem {
            label: attr.name.clone(),
            kind: Some(CompletionItemKind::FIELD),
            detail: Some(format!("{}.{}", class.name, attr.name)),
            ..Default::default()
        });
    }
    for method_name in &class.method_names {
        if !prefix.is_empty() && !method_name.starts_with(prefix) {
            continue;
        }
        items.push(CompletionItem {
            label: method_name.clone(),
            kind: Some(CompletionItemKind::METHOD),
            detail: Some(format!("{}.{}", class.name, method_name)),
            ..Default::default()
        });
    }
    items
}
