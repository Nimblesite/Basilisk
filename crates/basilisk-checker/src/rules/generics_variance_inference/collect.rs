//! Implements [`generics_variance_inference`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Collection helpers for `generics_variance_inference`.

use std::collections::HashMap;

use super::types::{GenericClassDef, GenericInstance};
use crate::rules::shared::parse_subscript_annotation;

use super::utils::extract_typevar_params_from_generic;

/// Scan source text to collect generic class definitions.
pub(super) fn collect_generic_classes(source: &str) -> Vec<GenericClassDef> {
    let lines: Vec<&str> = source.lines().collect();
    let mut classes: Vec<GenericClassDef> = Vec::new();

    let mut idx = 0usize;
    while idx < lines.len() {
        let Some(line) = lines.get(idx).copied() else {
            break;
        };
        let trimmed = line.trim();

        // Detect class definitions with Generic[...] base.
        if trimmed.starts_with("class ") && trimmed.contains("Generic[") {
            // Extract class name.
            let after_class = &trimmed[6..];
            let class_name = after_class
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .map_or(after_class, |pos| &after_class[..pos])
                .to_owned();

            // Extract TypeVar params from Generic[...].
            let typevar_params = extract_typevar_params_from_generic(trimmed);

            // Determine class body indentation (next non-empty, non-comment line).
            let class_indent = line.len() - line.trim_start().len();

            // Scan the class body for method definitions.
            let mut methods: HashMap<String, Vec<(String, String)>> = HashMap::new();
            let mut body_idx = idx + 1;
            while body_idx < lines.len() {
                let Some(body_line) = lines.get(body_idx).copied() else {
                    break;
                };
                let body_trimmed = body_line.trim();

                if body_trimmed.is_empty() || body_trimmed.starts_with('#') {
                    body_idx += 1;
                    continue;
                }

                let body_indent = body_line.len() - body_line.trim_start().len();

                // If we hit something at or before class indent that's not empty, stop.
                if body_indent <= class_indent && !body_trimmed.is_empty() {
                    break;
                }

                // Only look at direct methods (one indent level deeper).
                if body_indent == class_indent + 4 && body_trimmed.starts_with("def ") {
                    // Parse the method signature.
                    let after_def = &body_trimmed[4..];
                    if let Some(paren_pos) = after_def.find('(') {
                        let method_name = after_def[..paren_pos].trim().to_owned();
                        // Extract params from the full signature text.
                        // Check that the closing paren is on this line (single-line defs only).
                        let mut depth = 0i32;
                        let mut found_close = false;
                        for ch in body_trimmed.chars() {
                            match ch {
                                '(' => depth += 1,
                                ')' => {
                                    depth -= 1;
                                    if depth == 0 {
                                        found_close = true;
                                        break;
                                    }
                                }
                                _ => {}
                            }
                        }
                        // If not found on this line, skip (multi-line not supported).
                        if !found_close {
                            body_idx += 1;
                            continue;
                        }

                        // Extract params from within parens.
                        if let Some(open) = body_trimmed.find('(') {
                            if let Some(close) = body_trimmed.rfind(')') {
                                let params_text = &body_trimmed[open + 1..close];
                                let params = parse_method_params(params_text);
                                let _ = methods.insert(method_name, params);
                            }
                        }
                    }
                }

                body_idx += 1;
            }

            classes.push(GenericClassDef {
                name: class_name,
                typevar_params,
                methods,
            });
        }

        idx += 1;
    }

    classes
}

/// Parse method parameters text (inside parens), returning `(param_name, annotation)` pairs.
/// Skips `self`.
pub(super) fn parse_method_params(params_text: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    let mut param_parts: Vec<&str> = Vec::new();

    for (idx, ch) in params_text.char_indices() {
        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' => depth -= 1,
            ',' if depth == 0 => {
                param_parts.push(params_text[start..idx].trim());
                start = idx + 1;
            }
            _ => {}
        }
    }
    param_parts.push(params_text[start..].trim());

    let mut first = true;
    for param in param_parts {
        if param.is_empty() {
            continue;
        }
        // Skip `self` and `cls`.
        if first {
            let param_name = param.split(':').next().unwrap_or(param).trim();
            if param_name == "self" || param_name == "cls" {
                first = false;
                continue;
            }
        }
        first = false;

        // Parse `name: annotation` or just `name`.
        if let Some(colon_pos) = param.find(':') {
            let param_name = param[..colon_pos].trim().to_owned();
            // Strip defaults: find `=` at depth 0 in annotation.
            let ann_raw = &param[colon_pos + 1..];
            let annotation = ann_raw
                .split('=')
                .next()
                .unwrap_or(ann_raw)
                .trim()
                .to_owned();
            result.push((param_name, annotation));
        }
    }
    result
}

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
