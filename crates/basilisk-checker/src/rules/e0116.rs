//! BSK-E0116: `NamedTuple` class definition errors.
//!
//! Detects several categories of `NamedTuple` definition errors:
//!
//! 1. **Underscore field names**: Field names starting with `_` are illegal in
//!    `NamedTuple` definitions (the runtime raises `ValueError`).
//!
//! 2. **Default ordering**: Fields with default values must come after all fields
//!    without defaults (same rule as the runtime enforces).
//!
//! 3. **Subclass field conflict**: A `NamedTuple` subclass cannot redefine fields
//!    that exist in the base `NamedTuple`.
//!
//! 4. **Multiple inheritance**: `NamedTuple` does not support inheriting from
//!    multiple bases (other than `Generic[...]`).

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, ResolvedModule};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0116",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0116",
};

/// Emits BSK-E0116 for `NamedTuple` class definition errors.
pub(crate) struct NamedTupleDefError;

impl Rule for NamedTupleDefError {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let class_map: HashMap<&str, &ClassInfo> = module
            .classes
            .iter()
            .map(|c| (c.name.as_str(), c))
            .collect();

        for class in &module.classes {
            if !is_namedtuple(class) {
                continue;
            }

            check_underscore_fields(class, &module.path, diagnostics);
            check_default_ordering(class, &module.path, &module.source, diagnostics);
            check_multiple_inheritance(class, &module.path, diagnostics);
            check_subclass_field_conflict(class, &class_map, &module.path, diagnostics);
        }
    }
}

/// Whether this class directly inherits from `NamedTuple`.
fn is_namedtuple(class: &ClassInfo) -> bool {
    class.bases.iter().any(|b| b == "NamedTuple")
}

/// Check 1: Field names starting with an underscore.
fn check_underscore_fields(class: &ClassInfo, path: &str, diagnostics: &mut Vec<Diagnostic>) {
    for attr in &class.attributes {
        if !attr.has_annotation {
            continue;
        }
        if attr.name.starts_with('_') {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "`NamedTuple` field name `{}` must not start with an underscore",
                    attr.name
                ),
                span: attr.name_span,
                path: path.to_owned(),
                help: Some("Rename the field to remove the leading underscore".to_owned()),
                note: Some(
                    "The `NamedTuple` runtime raises `ValueError` for field names \
                     starting with `_`"
                        .to_owned(),
                ),
            });
        }
    }
}

/// Check 2: Fields without defaults after fields with defaults.
fn check_default_ordering(
    class: &ClassInfo,
    path: &str,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen_default = false;

    for attr in &class.attributes {
        if !attr.has_annotation {
            // Un-annotated attributes are not NamedTuple fields.
            continue;
        }

        // Skip ClassVar fields - they are not NamedTuple fields.
        if let Some(ann_span) = attr.annotation_span {
            if let Some(ann_text) = source.get(ann_span.start as usize..ann_span.end as usize) {
                if ann_text.trim().starts_with("ClassVar") {
                    continue;
                }
            }
        }

        if attr.has_value {
            seen_default = true;
        } else if seen_default {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "`NamedTuple` field `{}` without a default value follows a field \
                     with a default value",
                    attr.name
                ),
                span: attr.name_span,
                path: path.to_owned(),
                help: Some(
                    "Move fields without defaults before fields with defaults, \
                     or add a default value"
                        .to_owned(),
                ),
                note: Some(
                    "In `NamedTuple`, fields with defaults must come after all \
                     fields without defaults"
                        .to_owned(),
                ),
            });
        }
    }
}

/// Check 3: Multiple inheritance with `NamedTuple`.
fn check_multiple_inheritance(class: &ClassInfo, path: &str, diagnostics: &mut Vec<Diagnostic>) {
    // NamedTuple allows inheriting from Generic[T] alongside NamedTuple.
    // Any other additional base (besides object) is illegal.
    let non_namedtuple_bases: Vec<&str> = class
        .bases
        .iter()
        .map(|b| b.split('[').next().unwrap_or(b.as_str()))
        .filter(|b| *b != "NamedTuple" && *b != "Generic" && *b != "object")
        .collect();

    if !non_namedtuple_bases.is_empty() {
        diagnostics.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "`NamedTuple` class `{}` cannot inherit from multiple bases: `{}`",
                class.name,
                non_namedtuple_bases.join("`, `")
            ),
            span: class.name_span,
            path: path.to_owned(),
            help: Some(
                "`NamedTuple` does not support multiple inheritance; \
                 remove the extra base classes"
                    .to_owned(),
            ),
            note: Some(
                "Only `Generic[...]` is allowed alongside `NamedTuple` as a base class".to_owned(),
            ),
        });
    }
}

/// Check 4: Subclass field conflict with base `NamedTuple`.
fn check_subclass_field_conflict(
    class: &ClassInfo,
    class_map: &HashMap<&str, &ClassInfo>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // This class is itself a NamedTuple. Check its subclasses for field conflicts.
    // Actually, we need to check classes that inherit FROM this NamedTuple class.
    // We check each class that has a base that is a NamedTuple class in the module.

    // Instead: for each NamedTuple class, collect field names from all NamedTuple
    // base classes and check for conflicts.
    // The current class IS a NamedTuple. Its bases may be other NamedTuple classes.
    // Collect field names from NamedTuple base classes.
    let base_field_names = collect_base_namedtuple_fields(class, class_map);
    if base_field_names.is_empty() {
        return;
    }

    for attr in &class.attributes {
        if !attr.has_annotation {
            continue;
        }
        if base_field_names.contains(&attr.name.as_str()) {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "`NamedTuple` subclass `{}` redefines field `{}` from base class",
                    class.name, attr.name
                ),
                span: attr.name_span,
                path: path.to_owned(),
                help: Some(format!(
                    "Remove the redefinition of `{}` or use a different field name",
                    attr.name
                )),
                note: Some(
                    "Fields added in a `NamedTuple` subclass must not conflict with \
                     fields in the base class"
                        .to_owned(),
                ),
            });
        }
    }
}

/// Collect field names from all base `NamedTuple` classes.
fn collect_base_namedtuple_fields<'a>(
    class: &'a ClassInfo,
    class_map: &'a HashMap<&str, &'a ClassInfo>,
) -> Vec<&'a str> {
    let mut field_names = Vec::new();
    for base_name in &class.bases {
        let stripped = base_name.split('[').next().unwrap_or(base_name.as_str());
        if stripped == "NamedTuple" || stripped == "Generic" || stripped == "object" {
            continue;
        }
        if let Some(base_class) = class_map.get(stripped) {
            // Check if the base is itself a NamedTuple.
            if base_class.bases.iter().any(|b| b == "NamedTuple")
                || is_transitive_namedtuple(base_class, class_map)
            {
                for attr in &base_class.attributes {
                    if attr.has_annotation {
                        field_names.push(attr.name.as_str());
                    }
                }
            }
        }
    }
    field_names
}

/// Check if a class transitively inherits from `NamedTuple`.
fn is_transitive_namedtuple(class: &ClassInfo, class_map: &HashMap<&str, &ClassInfo>) -> bool {
    for base_name in &class.bases {
        let stripped = base_name.split('[').next().unwrap_or(base_name.as_str());
        if stripped == "NamedTuple" {
            return true;
        }
        if let Some(base_class) = class_map.get(stripped) {
            if is_transitive_namedtuple(base_class, class_map) {
                return true;
            }
        }
    }
    false
}
