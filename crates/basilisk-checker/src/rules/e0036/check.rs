//! Check functions for BSK-E0036.

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, Severity};

use super::types::TypeParamKind;
use super::utils::{
    contains_type_param, count_top_level_args, has_classvar, is_ident_char, is_numeric_literal,
    is_runtime_variable, make_diagnostic, span_text, CODE,
};

/// Validate `ClassVar` arguments for correctness.
pub(super) fn check_classvar_args(
    inner: &str,
    attr_name: &str,
    name_span: Span,
    path: &str,
    type_param_names: &[(String, TypeParamKind)],
    module_var_names: &[String],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let arg_count = count_top_level_args(inner);

    // Too many arguments (ClassVar accepts at most 1)
    if arg_count > 1 {
        diagnostics.push(make_diagnostic(
            format!(
                "`ClassVar` accepts at most one type argument, but `{attr_name}` has {arg_count}",
            ),
            name_span,
            path,
        ));
        return;
    }

    let trimmed = inner.trim();

    // Check for numeric literal argument (e.g. ClassVar[3])
    if is_numeric_literal(trimmed) {
        diagnostics.push(make_diagnostic(
            format!(
                "Invalid `ClassVar` argument for `{attr_name}`: `{trimmed}` is not a valid type",
            ),
            name_span,
            path,
        ));
        return;
    }

    // Check for runtime variable argument (e.g. ClassVar[var])
    if is_runtime_variable(trimmed, module_var_names) {
        diagnostics.push(make_diagnostic(
            format!(
                "Invalid `ClassVar` argument for `{attr_name}`: `{trimmed}` is a runtime variable, not a type",
            ),
            name_span,
            path,
        ));
        return;
    }

    // Check for TypeVar/ParamSpec/TypeVarTuple in ClassVar argument
    if let Some(kind) = contains_type_param(trimmed, type_param_names) {
        let kind_name = match kind {
            TypeParamKind::TypeVar => "TypeVar",
            TypeParamKind::ParamSpec => "ParamSpec",
            TypeParamKind::TypeVarTuple => "TypeVarTuple",
        };
        diagnostics.push(make_diagnostic(
            format!("`ClassVar` parameter for `{attr_name}` cannot contain {kind_name}",),
            name_span,
            path,
        ));
    }
}

/// Check for type mismatch between a `ClassVar` annotation's inner type and the
/// RHS value.  For example, `ClassVar[list[str]] = {}` is a mismatch because `{}`
/// is a dict literal but the annotation expects a list.
pub(super) fn check_classvar_type_mismatch(
    inner: &str,
    rhs_text: &str,
    attr_name: &str,
    name_span: Span,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let trimmed_inner = inner.trim();
    let trimmed_rhs = rhs_text.trim();

    if trimmed_rhs.is_empty() || trimmed_inner.is_empty() {
        return;
    }

    // Detect dict literal `{}` or `{...}` assigned to a list/set/tuple type
    let rhs_is_dict = trimmed_rhs.starts_with('{');
    let rhs_is_list = trimmed_rhs.starts_with('[');

    let inner_is_list = trimmed_inner.starts_with("list") || trimmed_inner.starts_with("List");
    let inner_is_dict = trimmed_inner.starts_with("dict") || trimmed_inner.starts_with("Dict");
    let inner_is_set = trimmed_inner.starts_with("set")
        || trimmed_inner.starts_with("Set")
        || trimmed_inner.starts_with("frozenset")
        || trimmed_inner.starts_with("FrozenSet");

    // Dict literal assigned to list/set/tuple type
    if rhs_is_dict && (inner_is_list || inner_is_set) {
        diagnostics.push(make_diagnostic(
            format!(
                "Type mismatch in `ClassVar` attribute `{attr_name}`: \
                 annotated as `{trimmed_inner}` but initialized with a dict literal",
            ),
            name_span,
            path,
        ));
        return;
    }

    // List literal assigned to dict type
    if rhs_is_list && inner_is_dict {
        diagnostics.push(make_diagnostic(
            format!(
                "Type mismatch in `ClassVar` attribute `{attr_name}`: \
                 annotated as `{trimmed_inner}` but initialized with a list literal",
            ),
            name_span,
            path,
        ));
    }
}

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
                span_text(source, attr.annotation_span).is_some_and(|ann| {
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
        if let Some(rhs) = span_text(source, var.rhs_span) {
            // Check if RHS is a constructor call: ClassName(...)
            if let Some(paren_idx) = rhs.find('(') {
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

/// Check module-level annotated assignments for protocol `ClassVar` conformance.
///
/// When a variable is typed as a `Protocol` with `ClassVar` attributes, the RHS
/// implementation class must have those attributes defined at the **class level**
/// (not merely as `self.x = ...` in `__init__`).
///
/// e.g. `a: ProtoA = ProtoAImpl()` where `ProtoA` requires `y: ClassVar[str]`
/// but `ProtoAImpl` only sets `self.y = ""` in `__init__` (instance variable).
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
                span_text(source, attr.annotation_span).is_some_and(|ann| {
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
        let Some(ann_text) = span_text(source, var.annotation_span) else {
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
        let Some(rhs_text) = span_text(source, var.rhs_span) else {
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


