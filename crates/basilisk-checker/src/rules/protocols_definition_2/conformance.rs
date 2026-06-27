//! Implements [protocols_definition_2] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! Protocol member-kind conformance checks for protocols_definition_2.
//!
//! Beyond simple member-name presence, these checks verify that an
//! implementation provides each protocol member in a compatible *form*:
//! read-write properties need settable members, and read-write instance
//! variables need a same-kind, same-type writable attribute.

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, FunctionInfo, ResolvedModule, VariableInfo};

use super::ast_index::AstIndex;
use super::CODE;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic};
use crate::span_util::slice_span;

/// Returns `true` when the annotation text is a `ClassVar` form.
pub(super) fn is_classvar_ann(ann: &str) -> bool {
    ann.starts_with("ClassVar[") || ann.starts_with("ClassVar ") || ann == "ClassVar"
}

/// Whitespace-insensitive comparison key for type text.
fn norm(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

/// The decorator lists recorded for `member` in `cls.method_decorators`.
fn decorator_lists<'a>(cls: &'a ClassInfo, member: &str) -> Vec<&'a Vec<String>> {
    cls.method_decorators
        .iter()
        .filter(|(name, _)| name == member)
        .map(|(_, decs)| decs)
        .collect()
}

/// `(is_property, has_setter)` for `member` based on its decorators.
fn property_kind(cls: &ClassInfo, member: &str) -> (bool, bool) {
    let lists = decorator_lists(cls, member);
    let is_property = lists.iter().any(|ds| ds.iter().any(|d| d == "property"));
    let has_setter = lists.iter().any(|ds| ds.iter().any(|d| d == "setter"));
    (is_property, has_setter)
}

// ---------------------------------------------------------------------------
// Read-write property conformance (e.g. a protocol `@property` + `@x.setter`)
// ---------------------------------------------------------------------------

/// Names of a protocol's read-write properties: a method decorated `@property`
/// that also has a sibling `@<name>.setter` (recorded as a `"setter"` decorator).
fn readwrite_property_members(cls: &ClassInfo) -> Vec<&str> {
    let mut members: Vec<&str> = Vec::new();
    for (name, decs) in &cls.method_decorators {
        let is_property = decs.iter().any(|d| d == "property");
        let (_, has_setter) = property_kind(cls, name);
        if is_property && has_setter && !members.contains(&name.as_str()) {
            members.push(name.as_str());
        }
    }
    members
}

/// If `impl_cls` provides `member` but in a form that cannot be written to,
/// return the human-readable reason; otherwise `None` (settable, or absent —
/// absence is reported separately as a missing member).
fn readwrite_property_violation(impl_cls: &ClassInfo, member: &str) -> Option<&'static str> {
    let has_attr = impl_cls.attributes.iter().any(|a| a.name == member);
    let has_method = impl_cls.method_names.iter().any(|m| m == member);
    if !has_attr && !has_method {
        return None;
    }

    if impl_cls.bases.iter().any(|b| b == "NamedTuple") {
        return Some("named tuple fields are immutable");
    }
    if impl_cls.is_dataclass_frozen {
        return Some("frozen dataclass fields are immutable");
    }

    // A read-only property (a `@property` getter with no `@<name>.setter`) is
    // not writable; a plain attribute or a property-with-setter is.
    let (is_property, has_setter) = property_kind(impl_cls, member);
    if is_property && !has_setter {
        return Some("a read-only property cannot satisfy a writable protocol member");
    }

    None
}

/// Check that read-write protocol properties are satisfied by settable members.
pub(super) fn check_readwrite_property_conformance(
    protocol_name: &str,
    protocol_class: &ClassInfo,
    rhs_class_name: &str,
    class_map: &HashMap<&str, &ClassInfo>,
    var: &VariableInfo,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let rw_props = readwrite_property_members(protocol_class);
    if rw_props.is_empty() {
        return;
    }
    let Some(impl_cls) = class_map.get(rhs_class_name) else {
        return;
    };

    for member in rw_props {
        if let Some(reason) = readwrite_property_violation(impl_cls, member) {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Class `{rhs_class_name}` is incompatible with protocol `{protocol_name}`: \
                     `{member}` must be writable but {reason}"
                ),
                var.name_span,
                path,
                Some(format!(
                    "Provide `{member}` as a writable attribute or a property with a setter \
                     in `{rhs_class_name}`"
                )),
                Some(
                    "A protocol property with a setter is read-write; the implementation must \
                     allow assignment to the member"
                        .to_owned(),
                ),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Read-write instance-variable conformance (a plain, non-`ClassVar` protocol
// attribute is a writable instance variable; its type is invariant)
// ---------------------------------------------------------------------------

/// Read-write instance-variable members of a protocol: class-body attributes
/// whose annotation is **not** `ClassVar`. Returns `(name, type_text)`.
fn instance_var_members<'a>(cls: &'a ClassInfo, source: &'a str) -> Vec<(&'a str, &'a str)> {
    cls.attributes
        .iter()
        .filter_map(|attr| {
            let ann = attr
                .annotation_span
                .and_then(|sp| slice_span(source, sp))?
                .trim();
            if is_classvar_ann(ann) {
                return None;
            }
            Some((attr.name.as_str(), ann))
        })
        .collect()
}

/// If `impl_cls` provides `member` but in a form incompatible with a writable
/// instance variable of type `proto_type`, return `(message, hint)`.
///
/// Only **present-but-wrong** cases are reported: a `ClassVar`, a read-only
/// property, or a type mismatch. Absence is intentionally NOT reported here —
/// an implementation may legitimately set the attribute via `self.x = ...` in
/// `__init__`, which is invisible to class-body attribute collection, so a
/// "missing" diagnostic would be a false positive on common real-world code.
fn instance_var_violation(
    impl_cls: &ClassInfo,
    member: &str,
    proto_type: &str,
    source: &str,
) -> Option<(String, String)> {
    if let Some(attr) = impl_cls.attributes.iter().find(|a| a.name == member) {
        let ann = attr
            .annotation_span
            .and_then(|sp| slice_span(source, sp))
            .map(str::trim)?;
        if is_classvar_ann(ann) {
            return Some((
                format!(
                    "`{member}` is required to be a writable instance variable but is \
                     declared as a class variable (`ClassVar`)"
                ),
                format!("Declare `{member}` as an instance variable, not `ClassVar`"),
            ));
        }
        // Writable instance variables are invariant: the type must match exactly.
        if norm(ann) != norm(proto_type) {
            return Some((
                format!(
                    "`{member}` has type `{ann}` but the protocol requires the writable \
                     instance variable to be `{proto_type}`"
                ),
                format!("Change the type of `{member}` to `{proto_type}`"),
            ));
        }
        return None;
    }

    // Not a class-body attribute: a read-only property cannot satisfy a
    // writable instance variable.
    let (is_property, has_setter) = property_kind(impl_cls, member);
    if is_property && !has_setter {
        return Some((
            format!(
                "`{member}` is required to be a writable instance variable but is a \
                 read-only property"
            ),
            format!("Make `{member}` a writable attribute or add a setter"),
        ));
    }

    None
}

/// Check that read-write protocol instance variables are satisfied by a
/// writable, same-type implementation attribute.
#[expect(
    clippy::too_many_arguments,
    reason = "protocol instance-var conformance threads full class/source context"
)]
pub(super) fn check_instance_var_conformance(
    protocol_name: &str,
    protocol_class: &ClassInfo,
    rhs_class_name: &str,
    class_map: &HashMap<&str, &ClassInfo>,
    source: &str,
    var: &VariableInfo,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let members = instance_var_members(protocol_class, source);
    if members.is_empty() {
        return;
    }
    let Some(impl_cls) = class_map.get(rhs_class_name) else {
        return;
    };

    for (member, proto_type) in members {
        if let Some((message, hint)) = instance_var_violation(impl_cls, member, proto_type, source)
        {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Class `{rhs_class_name}` is incompatible with protocol `{protocol_name}`: \
                     {message}"
                ),
                var.name_span,
                path,
                Some(hint),
                Some(
                    "Protocol instance variables are read-write and invariant: the \
                     implementation must provide a writable attribute of the same type"
                        .to_owned(),
                ),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Method-member conformance: a protocol property must not be satisfied by a
// plain method, and a protocol method's signature must be compatible.
// ---------------------------------------------------------------------------

/// Protocol members decorated `@property` (read-only or read-write).
fn property_members(cls: &ClassInfo) -> Vec<&str> {
    let mut members: Vec<&str> = Vec::new();
    for (name, decs) in &cls.method_decorators {
        if decs.iter().any(|d| d == "property") && !members.contains(&name.as_str()) {
            members.push(name.as_str());
        }
    }
    members
}

/// A protocol property member must be satisfied by a property or an attribute,
/// never by a plain (non-property) method.
pub(super) fn check_property_method_conformance(
    protocol_name: &str,
    protocol_class: &ClassInfo,
    rhs_class_name: &str,
    class_map: &HashMap<&str, &ClassInfo>,
    var: &VariableInfo,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let props = property_members(protocol_class);
    if props.is_empty() {
        return;
    }
    let Some(impl_cls) = class_map.get(rhs_class_name) else {
        return;
    };
    for member in props {
        let has_attr = impl_cls.attributes.iter().any(|a| a.name == member);
        let (impl_is_property, _) = property_kind(impl_cls, member);
        let has_method = impl_cls.method_names.iter().any(|m| m == member);
        if has_method && !impl_is_property && !has_attr {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Class `{rhs_class_name}` is incompatible with protocol `{protocol_name}`: \
                     `{member}` is a property in the protocol but a plain method in the \
                     implementation"
                ),
                var.name_span,
                path,
                Some(format!(
                    "Decorate `{member}` with `@property` in `{rhs_class_name}`"
                )),
                None,
            ));
        }
    }
}

/// Find the `FunctionInfo` for `class_name`'s `method`, if any.
fn find_method<'a>(
    module: &'a ResolvedModule,
    class_name: &str,
    method: &str,
) -> Option<&'a FunctionInfo> {
    module
        .functions
        .iter()
        .find(|f| f.name == method && f.class_name.as_deref() == Some(class_name))
}

/// Positional parameter names of a method, dropping the implicit `self`/`cls`
/// receiver for instance and class methods (but not for static methods).
fn logical_param_names(func: &FunctionInfo) -> Vec<&str> {
    let is_static = func.decorators.iter().any(|d| d == "staticmethod");
    let skip = usize::from(!is_static);
    func.parameters
        .iter()
        .skip(skip)
        .map(|p| p.name.as_str())
        .collect()
}

/// Emit a method-signature conformance diagnostic.
fn push_signature_diag(
    protocol_name: &str,
    rhs_class_name: &str,
    member: &str,
    reason: &str,
    var: &VariableInfo,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Class `{rhs_class_name}` is incompatible with protocol `{protocol_name}`: \
             method `{member}` signature is incompatible — {reason}"
        ),
        var.name_span,
        path,
        Some(format!(
            "Match `{member}`'s signature to the protocol declaration in `{protocol_name}`"
        )),
        None,
    ));
}

/// Check method-signature compatibility for plain (non-property, non-dunder)
/// protocol methods: a `@staticmethod` with a `self` receiver, and
/// positional-or-keyword parameter-name mismatches.
#[expect(
    clippy::too_many_arguments,
    reason = "method-signature conformance threads full protocol/impl context plus the AST index"
)]
pub(super) fn check_method_signature_conformance(
    protocol_name: &str,
    protocol_class: &ClassInfo,
    rhs_class_name: &str,
    module: &ResolvedModule,
    ast_index: Option<&AstIndex<'_>>,
    var: &VariableInfo,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let props = property_members(protocol_class);
    for member in &protocol_class.method_names {
        if member.starts_with("__") || props.contains(&member.as_str()) {
            continue;
        }
        let Some(proto_fn) = find_method(module, &protocol_class.name, member) else {
            continue;
        };
        let Some(impl_fn) = find_method(module, rhs_class_name, member) else {
            continue; // absence is reported by the missing-members check
        };

        // A `@staticmethod` whose first parameter is `self` cannot satisfy an
        // instance method — it has no bound receiver.
        let impl_static = impl_fn.decorators.iter().any(|d| d == "staticmethod");
        if impl_static && impl_fn.parameters.first().is_some_and(|p| p.name == "self") {
            push_signature_diag(
                protocol_name,
                rhs_class_name,
                member,
                "a static method with a `self` parameter cannot satisfy an instance method",
                var,
                path,
                diagnostics,
            );
            continue;
        }

        // Positional-or-keyword parameter names are part of the protocol. Skip
        // when either side uses `*args` (which accepts arbitrary positionals).
        if proto_fn.vararg.is_some() || impl_fn.vararg.is_some() {
            continue;
        }
        let proto_params = logical_param_names(proto_fn);
        let impl_params = logical_param_names(impl_fn);
        if proto_params != impl_params {
            push_signature_diag(
                protocol_name,
                rhs_class_name,
                member,
                &format!(
                    "parameter names {impl_params:?} do not match the protocol's {proto_params:?}"
                ),
                var,
                path,
                diagnostics,
            );
            continue;
        }

        // Names match — the parameter *calling conventions* must be compatible
        // too: a protocol positional-or-keyword parameter cannot be satisfied by
        // a keyword-only or positional-only one. This needs parameter-kind data
        // the resolver flattens away, so it consults the AST index.
        if let Some(reason) = ast_index.and_then(|index| {
            let proto_sig = index.method_signature(&protocol_class.name, member)?;
            let impl_sig = index.method_signature(rhs_class_name, member)?;
            proto_sig.calling_convention_mismatch(&impl_sig)
        }) {
            push_signature_diag(
                protocol_name,
                rhs_class_name,
                member,
                &reason,
                var,
                path,
                diagnostics,
            );
        }
    }
}
