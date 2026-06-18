//! Implements [BSK-E0121] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! BSK-E0121: Protocol conformance violation in annotated assignment.
//!
//! Detects two kinds of errors in annotated assignments at module level:
//!
//! 1. **Missing protocol members**: the annotation names a Protocol class and the
//!    RHS constructs a class that does not implement all required methods.
//!
//! 2. **Non-protocol structural assignment**: the annotation names a class that
//!    inherits from a Protocol but does *not* itself include `Protocol` in its
//!    bases (i.e. it is a concrete/abstract class, not a protocol).  In this case
//!    structural subtyping does not apply and only nominal subclasses are allowed.
//!
//! ```python
//! class P(Protocol):
//!     def method(self) -> None: ...
//!
//! class NotP(P):           # Note: no Protocol — this is a concrete class
//!     def method(self) -> None: pass
//!
//! class C:
//!     pass
//!
//! x: P = C()       # E — C does not implement `method` (case 1)
//! y: NotP = C()    # E — NotP is not a Protocol, no structural subtyping (case 2)
//! ```

use std::collections::HashMap;

use basilisk_resolver::ResolvedModule;

use super::Rule;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0121",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0121",
};

/// Well-known stdlib protocol classes and their required dunder methods.
fn known_protocol_methods(name: &str) -> Option<&'static [&'static str]> {
    match name {
        "Sized" => Some(&["__len__"]),
        "Hashable" => Some(&["__hash__"]),
        "Iterable" => Some(&["__iter__"]),
        "Iterator" => Some(&["__iter__", "__next__"]),
        "Reversible" => Some(&["__reversed__"]),
        "Container" => Some(&["__contains__"]),
        "Collection" => Some(&["__contains__", "__iter__", "__len__"]),
        "Callable" => Some(&["__call__"]),
        "Awaitable" => Some(&["__await__"]),
        "ContextManager" => Some(&["__enter__", "__exit__"]),
        "AsyncContextManager" => Some(&["__aenter__", "__aexit__"]),
        "SupportsInt" => Some(&["__int__"]),
        "SupportsFloat" => Some(&["__float__"]),
        "SupportsComplex" => Some(&["__complex__"]),
        "SupportsBytes" => Some(&["__bytes__"]),
        "SupportsAbs" => Some(&["__abs__"]),
        "SupportsRound" => Some(&["__round__"]),
        "Buffer" => Some(&["__buffer__"]),
        _ => None,
    }
}

/// Emits BSK-E0121 for protocol conformance violations in annotated assignments.
pub(crate) struct ProtocolAssignmentConformance;

impl Rule for ProtocolAssignmentConformance {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let source = &module.source;
        let path = &module.path;

        // Build class lookup: name -> ClassInfo
        let class_map: HashMap<&str, &basilisk_resolver::ClassInfo> =
            basilisk_resolver::name_lookup(&module.classes);

        // Build class method lookup: class_name -> set of method names
        let class_methods: HashMap<&str, Vec<&str>> = module
            .classes
            .iter()
            .map(|cls| {
                let methods: Vec<&str> = cls.method_names.iter().map(String::as_str).collect();
                (cls.name.as_str(), methods)
            })
            .collect();

        // Check each module-level variable assignment.
        for var in &module.module_vars {
            if !var.has_annotation {
                continue;
            }

            let Some(ann_span) = var.annotation_span else {
                continue;
            };
            let Some(rhs_span) = var.rhs_span else {
                continue;
            };

            // Extract annotation text (the type name).
            let Some(ann_text) = slice_span(source, ann_span) else {
                continue;
            };
            let ann_name = ann_text.trim();

            // Extract RHS text and check if it's a constructor call `ClassName()`.
            let Some(rhs_text) = slice_span(source, rhs_span) else {
                continue;
            };
            let rhs_trimmed = rhs_text.trim();

            // Must be a simple call: `Name(...)`.
            let Some(paren_pos) = rhs_trimmed.find('(') else {
                continue;
            };
            let rhs_class_name = rhs_trimmed[..paren_pos].trim();
            if rhs_class_name.is_empty()
                || !rhs_class_name
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_')
            {
                continue;
            }

            // Skip if annotation and RHS are the same class, or if RHS is a subclass
            // of the annotation type.
            if ann_name == rhs_class_name {
                continue;
            }

            // Check if the RHS class is a nominal subclass of the annotation class.
            if is_nominal_subclass(rhs_class_name, ann_name, &class_map) {
                continue;
            }

            // Now determine what the annotation class is:
            if let Some(ann_class) = class_map.get(ann_name) {
                let ann_is_protocol = ann_class.bases.iter().any(|b| b == "Protocol");

                if ann_is_protocol {
                    // Case 1: annotation is a Protocol. Check structural conformance.
                    check_protocol_conformance(
                        ann_name,
                        ann_class,
                        rhs_class_name,
                        &class_map,
                        &class_methods,
                        module,
                        var,
                        path,
                        diagnostics,
                    );
                } else {
                    // Case 2: annotation inherits from a Protocol but is not itself
                    // a Protocol. No structural subtyping — flag it.
                    let inherits_protocol = class_inherits_protocol(ann_name, &class_map);
                    if inherits_protocol {
                        diagnostics.push(error_diagnostic_owned(
                            CODE.clone(),
                            format!(
                                "Cannot assign `{rhs_class_name}()` to type `{ann_name}`: \
                                 `{ann_name}` is not a protocol and does not support structural subtyping"
                            ),
                            var.name_span,
                            path,
                            Some(format!(
                                "`{ann_name}` inherits from a protocol but does not include \
                                 `Protocol` in its bases, so it is a concrete class; \
                                 `{rhs_class_name}` is not a subclass of `{ann_name}`"
                            )),
                            Some(
                                "Without `Protocol` in the base class list, a class that \
                                 inherits from a protocol is downgraded to a regular ABC \
                                 that cannot be used with structural subtyping"
                                    .to_owned(),
                            ),
                        ));
                    }
                }
            }
        }
    }
}

/// Check if `rhs_class` is a nominal subclass of `target_class`.
fn is_nominal_subclass(
    rhs_class: &str,
    target_class: &str,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
) -> bool {
    let Some(rhs_info) = class_map.get(rhs_class) else {
        return false;
    };

    // Direct base check.
    for base in &rhs_info.bases {
        if base == target_class {
            return true;
        }
        // Recurse through base classes.
        if is_nominal_subclass(base, target_class, class_map) {
            return true;
        }
    }

    false
}

/// Check if a class (transitively) inherits from a Protocol class.
fn class_inherits_protocol(
    class_name: &str,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
) -> bool {
    let Some(cls) = class_map.get(class_name) else {
        return false;
    };

    for base in &cls.bases {
        if base == "Protocol" {
            return true;
        }
        // Check if the base is a protocol class.
        if let Some(base_cls) = class_map.get(base.as_str()) {
            if base_cls.bases.iter().any(|b| b == "Protocol") {
                return true;
            }
        }
        // Also check known stdlib protocols.
        if known_protocol_methods(base).is_some() {
            return true;
        }
    }

    false
}

/// Collect all methods required by a protocol, including inherited ones.
fn collect_protocol_required_methods(
    protocol_class: &basilisk_resolver::ClassInfo,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
) -> Vec<String> {
    let mut methods: Vec<String> = protocol_class.method_names.clone();

    // Add methods from base protocol classes.
    for base in &protocol_class.bases {
        if base == "Protocol" || base == "object" || base == "Generic" {
            continue;
        }

        // Check known stdlib protocols first.
        if let Some(required) = known_protocol_methods(base) {
            for method in required {
                if !methods.iter().any(|m| m == method) {
                    methods.push((*method).to_owned());
                }
            }
            continue;
        }

        // Check locally defined protocol classes.
        if let Some(base_cls) = class_map.get(base.as_str()) {
            if base_cls.bases.iter().any(|b| b == "Protocol") {
                for method in &base_cls.method_names {
                    if !methods.contains(method) {
                        methods.push(method.clone());
                    }
                }
            }
        }
    }

    methods
}

/// Check if a concrete class satisfies a protocol's structural requirements.
#[expect(
    clippy::too_many_arguments,
    reason = "protocol conformance check requires full context"
)]
fn check_protocol_conformance(
    protocol_name: &str,
    protocol_class: &basilisk_resolver::ClassInfo,
    rhs_class_name: &str,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    class_methods: &HashMap<&str, Vec<&str>>,
    _module: &ResolvedModule,
    var: &basilisk_resolver::VariableInfo,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let required_members = collect_protocol_required_methods(protocol_class, class_map);

    // Get the RHS class methods.
    let rhs_methods: Vec<&str> = class_methods
        .get(rhs_class_name)
        .cloned()
        .unwrap_or_default();

    // Get the RHS class attribute names. Class attributes, dataclass fields,
    // and NamedTuple fields all satisfy protocol property requirements.
    let rhs_attributes: Vec<&str> = class_map
        .get(rhs_class_name)
        .map(|cls| cls.attributes.iter().map(|a| a.name.as_str()).collect())
        .unwrap_or_default();

    // Find missing members: a protocol member is satisfied by either a method
    // or an attribute with the same name.
    let missing: Vec<&str> = required_members
        .iter()
        .filter(|m| {
            let name = m.as_str();
            !rhs_methods.contains(&name) && !rhs_attributes.contains(&name)
        })
        .map(String::as_str)
        .collect();

    if !missing.is_empty() {
        let missing_list = missing.join("`, `");
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Class `{rhs_class_name}` is incompatible with protocol `{protocol_name}`: \
                 missing method{} `{missing_list}`",
                if missing.len() == 1 { "" } else { "s" }
            ),
            var.name_span,
            path,
            Some(format!(
                "Add the missing method{} to `{rhs_class_name}` or use a compatible class",
                if missing.len() == 1 { "" } else { "s" }
            )),
            Some(
                "Protocol classes use structural subtyping: the assigned class must \
                 implement all methods declared by the protocol"
                    .to_owned(),
            ),
        ));
    }

    // Beyond name presence: a read-write (settable) protocol property requires a
    // settable implementation member.
    check_readwrite_property_conformance(
        protocol_name,
        protocol_class,
        rhs_class_name,
        class_map,
        var,
        path,
        diagnostics,
    );
}

/// Names of a protocol's read-write properties: a method decorated `@property`
/// that also has a sibling `@<name>.setter` (recorded as a `"setter"` decorator).
fn readwrite_property_members(cls: &basilisk_resolver::ClassInfo) -> Vec<&str> {
    let mut members: Vec<&str> = Vec::new();
    for (name, decs) in &cls.method_decorators {
        let is_property = decs.iter().any(|d| d == "property");
        let has_setter = cls
            .method_decorators
            .iter()
            .any(|(n, ds)| n == name && ds.iter().any(|d| d == "setter"));
        if is_property && has_setter && !members.contains(&name.as_str()) {
            members.push(name.as_str());
        }
    }
    members
}

/// If `impl_cls` provides `member` but in a form that cannot be written to,
/// return the human-readable reason; otherwise `None` (settable, or absent —
/// absence is reported separately as a missing member).
fn readwrite_property_violation(
    impl_cls: &basilisk_resolver::ClassInfo,
    member: &str,
) -> Option<&'static str> {
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
    let entries: Vec<&Vec<String>> = impl_cls
        .method_decorators
        .iter()
        .filter(|(n, _)| n == member)
        .map(|(_, ds)| ds)
        .collect();
    let is_property = entries.iter().any(|ds| ds.iter().any(|d| d == "property"));
    let has_setter = entries.iter().any(|ds| ds.iter().any(|d| d == "setter"));
    if is_property && !has_setter {
        return Some("a read-only property cannot satisfy a writable protocol member");
    }

    None
}

/// Check that read-write protocol properties are satisfied by settable members.
#[expect(
    clippy::too_many_arguments,
    reason = "protocol property conformance needs protocol, impl, class map and context"
)]
fn check_readwrite_property_conformance(
    protocol_name: &str,
    protocol_class: &basilisk_resolver::ClassInfo,
    rhs_class_name: &str,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    var: &basilisk_resolver::VariableInfo,
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
