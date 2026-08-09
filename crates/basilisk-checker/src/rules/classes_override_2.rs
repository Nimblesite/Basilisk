//! Implements [`classes_override_2`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! `classes_override_2`: Incompatible class attribute override.
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
fn is_typed_dict_hierarchy(_child: &ClassInfo, _class_map: &HashMap<&str, &ClassInfo>) -> bool {
    // ######################################################################
    // # DELETED BODY. DO NOT RESTORE IT AND DO NOT RETURN A DEFAULT.       #
    // #                                                                    #
    // #   class_map.get(base.as_str()).is_some_and(|b| b.is_typed_dict)    #
    // #                                                                    #
    // # It took a base class's identity from its RENDERED NAME and looked  #
    // # that string up in a name-keyed map. A base reached through an      #
    // # alias missed; a base sharing a rendered name with an unrelated     #
    // # local class matched. This gate decides whether the whole rule runs #
    // # at all, so a wrong answer here silently disables E0017 or applies  #
    // # it to a TypedDict where it produces only false positives.          #
    // #                                                                    #
    // # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs         #
    // ######################################################################
    panic!(
        "basilisk-checker: `is_typed_dict_hierarchy` was DELETED because it identified \
         a base class by its RENDERED NAME in a name-keyed map. It panics because the \
         real implementation — resolving each base expression through the binding table \
         — DOES NOT EXIST YET. Do not restore the lookup and do not return a default in \
         its place."
    )
}

#[expect(
    dead_code,
    reason = "caller deleted for spelling dependence; retained for the rebuild — see \
              tests/string_keyed_class_hierarchy_pin_tests.rs"
)]
const CODE: ErrorCode = ErrorCode {
    code: "classes_override_2",
    docs_url: "https://www.basilisk-python.dev/errors/classes_override_2",
};

/// Emits `classes_override_2` for class attributes that override a base-class attribute
/// with a different type annotation.
pub(crate) struct IncompatibleVariableOverride;

impl Rule for IncompatibleVariableOverride {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
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
                &module.source,
                &module.path,
                diagnostics,
            );
        });
    }
}

// ##################################################################
// # DELETED BODY. DO NOT RESTORE IT AND DO NOT RETURN A DEFAULT.
// #
// # `class_names.contains(&base_name.as_str())` and `attr_map.get(&(base_name.as_str(), ...))` keyed both the base class AND the inherited attribute on rendered names.
// #
// # `ClassInfo::bases` holds RENDERED SIMPLE NAMES ("complex
// # expressions ignored") and the lookup map is keyed on
// # `ClassInfo::name`, so an aliased base MISSED, a dotted base
// # collided with any local class sharing its trailing word, and two
// # classes with one rendered name were a single entry.
// #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
// ##################################################################
fn check_class(
    _child: &ClassInfo,
    _attr_map: &HashMap<(&str, &str), &AttributeInfo>,
    _class_names: &[&str],
    _source: &str,
    _path: &str,
    _out: &mut Vec<Diagnostic>,
) {
    panic!(
        "basilisk-checker: `classes_override_2::check_class` was DELETED because it identified base classes by \
         their RENDERED NAMES. It panics because the real implementation — base \
         expressions resolved through the binding table — DOES NOT EXIST YET. Do not \
         restore the name lookup and do not substitute a default answer."
    )
}

#[expect(
    dead_code,
    reason = "caller deleted for spelling dependence; retained for the rebuild — see \
              tests/string_keyed_class_hierarchy_pin_tests.rs"
)]
/// Extract annotation text from source given an optional span.
fn annotation_text(source: &str, span: Option<Span>) -> Option<&str> {
    slice_span(source, span?)
}

#[expect(
    dead_code,
    reason = "caller deleted for spelling dependence; retained for the rebuild — see \
              tests/string_keyed_class_hierarchy_pin_tests.rs"
)]
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
