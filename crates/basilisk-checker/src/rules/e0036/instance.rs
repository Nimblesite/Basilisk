//! Instance-level ClassVar violation checks for BSK-E0036.
//!
//! Handles two cases:
//! 1. `self.x: ClassVar[T]` annotations inside methods (invalid context).
//! 2. `instance.classvar_attr = value` assignments to class-level ClassVar
//!    attributes through an instance (forbidden by PEP 526).

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, Severity};

use super::helpers::{CODE, is_ident_char, make_diagnostic};

/// Scan source text for `self.<name>: ClassVar` or `self.<name>: CV` patterns
/// and return the spans and names of violations.
pub(super) fn find_self_classvar_annotations(source: &str) -> Vec<(String, Span)> {
    let mut results = Vec::new();
    let bytes = source.as_bytes();
    let source_len = bytes.len();
    let self_dot = b"self.";

    let mut idx = 0;
    while idx + self_dot.len() < source_len {
        // Find "self."
        if bytes.get(idx..idx + self_dot.len()) != Some(self_dot.as_slice()) {
            idx += 1;
            continue;
        }

        // Check that "self." is preceded by whitespace/newline/start (not part of a larger name)
        if idx > 0 && bytes.get(idx - 1).is_some_and(|&b| is_ident_char(b)) {
            idx += 1;
            continue;
        }

        let attr_start = idx + self_dot.len();

        // Collect the attribute name
        let mut attr_end = attr_start;
        while bytes.get(attr_end).is_some_and(|&b| is_ident_char(b)) {
            attr_end += 1;
        }
        if attr_end == attr_start {
            idx += 1;
            continue;
        }

        let Some(attr_bytes) = bytes.get(attr_start..attr_end) else {
            idx += 1;
            continue;
        };
        let attr_name = if let Ok(name) = std::str::from_utf8(attr_bytes) {
            name.to_owned()
        } else {
            idx += 1;
            continue;
        };

        // Skip whitespace after the attribute name
        let mut colon_idx = attr_end;
        while bytes.get(colon_idx) == Some(&b' ') {
            colon_idx += 1;
        }

        // Check for ":"
        if bytes.get(colon_idx) != Some(&b':') {
            idx = attr_end;
            continue;
        }

        // Skip whitespace after ":"
        let mut ann_start = colon_idx + 1;
        while bytes.get(ann_start) == Some(&b' ') {
            ann_start += 1;
        }

        // Check if annotation starts with "ClassVar" or "CV"
        let has_cv = if bytes.get(ann_start..ann_start + 8) == Some(b"ClassVar") {
            true
        } else {
            bytes.get(ann_start..ann_start + 2) == Some(b"CV")
                && (ann_start + 2 >= source_len
                    || bytes.get(ann_start + 2) == Some(&b'[')
                    || bytes.get(ann_start + 2) == Some(&b' '))
        };

        if has_cv {
            let Some(span_start) = u32::try_from(idx).ok() else {
                idx = attr_end;
                continue;
            };
            let Some(span_end) = u32::try_from(attr_end).ok() else {
                idx = attr_end;
                continue;
            };
            let span = Span {
                start: span_start,
                end: span_end,
            };
            results.push((attr_name, span));
        }

        idx = attr_end;
    }

    results
}

/// Check module-level attribute assignments to ClassVar-annotated class attributes.
///
/// e.g. `enterprise_d.stats = {}` where `stats: ClassVar[dict[str, int]]` in the class.
pub(super) fn check_instance_classvar_assignments(
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = &module.source;
    let path = &module.path;

    // Build a map of class names to their ClassVar attribute names
    let mut classvar_attrs: Vec<(&str, Vec<&str>)> = Vec::new();
    for cls in &module.classes {
        let cv_names: Vec<&str> = cls
            .attributes
            .iter()
            .filter(|attr| {
                crate::span_util::slice_span(source, attr.annotation_span.unwrap_or_default())
                    .is_some_and(|ann| {
                        ann.starts_with("ClassVar[")
                            || ann.starts_with("ClassVar ")
                            || ann == "ClassVar"
                            || ann.starts_with("CV[")
                            || ann == "CV"
                    })
            })
            .map(|attr| attr.name.as_str())
            .collect();
        if !cv_names.is_empty() {
            classvar_attrs.push((&cls.name, cv_names));
        }
    }

    if classvar_attrs.is_empty() {
        return;
    }

    // Build a map of variable names to their class types (simple heuristic)
    // Look for module-level assignments like `enterprise_d = Starship(3000)`
    let mut instance_class_map: Vec<(String, String)> = Vec::new();
    for var in &module.module_vars {
        let Some(rhs) = var.rhs_span.and_then(|s| crate::span_util::slice_span(source, s)) else {
            continue;
        };
        // Check if RHS is a constructor call: ClassName(...)
        let Some(paren_idx) = rhs.find('(') else {
            continue;
        };
        let Some(class_name) = rhs.get(..paren_idx) else {
            continue;
        };
        let class_name = class_name.trim();
        if !class_name.is_empty()
            && class_name
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_uppercase())
            && class_name
                .chars()
                .all(|ch| ch.is_alphanumeric() || ch == '_')
        {
            instance_class_map.push((var.name.clone(), class_name.to_owned()));
        }
    }

    // Check each module-level attribute assignment
    for assignment in &module.module_attr_assignments {
        // Find if the object is an instance of a class with ClassVar attrs
        let Some(class_name) = instance_class_map
            .iter()
            .find(|(var_name, _)| var_name == &assignment.object_name)
            .map(|(_, cls)| cls.as_str())
        else {
            // Could also be a direct class assignment (Starship.stats = {}) which is OK
            continue;
        };

        // Check if the attribute is a ClassVar
        let is_classvar_attr = classvar_attrs.iter().any(|(cls, attrs)| {
            *cls == class_name && attrs.contains(&assignment.attr_name.as_str())
        });

        if is_classvar_attr {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Cannot assign to `ClassVar` attribute `{}` through an instance of `{}`",
                    assignment.attr_name, class_name
                ),
                span: assignment.target_span,
                path: path.to_owned(),
                help: Some(
                    "Assign to the class directly instead: `ClassName.attr = value`".to_owned(),
                ),
                note: Some(
                    "PEP 526: ClassVar attributes can only be assigned on the class itself, \
                     not through instances"
                        .to_owned(),
                ),
            });
        }
    }
}

/// Emit BSK-E0036 for every `self.<name>: ClassVar` annotation found inside a method body.
///
/// These are not captured in `local_vars` because the assignment target is an `Attribute`
/// node rather than a `Name` node.
pub(super) fn check_self_classvar_annotations(
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = &module.source;
    let path = &module.path;

    let self_classvar_violations = find_self_classvar_annotations(source);
    for (attr_name, span) in &self_classvar_violations {
        // Only flag self.xxx ClassVar inside methods (verify by checking the span
        // falls within a function that has a class_name)
        let in_method = module.functions.iter().any(|func| {
            func.class_name.is_some()
                && func.def_span.start <= span.start
                // Use the next function/class boundary or end of source as upper bound
                && span.start > func.name_span.start
        });
        if in_method {
            diagnostics.push(make_diagnostic(
                format!(
                    "`ClassVar` is not allowed in self-attribute annotation for `{attr_name}`",
                ),
                *span,
                path,
            ));
        }
    }
}
