//! Protocol `ClassVar` conformance checks for BSK-E0036.
//!
//! When a variable is typed as a `Protocol` with `ClassVar` attributes, the RHS
//! implementation class must have those attributes defined at the **class level**
//! (not merely as `self.x = ...` in `__init__`).
//!
//! e.g. `a: ProtoA = ProtoAImpl()` where `ProtoA` requires `y: ClassVar[str]`
//! but `ProtoAImpl` only sets `self.y = ""` in `__init__` (instance variable).

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, error_diagnostic_owned};

use super::helpers::{span_text, CODE};

/// Returns `true` when the annotation text looks like a `ClassVar` annotation.
fn is_classvar_annotation(ann: &str) -> bool {
    ann.starts_with("ClassVar[")
        || ann.starts_with("ClassVar ")
        || ann == "ClassVar"
        || ann.starts_with("CV[")
        || ann == "CV"
}

/// Check module-level annotated assignments for protocol `ClassVar` conformance.
///
/// When a variable is typed as a `Protocol` with `ClassVar` attributes, the RHS
/// implementation class must have those attributes defined at the **class level**
/// (not merely as `self.x = ...` in `__init__`).
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
                span_text(source, attr.annotation_span).is_some_and(is_classvar_annotation)
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
            let attr_names: Vec<&str> = basilisk_resolver::collect_names(&cls.attributes);
            (cls.name.as_str(), attr_names)
        })
        .collect();

    // Step 3: Check module-level annotated assignments like `a: ProtoName = ClassName(...)`.
    for var in &module.module_vars {
        // Get the annotation text (e.g. "ProtoA").
        let Some(ann_trimmed) = span_text(source, var.annotation_span).map(str::trim) else {
            continue;
        };

        // Check if the annotation names a protocol with ClassVar attrs.
        let Some((_proto_name, required_cv_attrs)) = protocol_classvar_attrs
            .iter()
            .find(|(name, _)| *name == ann_trimmed)
        else {
            continue;
        };

        // Get the RHS text and check if it's a constructor call.
        let Some(rhs_trimmed) = span_text(source, var.rhs_span).map(str::trim) else {
            continue;
        };

        // Extract class name from constructor call: ClassName(...)
        let Some(paren_idx) = rhs_trimmed.find('(') else {
            continue;
        };
        let Some(impl_class_name) = rhs_trimmed.get(..paren_idx).map(str::trim) else {
            continue;
        };

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
        emit_protocol_violations(
            required_cv_attrs,
            cls_attrs,
            impl_class_name,
            ann_trimmed,
            var.name_span,
            path,
            diagnostics,
        );
    }
}

/// Emit diagnostics for any required `ClassVar` attributes missing from the implementation class.
fn emit_protocol_violations(
    required_cv_attrs: &[&str],
    cls_attrs: &[&str],
    impl_class_name: &str,
    ann_trimmed: &str,
    name_span: basilisk_resolver::Span,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for cv_attr in required_cv_attrs {
        if !cls_attrs.contains(cv_attr) {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Class `{impl_class_name}` is not compatible with protocol \
                     `{ann_trimmed}`: attribute `{cv_attr}` is required to be a \
                     class variable (`ClassVar`) but is not defined at class level",
                ),
                name_span,
                path,
                Some(format!(
                    "Define `{cv_attr}` as a class-level attribute in \
                     `{impl_class_name}` instead of assigning via `self.{cv_attr}` \
                     in `__init__`",
                )),
                Some(
                    "Protocol `ClassVar` attributes must be class-level variables \
                     in the implementation, not instance variables"
                        .to_owned(),
                ),
            ));
        }
    }
}
