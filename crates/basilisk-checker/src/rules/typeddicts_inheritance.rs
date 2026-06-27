//! Implements [BSK-E0038] from [CHKARCH-DIAG-OWNERSHIP] and
//! [CHKARCH-DIAG-TYPEDDICT-READONLY-INHERITANCE]. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPEDDICT-READONLY-INHERITANCE
//! BSK-E0038: Invalid `TypedDict` inheritance.
//!
//! PEP 589 and the typing spec place constraints on `TypedDict` inheritance:
//!
//! 1. A `TypedDict` cannot inherit from both a `TypedDict` and a non-TypedDict
//!    base class (except `Generic`).
//!
//! 2. A `TypedDict` subclass cannot change the type of a field declared in a
//!    parent `TypedDict` class. PEP 705 refines this for the `ReadOnly`,
//!    `Required`, and `NotRequired` qualifiers:
//!    - A writable (non-`ReadOnly`) item may not be redeclared `ReadOnly`.
//!    - A required item may not be redeclared as not-required.
//!    - A writable item's value type is invariant; a `ReadOnly` item's value
//!      type may be narrowed to a subtype.
//!
//! 3. Multiple `TypedDict` inheritance is not allowed when two bases declare
//!    the same field with conflicting types or qualifiers.

use std::collections::HashMap;

use basilisk_resolver::{AttributeInfo, ClassInfo, ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0038",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0038",
};

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    error_diagnostic(
        CODE.clone(),
        message,
        span,
        path,
        None,
        Some("PEP 589: TypedDict subclassing has strict field-compatibility requirements"),
    )
}

fn annotation_text(source: &str, span: Option<Span>) -> Option<&str> {
    slice_span(source, span?)
}

/// A `TypedDict` field's qualifier state, parsed from its annotation text.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FieldQualifiers<'a> {
    /// `true` when the annotation wraps the type in `ReadOnly`.
    readonly: bool,
    /// Effective required-ness: an explicit `Required`/`NotRequired` wins,
    /// otherwise the declaring class's `total=` setting decides.
    required: bool,
    /// The underlying type text with all qualifier wrappers stripped.
    core: &'a str,
}

/// Parse the `ReadOnly`/`Required`/`NotRequired` qualifiers and core type out of
/// a field annotation, resolving implicit required-ness against `class_total`.
fn parse_field_qualifiers(annotation: &str, class_total: bool) -> FieldQualifiers<'_> {
    let lower = annotation.to_ascii_lowercase();
    let readonly = lower.contains("readonly[");
    // `NotRequired` must be tested first — its text contains `required[`.
    let required = if lower.contains("notrequired[") {
        false
    } else if lower.contains("required[") {
        true
    } else {
        class_total
    };
    FieldQualifiers {
        readonly,
        required,
        core: basilisk_resolver::strip_typeddict_qualifiers(annotation),
    }
}

/// Returns the reason a child redeclaration of an inherited `base` field is
/// illegal under the typing spec / PEP 705, or `None` when it is allowed.
fn redeclaration_violation(
    base: &FieldQualifiers<'_>,
    child: &FieldQualifiers<'_>,
) -> Option<&'static str> {
    // A writable item may not be redeclared as ReadOnly.
    if !base.readonly && child.readonly {
        return Some("a writable item may not be redeclared as `ReadOnly`");
    }
    // A required item may not be redeclared as not-required.
    if base.required && !child.required {
        return Some("a required item may not be redeclared as not-required");
    }
    // Value-type compatibility (invariant when writable, narrowing when ReadOnly).
    if base.core != child.core && value_type_incompatible(base, child) {
        return Some("the value type is not compatible with the inherited declaration");
    }
    None
}

/// Decide whether a changed value type is incompatible given the base's
/// read-only-ness. Writable items are invariant (any change is illegal);
/// `ReadOnly` items may narrow to a subtype, approximated as: a different
/// container head is a legal narrowing, but the same *invariant* container with
/// different type arguments is not.
fn value_type_incompatible(base: &FieldQualifiers<'_>, child: &FieldQualifiers<'_>) -> bool {
    if !base.readonly {
        return true;
    }
    type_head(base.core) == type_head(child.core) && is_invariant_container(type_head(base.core))
}

/// The container/base name of a type expression: the text before the first `[`.
fn type_head(core: &str) -> &str {
    core.split('[').next().unwrap_or(core).trim()
}

/// Containers whose element type is invariant, so narrowing their arguments in a
/// `ReadOnly` redeclaration remains illegal.
fn is_invariant_container(head: &str) -> bool {
    matches!(
        head,
        "list" | "dict" | "set" | "MutableSequence" | "MutableMapping" | "MutableSet"
    )
}

/// Returns `true` when two base declarations of the same field cannot be merged:
/// different core types, required-ness, or read-only-ness.
fn bases_conflict(left: &FieldQualifiers<'_>, right: &FieldQualifiers<'_>) -> bool {
    left.core != right.core || left.required != right.required || left.readonly != right.readonly
}

/// Checks rule 1: `TypedDict` cannot mix `TypedDict` and non-TypedDict bases (except `Generic`).
fn check_mixed_bases(
    cls: &ClassInfo,
    class_map: &HashMap<&str, &ClassInfo>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    const EXEMPT: &[&str] = &["Generic", "object"];
    let has_typed_dict_base = cls.bases.iter().any(|b| {
        b == "TypedDict" || basilisk_resolver::is_transitive_typeddict(b.as_str(), class_map)
    });
    if !has_typed_dict_base {
        return;
    }
    for base in &cls.bases {
        if base == "TypedDict" || EXEMPT.contains(&base.as_str()) {
            continue;
        }
        if !basilisk_resolver::is_transitive_typeddict(base.as_str(), class_map) {
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

/// Checks rule 2: a `TypedDict` subclass may not redeclare a parent field in a
/// way the typing spec / PEP 705 forbids (incompatible type, or an illegal
/// `ReadOnly`/`Required`/`NotRequired` change).
fn check_field_override(
    cls: &ClassInfo,
    typed_dict_bases: &[&str],
    class_map: &HashMap<&str, &ClassInfo>,
    attr_map: &HashMap<(&str, &str), &AttributeInfo>,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for base_name in typed_dict_bases {
        let base_total = class_map
            .get(base_name)
            .is_none_or(|c| c.is_typeddict_total);
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
            let Some(child_ann) = annotation_text(source, child_attr.annotation_span) else {
                continue;
            };
            let Some(base_ann) = annotation_text(source, base_attr.annotation_span) else {
                continue;
            };
            let base_q = parse_field_qualifiers(base_ann, base_total);
            let child_q = parse_field_qualifiers(child_ann, cls.is_typeddict_total);
            if let Some(reason) = redeclaration_violation(&base_q, &child_q) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "TypedDict `{}` illegally redeclares field `{}` from `{base_name}`: {reason}",
                        cls.name, child_attr.name,
                    ),
                    // Point at the offending field, not the class name — the
                    // redeclaration error belongs on the field's line.
                    child_attr.name_span,
                    path,
                ));
            }
        }
    }
}

/// Checks rule 3: Multiple `TypedDict` bases with conflicting declarations of
/// the same field — incompatible core types, required-ness, or read-only-ness.
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
    let mut seen: HashMap<&str, (&str, FieldQualifiers<'_>)> = HashMap::new();
    for base_name in typed_dict_bases {
        let Some(base_cls) = class_map.get(base_name) else {
            continue;
        };
        for attr in &base_cls.attributes {
            if !attr.has_annotation {
                continue;
            }
            let Some(ann) = annotation_text(source, attr.annotation_span) else {
                continue;
            };
            let qualifiers = parse_field_qualifiers(ann, base_cls.is_typeddict_total);
            if let Some((prev_base, prev_q)) =
                seen.insert(attr.name.as_str(), (base_name, qualifiers.clone()))
            {
                if bases_conflict(&prev_q, &qualifiers) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "TypedDict `{}` inherits incompatible declarations of field `{}` \
                             from `{prev_base}` and `{base_name}`",
                            cls.name, attr.name,
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
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let class_map = super::shared::class_name_map(&module.classes);

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
            if !basilisk_resolver::is_transitive_typeddict(cls.name.as_str(), &class_map) {
                continue;
            }

            check_mixed_bases(cls, &class_map, &module.path, diagnostics);

            let typed_dict_bases: Vec<&str> = cls
                .bases
                .iter()
                .filter(|b| {
                    *b != "TypedDict"
                        && basilisk_resolver::is_transitive_typeddict(b.as_str(), &class_map)
                })
                .map(String::as_str)
                .collect();

            check_field_override(
                cls,
                &typed_dict_bases,
                &class_map,
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
