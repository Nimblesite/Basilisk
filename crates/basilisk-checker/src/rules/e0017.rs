//! Implements [BSK-E0017] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-typesafety
//! BSK-E0017: Incompatible class attribute override.
//!
//! When a child class declares an attribute that also exists in a same-module
//! base class but with a different type annotation, Basilisk reports an
//! incompatible override.
//!
//! ```python
//! class Base:
//!     count: int = 0
//!
//! class Child(Base):
//!     count: str = "zero"   # annotation changed from int to str → E0017
//! ```

use std::collections::HashMap;

use basilisk_resolver::{AttributeInfo, ClassInfo, ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

/// Returns `true` when the child class is a `TypedDict` or inherits from one.
///
/// `TypedDict` subclassing has entirely different rules from normal OOP attribute
/// inheritance — subclasses can narrow `ReadOnly` items, change `Required`/`NotRequired`,
/// etc.  Applying E0017 to `TypedDict` classes produces only false positives.
fn is_typed_dict_hierarchy(child: &ClassInfo, class_map: &HashMap<&str, &ClassInfo>) -> bool {
    if child.is_typed_dict {
        return true;
    }
    child.bases.iter().any(|base| {
        class_map
            .get(base.as_str())
            .is_some_and(|b| b.is_typed_dict)
    })
}

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0017",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0017",
};

/// Emits BSK-E0017 for class attributes that override a base-class attribute
/// with a different type annotation.
pub(crate) struct IncompatibleVariableOverride;

impl Rule for IncompatibleVariableOverride {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Build map: class_name → &ClassInfo
        let class_map = super::shared::class_name_map(&module.classes);

        // Build map: (class_name, attr_name) → &AttributeInfo
        let attr_map: HashMap<(&str, &str), &AttributeInfo> = module
            .classes
            .iter()
            .flat_map(|cls| {
                cls.attributes
                    .iter()
                    .map(move |attr| ((cls.name.as_str(), attr.name.as_str()), attr))
            })
            .collect();

        let class_names: Vec<&str> = basilisk_resolver::collect_names(&module.classes);

        module.classes.iter().for_each(|child| {
            // TypedDict hierarchies have their own subtyping rules — skip.
            if is_typed_dict_hierarchy(child, &class_map) {
                return;
            }
            check_class(
                child,
                &attr_map,
                &class_names,
                &class_map,
                &module.source,
                &module.path,
                diagnostics,
            );
        });
    }
}

fn check_class(
    child: &ClassInfo,
    attr_map: &HashMap<(&str, &str), &AttributeInfo>,
    class_names: &[&str],
    class_map: &HashMap<&str, &ClassInfo>,
    source: &str,
    path: &str,
    out: &mut Vec<Diagnostic>,
) {
    for base_name in &child.bases {
        if !class_names.contains(&base_name.as_str()) {
            continue;
        }

        let base_is_dataclass = class_map
            .get(base_name.as_str())
            .is_some_and(|b| b.is_dataclass);

        for child_attr in &child.attributes {
            // Only check annotated attributes in child.
            if !child_attr.has_annotation {
                continue;
            }

            let Some(base_attr) = attr_map.get(&(base_name.as_str(), child_attr.name.as_str()))
            else {
                continue;
            };

            // Only compare if base also has an annotation.
            if !base_attr.has_annotation {
                continue;
            }

            let child_ann = annotation_text(source, child_attr.annotation_span);
            let base_ann = annotation_text(source, base_attr.annotation_span);

            // Skip when either side uses ReadOnly/Required/NotRequired wrappers.
            // TypedDict subclasses may legally strip ReadOnly, change Required to
            // NotRequired, etc. — string comparison cannot verify compatibility here.
            if uses_typed_dict_qualifier(child_ann) || uses_typed_dict_qualifier(base_ann) {
                continue;
            }

            // When both the child and the base are dataclasses, a non-`ClassVar`
            // annotation change is allowed (frozen dataclasses support covariant
            // attribute overrides).  We still flag `ClassVar` vs instance-variable
            // mismatches because those are always errors in dataclasses.
            if child.is_dataclass && base_is_dataclass {
                let child_is_classvar = child_ann.is_some_and(|s| s.contains("ClassVar"));
                let base_is_classvar = base_ann.is_some_and(|s| s.contains("ClassVar"));
                if child_is_classvar == base_is_classvar {
                    // Same ClassVar-ness: allow the override (frozen covariance).
                    continue;
                }
            }

            if child_ann != base_ann {
                out.push(make_diagnostic(
                    child_attr,
                    &child_attr.name,
                    &child.name,
                    base_name,
                    child_ann,
                    base_ann,
                    path,
                ));
            }
        }
    }
}

/// Returns `true` when an annotation string contains `TypedDict` qualifier wrappers.
///
/// `ReadOnly`, `Required`, and `NotRequired` change subtyping rules in ways
/// that a raw string comparison cannot capture — a subclass can legally strip
/// `ReadOnly` or change `Required` to `NotRequired`.  Skip E0017 for these.
fn uses_typed_dict_qualifier(ann: Option<&str>) -> bool {
    ann.is_some_and(|s| {
        s.contains("ReadOnly") || s.contains("Required") || s.contains("NotRequired")
    })
}

/// Extract annotation text from source given an optional span.
fn annotation_text(source: &str, span: Option<Span>) -> Option<&str> {
    slice_span(source, span?)
}

fn make_diagnostic(
    attr: &AttributeInfo,
    attr_name: &str,
    child_class: &str,
    base_class: &str,
    child_ann: Option<&str>,
    base_ann: Option<&str>,
    path: &str,
) -> Diagnostic {
    let child_ann_str = child_ann.unwrap_or("unknown");
    let base_ann_str = base_ann.unwrap_or("unknown");
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Attribute `{attr_name}` in `{child_class}` has type `{child_ann_str}` but \
             base class `{base_class}` declares it as `{base_ann_str}`"
        ),
        attr.name_span,
        path,
        Some(format!(
            "Change the annotation of `{attr_name}` in `{child_class}` to `{base_ann_str}` \
             to match the base class, or restructure the class hierarchy"
        )),
        Some(
            "In Basilisk, child class attributes must have type-compatible annotations \
             with any same-name attributes in base classes"
                .to_owned(),
        ),
    )
}
