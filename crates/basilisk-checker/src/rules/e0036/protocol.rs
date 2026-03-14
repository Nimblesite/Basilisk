//! Protocol ClassVar conformance checks for BSK-E0036.
//!
//! When a variable is typed as a `Protocol` with `ClassVar` attributes, the RHS
//! implementation class must have those attributes defined at the **class level**
//! (not merely as `self.x = ...` in `__init__`).
//!
//! e.g. `a: ProtoA = ProtoAImpl()` where `ProtoA` requires `y: ClassVar[str]`
//! but `ProtoAImpl` only sets `self.y = ""` in `__init__` (instance variable).

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, Severity};

use super::helpers::CODE;

/// Check module-level annotated assignments for protocol `ClassVar` conformance.
///
/// When a variable is typed as a `Protocol` with `ClassVar` attributes, the RHS
/// implementation class must have those attributes defined at the **class level**
/// (not merely as `self.x = ...` in `__init__`).
#[expect(
    clippy::too_many_lines,
    reason = "protocol ClassVar conformance requires extensive matching logic"
)]
pub(super) fn check_protocol_classvar_conformance(
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = &module.source;
    let path = &module.path;

    // Step 1: Build a map of protocol classes -> their ClassVar attribute names.
    let mut protocol_classvar_attrs: Vec<(&str, Vec<&str>)> = Vec::new();
    for cls in &module.classes {
        if !cls.bases.iter().any(|b| b == "Protocol") {
            continue;
        }
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
            protocol_classvar_attrs.push((&cls.name, cv_names));
        }
    }

    if protocol_classvar_attrs.is_empty() {
        return;
    }

    // Step 2: Build a map of non-protocol class names -> their class-level attribute names.
    let class_level_attrs: Vec<(&str, Vec<&str>)> = module
        .classes
        .iter()
        .filter(|cls| !cls.bases.iter().any(|b| b == "Protocol"))
        .map(|cls| {
            let attr_names: Vec<&str> = cls.attributes.iter().map(|a| a.name.as_str()).collect();
            (cls.name.as_str(), attr_names)
        })
        .collect();

    // Step 3: Check module-level annotated assignments like `a: ProtoName = ClassName(...)`.
    for var in &module.module_vars {
        // Get the annotation text (e.g. "ProtoA").
        let Some(ann_text) =
            var.annotation_span
                .and_then(|s| crate::span_util::slice_span(source, s))
        else {
            continue;
        };
        let ann_trimmed = ann_text.trim();

        // Check if the annotation names a protocol with ClassVar attrs.
        let Some((_proto_name, required_cv_attrs)) = protocol_classvar_attrs
            .iter()
            .find(|(name, _)| *name == ann_trimmed)
        else {
            continue;
        };

        // Get the RHS text and check if it's a constructor call.
        let Some(rhs_text) = var.rhs_span.and_then(|s| crate::span_util::slice_span(source, s))
        else {
            continue;
        };
        let rhs_trimmed = rhs_text.trim();

        // Extract class name from constructor call: ClassName(...)
        let Some(paren_idx) = rhs_trimmed.find('(') else {
            continue;
        };
        let Some(impl_class_name) = rhs_trimmed.get(..paren_idx) else {
            continue;
        };
        let impl_class_name = impl_class_name.trim();
        if impl_class_name.is_empty()
            || !impl_class_name
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_uppercase())
            || !impl_class_name
                .chars()
                .all(|ch| ch.is_alphanumeric() || ch == '_')
        {
            continue;
        }

        // Find the implementation class's class-level attributes.
        let Some((_cls_name, cls_attrs)) = class_level_attrs
            .iter()
            .find(|(name, _)| *name == impl_class_name)
        else {
            continue;
        };

        // Check each required ClassVar attribute.
        for cv_attr in required_cv_attrs {
            if !cls_attrs.contains(cv_attr) {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Class `{impl_class_name}` is not compatible with protocol \
                         `{ann_trimmed}`: attribute `{cv_attr}` is required to be a \
                         class variable (`ClassVar`) but is not defined at class level",
                    ),
                    span: var.name_span,
                    path: path.to_owned(),
                    help: Some(format!(
                        "Define `{cv_attr}` as a class-level attribute in \
                         `{impl_class_name}` instead of assigning via `self.{cv_attr}` \
                         in `__init__`",
                    )),
                    note: Some(
                        "Protocol `ClassVar` attributes must be class-level variables \
                         in the implementation, not instance variables"
                            .to_owned(),
                    ),
                });
            }
        }
    }
}
