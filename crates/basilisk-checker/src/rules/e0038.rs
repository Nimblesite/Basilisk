//! BSK-E0038: Invalid `TypedDict` inheritance.
//!
//! PEP 589 and the typing spec place constraints on `TypedDict` inheritance:
//!
//! 1. A `TypedDict` cannot inherit from both a `TypedDict` and a non-TypedDict
//!    base class (except `Generic`).
//!
//! 2. A `TypedDict` subclass cannot change the type of a field declared in a
//!    parent `TypedDict` class.
//!
//! 3. Multiple `TypedDict` inheritance is not allowed when two bases declare
//!    the same field with conflicting types.

use std::collections::HashMap;

use basilisk_resolver::{AttributeInfo, ClassInfo, ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0038",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0038",
};

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message,
        span,
        path: path.to_owned(),
        help: None,
        note: Some(
            "PEP 589: TypedDict subclassing has strict field-compatibility requirements"
                .to_owned(),
        ),
    }
}

/// Returns `true` if this class is in a `TypedDict` hierarchy (directly or transitively).
fn is_typed_dict_class(name: &str, class_map: &HashMap<&str, &ClassInfo>) -> bool {
    let Some(cls) = class_map.get(name) else {
        return false;
    };
    if cls.is_typed_dict {
        return true;
    }
    cls.bases
        .iter()
        .any(|base| is_typed_dict_class(base.as_str(), class_map))
}

fn annotation_text(source: &str, span: Option<Span>) -> Option<&str> {
    let span = span?;
    source.get(span.start as usize..span.end as usize)
}

/// Returns `true` when the annotation contains `TypedDict` qualifier wrappers
/// (`ReadOnly`, `Required`, `NotRequired`) that change field subtyping semantics.
fn uses_qualifier_wrapper(ann: Option<&str>) -> bool {
    ann.is_some_and(|s| {
        s.contains("ReadOnly")
            || s.contains("Required")
            || s.contains("NotRequired")
    })
}

/// Checks rule 1: `TypedDict` cannot mix `TypedDict` and non-TypedDict bases (except `Generic`).
fn check_mixed_bases(
    cls: &ClassInfo,
    class_map: &HashMap<&str, &ClassInfo>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    const EXEMPT: &[&str] = &["Generic", "object"];
    let has_typed_dict_base = cls
        .bases
        .iter()
        .any(|b| b == "TypedDict" || is_typed_dict_class(b.as_str(), class_map));
    if !has_typed_dict_base {
        return;
    }
    for base in &cls.bases {
        if base == "TypedDict" || EXEMPT.contains(&base.as_str()) {
            continue;
        }
        if !is_typed_dict_class(base.as_str(), class_map) {
            diagnostics.push(make_diagnostic(
                format!(
                    "TypedDict `{}` cannot inherit from non-TypedDict class `{}`",
                    cls.name, base
                ),
                cls.name_span,
                path,
            ));
        }
    }
}

/// Checks rule 2: `TypedDict` subclass cannot change a parent field type.
fn check_field_override(
    cls: &ClassInfo,
    typed_dict_bases: &[&str],
    attr_map: &HashMap<(&str, &str), &AttributeInfo>,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for base_name in typed_dict_bases {
        for child_attr in &cls.attributes {
            if !child_attr.has_annotation {
                continue;
            }
            let Some(base_attr) = attr_map.get(&(base_name, child_attr.name.as_str())) else {
                continue;
            };
            if !base_attr.has_annotation {
                continue;
            }
            let child_ann = annotation_text(source, child_attr.annotation_span);
            let base_ann = annotation_text(source, base_attr.annotation_span);
            // Skip when qualifier wrappers are involved — subclasses may change
            // ReadOnly, Required, NotRequired status legally.
            if uses_qualifier_wrapper(child_ann) || uses_qualifier_wrapper(base_ann) {
                continue;
            }
            if child_ann != base_ann {
                diagnostics.push(make_diagnostic(
                    format!(
                        "TypedDict `{}` cannot override field `{}` with type `{}` \
                         (parent `{}` declares it as `{}`)",
                        cls.name,
                        child_attr.name,
                        child_ann.unwrap_or("unknown"),
                        base_name,
                        base_ann.unwrap_or("unknown"),
                    ),
                    cls.name_span,
                    path,
                ));
            }
        }
    }
}

/// Checks rule 3: Multiple `TypedDict` bases with conflicting field types.
fn check_conflicting_bases(
    cls: &ClassInfo,
    typed_dict_bases: &[&str],
    class_map: &HashMap<&str, &ClassInfo>,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if typed_dict_bases.len() < 2 {
        return;
    }
    let mut seen: HashMap<&str, (&str, Option<&str>)> = HashMap::new();
    for base_name in typed_dict_bases {
        let Some(base_cls) = class_map.get(base_name) else {
            continue;
        };
        for attr in &base_cls.attributes {
            if !attr.has_annotation {
                continue;
            }
            let ann = annotation_text(source, attr.annotation_span);
            if let Some((prev_base, prev_ann)) = seen.insert(attr.name.as_str(), (base_name, ann))
            {
                if prev_ann != ann
                    && !uses_qualifier_wrapper(ann)
                    && !uses_qualifier_wrapper(prev_ann)
                {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "TypedDict `{}` has conflicting definitions \
                             for field `{}`: `{}` declares `{}` but `{}` declares `{}`",
                            cls.name,
                            attr.name,
                            prev_base,
                            prev_ann.unwrap_or("unknown"),
                            base_name,
                            ann.unwrap_or("unknown"),
                        ),
                        cls.name_span,
                        path,
                    ));
                }
            }
        }
    }
}

/// Emits BSK-E0038 for invalid `TypedDict` inheritance.
pub(crate) struct InvalidTypedDictInheritance;

impl Rule for InvalidTypedDictInheritance {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let class_map: HashMap<&str, &ClassInfo> = module
            .classes
            .iter()
            .map(|cls| (cls.name.as_str(), cls))
            .collect();

        let attr_map: HashMap<(&str, &str), &AttributeInfo> = module
            .classes
            .iter()
            .flat_map(|cls| {
                cls.attributes
                    .iter()
                    .map(move |a| ((cls.name.as_str(), a.name.as_str()), a))
            })
            .collect();

        for cls in &module.classes {
            if !is_typed_dict_class(cls.name.as_str(), &class_map) {
                continue;
            }

            check_mixed_bases(cls, &class_map, &module.path, diagnostics);

            let typed_dict_bases: Vec<&str> = cls
                .bases
                .iter()
                .filter(|b| *b != "TypedDict" && is_typed_dict_class(b.as_str(), &class_map))
                .map(String::as_str)
                .collect();

            check_field_override(
                cls,
                &typed_dict_bases,
                &attr_map,
                &module.source,
                &module.path,
                diagnostics,
            );
            check_conflicting_bases(
                cls,
                &typed_dict_bases,
                &class_map,
                &module.source,
                &module.path,
                diagnostics,
            );
        }
    }
}
