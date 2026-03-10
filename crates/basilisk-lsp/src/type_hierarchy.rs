//! Type Hierarchy handler: supertypes and subtypes navigation.

use basilisk_resolver::ResolvedModule;
use tower_lsp::lsp_types::{SymbolKind, TypeHierarchyItem, Url};

use crate::util::span_to_range;

/// Build a `TypeHierarchyItem` from a class name, storing the name in `data` for later lookups.
fn class_to_item(
    class: &basilisk_resolver::ClassInfo,
    source: &str,
    uri: &Url,
) -> TypeHierarchyItem {
    TypeHierarchyItem {
        name: class.name.clone(),
        kind: SymbolKind::CLASS,
        tags: None,
        detail: if class.bases.is_empty() {
            None
        } else {
            Some(format!("({})", class.bases.join(", ")))
        },
        uri: uri.clone(),
        range: span_to_range(source, class.def_span),
        selection_range: span_to_range(source, class.name_span),
        data: Some(serde_json::Value::String(class.name.clone())),
    }
}

/// Prepare type hierarchy at cursor position.
///
/// Returns a `TypeHierarchyItem` for the class whose name span contains the
/// given byte offset, or an empty vec if the cursor is not on a class name.
#[must_use]
pub fn prepare(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
    uri: &Url,
) -> Vec<TypeHierarchyItem> {
    for class in &resolved.classes {
        let start = class.name_span.start as usize;
        let end = class.name_span.end as usize;
        if start <= byte_offset && byte_offset < end {
            return vec![class_to_item(class, source, uri)];
        }
    }
    vec![]
}

/// Find supertypes (base classes) of the given class.
///
/// Looks up the `ClassInfo` for `class_name`, then finds each base class by
/// name in the resolved module and returns a `TypeHierarchyItem` for each.
#[must_use]
pub fn supertypes(
    resolved: &ResolvedModule,
    source: &str,
    class_name: &str,
    uri: &Url,
) -> Vec<TypeHierarchyItem> {
    let Some(class) = resolved.classes.iter().find(|c| c.name == class_name) else {
        return vec![];
    };

    class
        .bases
        .iter()
        .filter_map(|base_name| {
            let base_class = resolved.classes.iter().find(|c| c.name == *base_name)?;
            Some(class_to_item(base_class, source, uri))
        })
        .collect()
}

/// Find subtypes (derived classes) of the given class.
///
/// Scans all classes in the resolved module whose `bases` list contains
/// `class_name` and returns a `TypeHierarchyItem` for each.
#[must_use]
pub fn subtypes(
    resolved: &ResolvedModule,
    source: &str,
    class_name: &str,
    uri: &Url,
) -> Vec<TypeHierarchyItem> {
    resolved
        .classes
        .iter()
        .filter(|c| c.bases.iter().any(|b| b == class_name))
        .map(|c| class_to_item(c, source, uri))
        .collect()
}
