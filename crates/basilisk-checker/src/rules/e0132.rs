//! BSK-E0132: Inconsistent `TypeVar` ordering across base classes.
//!
//! When a class inherits from multiple generic bases that share a common
//! generic ancestor, the `TypeVar` argument orderings must be consistent.
//!
//! ```python
//! class Grandparent(Generic[T1, T2]): ...
//! class Parent(Grandparent[T1, T2]): ...
//! class BadChild(Parent[T1, T2], Grandparent[T2, T1]): ...  # E
//! ```
//!
//! `BadChild` inherits `Grandparent` twice — once via `Parent[T1, T2]`
//! (which maps to `Grandparent[T1, T2]`) and once directly as
//! `Grandparent[T2, T1]`.  The orderings conflict.

use std::collections::HashMap;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};
use crate::rules::shared::split_top_level_commas;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0132",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0132",
};

/// Emits BSK-E0132 when base classes impose inconsistent `TypeVar` orderings.
pub(crate) struct InconsistentTypeVarOrder;

impl Rule for InconsistentTypeVarOrder {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Build a map of class_name -> ClassInfo for classes in this module.
        let class_map: HashMap<&str, &basilisk_resolver::ClassInfo> = module
            .classes
            .iter()
            .map(|cls| (cls.name.as_str(), cls))
            .collect();

        for class in &module.classes {
            check_class(class, &class_map, &module.source, &module.path, diagnostics);
        }
    }
}

/// Parsed base class: name and optional type arguments.
struct BaseSubscript {
    name: String,
    type_args: Vec<String>,
}

/// Extract the base class expressions (with subscripts) from source text.
fn extract_base_subscripts(
    source: &str,
    class: &basilisk_resolver::ClassInfo,
) -> Vec<BaseSubscript> {
    let name_end = class.name_span.end_usize();

    // Find the opening `(` after the class name.
    let after_name = source.get(name_end..).unwrap_or("");
    let Some(open_paren_offset) = after_name.find('(') else {
        return Vec::new();
    };
    let bases_start = name_end + open_paren_offset + 1;

    // Find the matching `)` considering nested brackets.
    let rest = source.get(bases_start..).unwrap_or("");
    let mut depth = 1i32;
    let mut bases_end = bases_start + rest.len();
    for (idx, ch) in rest.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    bases_end = bases_start + idx;
                    break;
                }
            }
            _ => {}
        }
    }

    let bases_text = source.get(bases_start..bases_end).unwrap_or("");

    // Split by commas at depth 0.
    split_top_level_commas(bases_text)
        .iter()
        .filter_map(|base_text| parse_base_subscript(base_text.trim()))
        .collect()
}

/// Parse `Name[T1, T2]` into a `BaseSubscript`.
fn parse_base_subscript(text: &str) -> Option<BaseSubscript> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    // Skip keyword arguments like `metaclass=...`.
    if text.contains('=') && !text.contains('[') {
        return None;
    }

    if let Some(bracket_pos) = text.find('[') {
        let name = text[..bracket_pos].trim().to_owned();
        let inner = text.get(bracket_pos + 1..text.len() - 1).unwrap_or("");
        let type_args: Vec<String> = split_top_level_commas(inner)
            .iter()
            .map(|a| a.trim().to_owned())
            .collect();
        Some(BaseSubscript { name, type_args })
    } else {
        Some(BaseSubscript {
            name: text.to_owned(),
            type_args: Vec::new(),
        })
    }
}

/// For a given class, check if any direct base leads to a shared ancestor
/// with conflicting `TypeVar` orderings.
fn check_class(
    class: &basilisk_resolver::ClassInfo,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let base_subscripts = extract_base_subscripts(source, class);
    if base_subscripts.len() < 2 {
        return;
    }

    // For each direct base, compute the implied ancestor type arg mappings.
    // ancestor_name -> list of (type_args as resolved through the chain, origin base index)
    let mut ancestor_args: HashMap<String, Vec<(Vec<String>, usize)>> = HashMap::new();

    for (idx, base) in base_subscripts.iter().enumerate() {
        // The direct base itself is an "ancestor".
        if !base.type_args.is_empty() {
            ancestor_args
                .entry(base.name.clone())
                .or_default()
                .push((base.type_args.clone(), idx));
        }

        // If this base is a class defined in this module, propagate through its bases.
        if let Some(parent_class) = class_map.get(base.name.as_str()) {
            propagate_ancestors(
                parent_class,
                &base.type_args,
                class_map,
                source,
                idx,
                &mut ancestor_args,
                0,
            );
        }
    }

    // Check for conflicts: same ancestor reached with different type arg orderings.
    for (ancestor_name, mappings) in &ancestor_args {
        if mappings.len() < 2 {
            continue;
        }
        let Some(first_mapping) = mappings.first() else {
            continue;
        };
        let first_args = &first_mapping.0;
        for other in mappings.get(1..).unwrap_or_default() {
            if other.0 != *first_args {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Inconsistent TypeVar ordering for `{}` in base classes of `{}`",
                        ancestor_name, class.name
                    ),
                    span: class.name_span,
                    path: path.to_owned(),
                    help: Some(
                        "All paths to a shared generic ancestor must use the same TypeVar ordering"
                            .to_owned(),
                    ),
                    note: Some(
                        "PEP 484: type variable ordering must be consistent across base classes"
                            .to_owned(),
                    ),
                    provenance: None,
                });
                return; // One diagnostic per class is enough.
            }
        }
    }
}

/// Propagate type arg mappings up the class hierarchy.
///
/// Given that a child inherits `Parent[T1, T2]` and `Parent` is defined as
/// `class Parent(Grandparent[T1, T2])`, we substitute `Parent`'s type params
/// with the actual type args to get `Grandparent[T1, T2]` as seen by the child.
fn propagate_ancestors(
    parent: &basilisk_resolver::ClassInfo,
    child_args_for_parent: &[String],
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    source: &str,
    origin_idx: usize,
    ancestor_args: &mut HashMap<String, Vec<(Vec<String>, usize)>>,
    depth: usize,
) {
    // Prevent infinite recursion in circular hierarchies.
    if depth > 10 {
        return;
    }

    // Build a substitution map: parent's generic param name -> child's type arg.
    let parent_params: Vec<String> = parent
        .generic_params
        .iter()
        .map(|p| p.name.clone())
        .collect();

    let substitution: HashMap<&str, &str> = parent_params
        .iter()
        .zip(child_args_for_parent.iter())
        .map(|(param, arg)| (param.as_str(), arg.as_str()))
        .collect();

    // Get parent's own base subscripts.
    let parent_bases = extract_base_subscripts(source, parent);

    for parent_base in &parent_bases {
        if parent_base.type_args.is_empty() {
            continue;
        }
        // Skip `Generic` itself — it's not a real ancestor in the hierarchy sense.
        if parent_base.name == "Generic" || parent_base.name == "Protocol" {
            continue;
        }

        // Substitute type args.
        let resolved_args: Vec<String> = parent_base
            .type_args
            .iter()
            .map(|arg| {
                substitution
                    .get(arg.as_str())
                    .map_or_else(|| arg.clone(), |s| (*s).to_owned())
            })
            .collect();

        ancestor_args
            .entry(parent_base.name.clone())
            .or_default()
            .push((resolved_args.clone(), origin_idx));

        // Recurse upward.
        if let Some(grandparent) = class_map.get(parent_base.name.as_str()) {
            propagate_ancestors(
                grandparent,
                &resolved_args,
                class_map,
                source,
                origin_idx,
                ancestor_args,
                depth + 1,
            );
        }
    }
}
