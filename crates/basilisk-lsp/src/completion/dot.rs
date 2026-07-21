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

    if let Some((type_name, literal_receiver)) =
        crate::hover::members::dot_receiver_builtin_type(resolved, text, byte_offset)
    {
        if let Some(class) = resolved.builtin_classes.get(&type_name) {
            return builtin_class_member_items(class, &prefix, literal_receiver);
        }
    }

    if receiver.as_deref() == Some("self") {
        crate::hover::access::enclosing_class(resolved, byte_offset)
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

fn builtin_class_member_items(
    class: &basilisk_resolver::scope::IndexedStubClass,
    prefix: &str,
    literal_receiver: bool,
) -> Vec<CompletionItem> {
    let mut seen = std::collections::HashSet::new();
    class
        .declaration
        .methods
        .iter()
        .filter(|method| prefix.is_empty() || method.name.starts_with(prefix))
        .filter(|method| {
            literal_receiver
                || method
                    .receiver
                    .as_ref()
                    .and_then(|receiver| receiver.annotation.as_deref())
                    .is_none_or(|annotation| !annotation.contains("LiteralString"))
        })
        .filter(|method| seen.insert(method.name.clone()))
        .map(|method| CompletionItem {
            label: method.name.clone(),
            kind: Some(CompletionItemKind::METHOD),
            detail: Some(basilisk_stubs::render_stub_signature(method)),
            data: Some(serde_json::json!({
                "kind": "method",
                "name": method.name,
                "sourceIdentity": class.source_identity,
                "sourcePath": class.source_path,
            })),
            ..Default::default()
        })
        .collect()
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
