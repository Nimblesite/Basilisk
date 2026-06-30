//! Implements [`assignment_compatibility`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-typesafety
//! Dataclass attribute assignment checking for `assignment_compatibility`.
//!
//! Validates module-level attribute assignments (`instance.field = value`)
//! against the declared field types of `dataclass`/`dataclass_transform`
//! classes, catching obvious literal kind mismatches.

use std::collections::HashMap;

use basilisk_resolver::{ResolvedModule, RhsKind, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic};
use crate::span_util::slice_span;

use super::CODE;

/// Returns `Some(description)` when the annotation text and RHS kind are
/// clearly incompatible; `None` when the pairing is acceptable or unknown.
pub(super) fn annotation_rhs_mismatch_simple(
    annotation: &str,
    rhs: &RhsKind,
) -> Option<&'static str> {
    // Normalise: strip generic parameters and whitespace, lower-case.
    let base = annotation
        .split('[')
        .next()
        .unwrap_or(annotation)
        .trim()
        .to_ascii_lowercase();

    match (base.as_str(), rhs) {
        ("int" | "bool" | "float" | "bytes", RhsKind::StrLiteral) => Some("a `str` literal"),
        ("int" | "str" | "float", RhsKind::BytesLiteral) => Some("a `bytes` literal"),
        ("int" | "str" | "bool", RhsKind::FloatLiteral) => Some("a `float` literal"),
        ("str" | "bytes", RhsKind::IntLiteral) => Some("an `int` literal"),
        _ => None,
    }
}

/// Checks module-level attribute assignments (`instance.field = value`) against
/// the declared field types of `dataclass`/`dataclass_transform` classes.
pub(super) fn check_dataclass_attr_assignments(
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if module.module_attr_assignments.is_empty() {
        return;
    }

    let transform_classes = super::super::guards::collect_transform_classes(module);

    // Build a map: class_name -> { field_name -> annotation_text }
    let mut class_field_types: HashMap<&str, HashMap<&str, &str>> = HashMap::new();
    for cls in &module.classes {
        let is_dc_like = cls.is_dataclass || transform_classes.contains_key(cls.name.as_str());
        if !is_dc_like {
            continue;
        }
        let mut fields = HashMap::new();
        for attr in &cls.attributes {
            if let Some(ann_span) = attr.annotation_span {
                if let Some(ann_text) = slice_span(&module.source, ann_span) {
                    let _ = fields.insert(attr.name.as_str(), ann_text.trim());
                }
            }
        }
        let _ = class_field_types.insert(cls.name.as_str(), fields);
    }

    if class_field_types.is_empty() {
        return;
    }

    // Build a map: variable_name -> class_name (for instances of DC-like classes)
    let source = &module.source;
    let instance_class: HashMap<&str, &str> = module
        .module_vars
        .iter()
        .filter_map(|var| {
            let rhs_span = var.rhs_span?;
            let rhs_text = slice_span(source, rhs_span)?;
            let callee = rhs_text.split(['(', '[']).next()?.trim();
            let callee = callee.rsplit('.').next().unwrap_or(callee);
            if class_field_types.contains_key(callee) {
                Some((var.name.as_str(), callee))
            } else {
                None
            }
        })
        .collect();

    if instance_class.is_empty() {
        return;
    }

    for assign in &module.module_attr_assignments {
        let Some(&class_name) = instance_class.get(assign.object_name.as_str()) else {
            continue;
        };
        let Some(fields) = class_field_types.get(class_name) else {
            continue;
        };
        let Some(&field_type) = fields.get(assign.attr_name.as_str()) else {
            continue;
        };

        // Extract the RHS literal kind from the source line
        let rhs_kind = extract_rhs_kind_from_assign(source, assign.target_span);
        if let Some(kind) = rhs_kind {
            if let Some(rhs_description) = annotation_rhs_mismatch_simple(field_type, &kind) {
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Type mismatch: `{}.{}` is typed `{field_type}` but assigned {rhs_description}",
                        assign.object_name, assign.attr_name
                    ),
                    assign.target_span,
                    &module.path,
                    Some(format!(
                        "Field `{}` of `{class_name}` expects `{field_type}`",
                        assign.attr_name
                    )),
                    Some(
                        "Basilisk requires attribute assignments to be compatible with the declared field type"
                            .to_owned(),
                    ),
                ));
            }
        }
    }
}

/// Extracts the RHS literal kind from a module-level attribute assignment line.
///
/// Given the target span of `obj.attr` in `obj.attr = value`, finds the `= value`
/// portion and determines the literal kind.
fn extract_rhs_kind_from_assign(source: &str, target_span: Span) -> Option<RhsKind> {
    let target_end = target_span.end_usize();
    let line_end = source
        .get(target_end..)?
        .find('\n')
        .map_or(source.len(), |pos| target_end + pos);
    let after_target = source.get(target_end..line_end)?;

    // Find `=` after the target
    let eq_pos = after_target.find('=')?;
    let rhs = after_target.get(eq_pos + 1..)?.trim();

    classify_literal(rhs)
}

/// Classifies a simple literal token into a `RhsKind`.
fn classify_literal(text: &str) -> Option<RhsKind> {
    if text.is_empty() {
        return None;
    }

    // Integer literal: starts with digit, no dot
    if text.bytes().next()?.is_ascii_digit() {
        if text.contains('.') {
            return Some(RhsKind::FloatLiteral);
        }
        return Some(RhsKind::IntLiteral);
    }

    // String literal
    if text.starts_with('"')
        || text.starts_with('\'')
        || text.starts_with("f\"")
        || text.starts_with("f'")
    {
        return Some(RhsKind::StrLiteral);
    }

    // Bytes literal
    if text.starts_with("b\"") || text.starts_with("b'") {
        return Some(RhsKind::BytesLiteral);
    }

    // None
    if text.starts_with("None") {
        return Some(RhsKind::NoneValue);
    }

    // Negative numbers
    if text.starts_with('-') {
        return classify_literal(text.get(1..)?.trim_start());
    }

    None
}
