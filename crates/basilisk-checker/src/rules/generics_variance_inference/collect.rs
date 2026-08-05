//! Implements [`generics_variance_inference`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Collection helpers for `generics_variance_inference`.

use super::types::GenericInstance;
use crate::rules::shared::parse_subscript_annotation;

/// Collect module-level variables annotated with a concrete generic type.
pub(super) fn collect_generic_instances(source: &str) -> Vec<GenericInstance> {
    let mut instances = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        // Only look at lines at module level (no leading indent for simplicity)
        // and annotated assignments: `name: GenericClass[Type] = ...`
        if line.starts_with(|c: char| c.is_whitespace()) {
            continue;
        }
        // Must contain `:` and `[` (annotation with generic type).
        if !trimmed.contains(':') || !trimmed.contains('[') {
            continue;
        }
        // Skip class/def lines.
        if trimmed.starts_with("class ") || trimmed.starts_with("def ") {
            continue;
        }
        // Skip comment lines.
        if trimmed.starts_with('#') {
            continue;
        }
        // Find `:` for annotation.
        let Some(colon_pos) = trimmed.find(':') else {
            continue;
        };
        let var_name = trimmed[..colon_pos].trim();
        // Variable name must be a simple identifier.
        if var_name.is_empty() || !var_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        // Extract annotation (up to `=` or end of line, stripping comments).
        let after_colon = &trimmed[colon_pos + 1..];
        let ann_raw = after_colon.split('=').next().unwrap_or(after_colon).trim();
        let ann_text = ann_raw.split('#').next().unwrap_or(ann_raw).trim();

        if let Some((class_name, type_args)) = parse_subscript_annotation(ann_text) {
            instances.push(GenericInstance {
                var_name: var_name.to_owned(),
                class_name: class_name.to_owned(),
                type_args,
            });
        }
    }

    instances
}
