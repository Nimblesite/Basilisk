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
use crate::diagnostic::{Diagnostic, ErrorCode, Severity};
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
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        // Build class lookup: name -> ClassInfo
        let class_map: HashMap<&str, &basilisk_resolver::ClassInfo> = module
            .classes
            .iter()
            .map(|cls| (cls.name.as_str(), cls))
            .collect();

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
                        diagnostics.push(Diagnostic {
                            code: CODE.clone(),
                            severity: Severity::Error,
                            message: format!(
                                "Cannot assign `{rhs_class_name}()` to type `{ann_name}`: \
                                 `{ann_name}` is not a protocol and does not support structural subtyping"
                            ),
                            span: var.name_span,
                            path: path.to_owned(),
                            help: Some(format!(
                                "`{ann_name}` inherits from a protocol but does not include \
                                 `Protocol` in its bases, so it is a concrete class; \
                                 `{rhs_class_name}` is not a subclass of `{ann_name}`"
                            )),
                            note: Some(
                                "Without `Protocol` in the base class list, a class that \
                                 inherits from a protocol is downgraded to a regular ABC \
                                 that cannot be used with structural subtyping"
                                    .to_owned(),
                            ),
                        });
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

/// Emit a protocol conformance error diagnostic.
fn emit_protocol_error(
    protocol_name: &str,
    rhs_class_name: &str,
    detail: &str,
    var: &basilisk_resolver::VariableInfo,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Class `{rhs_class_name}` is incompatible with protocol `{protocol_name}`: {detail}",
        ),
        span: var.name_span,
        path: path.to_owned(),
        help: None,
        note: Some(
            "Protocol classes use structural subtyping: the assigned class must \
             implement all members declared by the protocol"
                .to_owned(),
        ),
    });
}

/// Check if a method has a specific decorator.
fn has_decorator(cls: &basilisk_resolver::ClassInfo, method: &str, dec: &str) -> bool {
    cls.method_decorators
        .iter()
        .any(|(n, decs)| n == method && decs.iter().any(|d| d == dec))
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
    module: &ResolvedModule,
    var: &basilisk_resolver::VariableInfo,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let required_methods = collect_protocol_required_methods(protocol_class, class_map);
    let required_attrs: Vec<&str> = protocol_class
        .attributes
        .iter()
        .map(|a| a.name.as_str())
        .collect();

    let rhs_methods: Vec<&str> = class_methods
        .get(rhs_class_name)
        .cloned()
        .unwrap_or_default();

    let rhs_class = class_map.get(rhs_class_name);
    let rhs_attributes: Vec<&str> = rhs_class
        .map(|cls| cls.attributes.iter().map(|a| a.name.as_str()).collect())
        .unwrap_or_default();

    // Missing methods.
    let missing_methods: Vec<&str> = required_methods
        .iter()
        .filter(|m| !rhs_methods.contains(&m.as_str()) && !rhs_attributes.contains(&m.as_str()))
        .map(String::as_str)
        .collect();

    // Missing attributes — skip if RHS has __init__ (likely sets attrs via self.x).
    let rhs_has_init = rhs_methods.contains(&"__init__");
    let missing_attrs: Vec<&str> = if rhs_has_init {
        Vec::new()
    } else {
        required_attrs
            .iter()
            .filter(|a| !rhs_attributes.contains(a) && !rhs_methods.contains(a))
            .copied()
            .collect()
    };

    let all_missing: Vec<&str> = missing_methods
        .iter()
        .chain(missing_attrs.iter())
        .copied()
        .collect();

    if !all_missing.is_empty() {
        let list = all_missing.join("`, `");
        emit_protocol_error(
            protocol_name,
            rhs_class_name,
            &format!(
                "missing member{} `{list}`",
                if all_missing.len() == 1 { "" } else { "s" }
            ),
            var,
            path,
            diagnostics,
        );
        return;
    }

    // Check attribute compatibility.
    let Some(rhs_cls) = rhs_class else { return };
    check_attr_compat(
        protocol_name,
        protocol_class,
        rhs_class_name,
        rhs_cls,
        &rhs_methods,
        &module.source,
        var,
        path,
        diagnostics,
    );

    // Check method signature compatibility.
    check_method_compat(
        protocol_name,
        protocol_class,
        rhs_class_name,
        rhs_cls,
        module,
        var,
        path,
        diagnostics,
    );

    // Check mutability (property setters, frozen dataclass, NamedTuple).
    check_mutability(
        protocol_name,
        protocol_class,
        rhs_class_name,
        rhs_cls,
        var,
        path,
        diagnostics,
    );
}

/// Check protocol attribute compatibility (`ClassVar` vs instance, property vs method).
#[expect(
    clippy::too_many_arguments,
    reason = "attribute compat check needs full context"
)]
fn check_attr_compat(
    proto_name: &str,
    proto_cls: &basilisk_resolver::ClassInfo,
    rhs_name: &str,
    rhs_cls: &basilisk_resolver::ClassInfo,
    rhs_methods: &[&str],
    source: &str,
    var: &basilisk_resolver::VariableInfo,
    path: &str,
    diags: &mut Vec<Diagnostic>,
) {
    for proto_attr in &proto_cls.attributes {
        let ann = proto_attr
            .annotation_span
            .and_then(|sp| crate::span_util::slice_span(source, sp))
            .unwrap_or("");
        let proto_is_classvar = ann.trim().starts_with("ClassVar[");
        let proto_is_property = has_decorator(proto_cls, &proto_attr.name, "property");

        let rhs_attr = rhs_cls
            .attributes
            .iter()
            .find(|a| a.name == proto_attr.name);

        if let Some(rhs_a) = rhs_attr {
            if proto_is_classvar {
                let rhs_ann = rhs_a
                    .annotation_span
                    .and_then(|sp| crate::span_util::slice_span(source, sp))
                    .unwrap_or("");
                if !rhs_ann.trim().starts_with("ClassVar[") {
                    emit_protocol_error(
                        proto_name,
                        rhs_name,
                        &format!(
                            "attribute `{}` must be a `ClassVar` to match protocol",
                            proto_attr.name
                        ),
                        var,
                        path,
                        diags,
                    );
                }
            } else if !proto_is_property {
                // Protocol requires an instance variable — RHS must not be ClassVar.
                let rhs_ann = rhs_a
                    .annotation_span
                    .and_then(|sp| crate::span_util::slice_span(source, sp))
                    .unwrap_or("");
                if rhs_ann.trim().starts_with("ClassVar[") {
                    emit_protocol_error(
                        proto_name,
                        rhs_name,
                        &format!(
                            "attribute `{}` must be an instance variable, not `ClassVar`",
                            proto_attr.name
                        ),
                        var,
                        path,
                        diags,
                    );
                }
            }
        } else if rhs_methods.contains(&proto_attr.name.as_str()) {
            // RHS has method, protocol wants property or attribute.
            if proto_is_property && !has_decorator(rhs_cls, &proto_attr.name, "property") {
                emit_protocol_error(
                    proto_name,
                    rhs_name,
                    &format!(
                        "`{}` must be a property, not a plain method",
                        proto_attr.name
                    ),
                    var,
                    path,
                    diags,
                );
            }
        }
    }
}

/// Check protocol method signature compatibility.
#[expect(
    clippy::too_many_arguments,
    reason = "method compat check needs full context"
)]
fn check_method_compat(
    proto_name: &str,
    proto_cls: &basilisk_resolver::ClassInfo,
    rhs_name: &str,
    rhs_cls: &basilisk_resolver::ClassInfo,
    module: &ResolvedModule,
    var: &basilisk_resolver::VariableInfo,
    path: &str,
    diags: &mut Vec<Diagnostic>,
) {
    let proto_methods: Vec<&basilisk_resolver::FunctionInfo> = module
        .functions
        .iter()
        .filter(|f| f.class_name.as_deref() == Some(&proto_cls.name))
        .collect();

    for proto_fn in &proto_methods {
        if proto_fn.name.starts_with('_') {
            continue;
        }

        let rhs_fn = module
            .functions
            .iter()
            .find(|f| f.class_name.as_deref() == Some(rhs_name) && f.name == proto_fn.name);
        let Some(rhs_method) = rhs_fn else { continue };

        let proto_params: Vec<&str> = proto_fn
            .parameters
            .iter()
            .filter(|p| p.name != "self" && p.name != "cls")
            .map(|p| p.name.as_str())
            .collect();

        let rhs_is_static = has_decorator(rhs_cls, &rhs_method.name, "staticmethod");
        let rhs_params: Vec<&str> = rhs_method
            .parameters
            .iter()
            .filter(|p| {
                if rhs_is_static {
                    true
                } else {
                    p.name != "self" && p.name != "cls"
                }
            })
            .map(|p| p.name.as_str())
            .collect();

        // Staticmethod with `self` parameter is wrong.
        if rhs_is_static
            && rhs_method
                .parameters
                .first()
                .is_some_and(|p| p.name == "self")
        {
            emit_protocol_error(
                proto_name,
                rhs_name,
                &format!(
                    "method `{}`: static method should not have `self` parameter",
                    proto_fn.name
                ),
                var,
                path,
                diags,
            );
            continue;
        }

        // Check parameter names.
        if proto_params.len() == rhs_params.len() {
            for (pp, rp) in proto_params.iter().zip(rhs_params.iter()) {
                if pp != rp {
                    emit_protocol_error(
                        proto_name,
                        rhs_name,
                        &format!(
                            "method `{}`: parameter `{rp}` should be `{pp}`",
                            proto_fn.name
                        ),
                        var,
                        path,
                        diags,
                    );
                    break;
                }
            }
        }

        // Check positional-only violation: protocol has standard params but RHS has them as
        // keyword-only (via `*` separator) or positional-only (via `/`).
        if proto_fn.vararg.is_none() && rhs_method.vararg.is_some() && rhs_method.kwarg.is_none() {
            emit_protocol_error(proto_name, rhs_name, &format!(
                "method `{}`: keyword-only parameters not compatible with protocol's positional params",
                proto_fn.name
            ), var, path, diags);
        }
    }
}

/// Check protocol mutability requirements (property setters, `NamedTuple`, frozen dataclass).
fn check_mutability(
    proto_name: &str,
    proto_cls: &basilisk_resolver::ClassInfo,
    rhs_name: &str,
    rhs_cls: &basilisk_resolver::ClassInfo,
    var: &basilisk_resolver::VariableInfo,
    path: &str,
    diags: &mut Vec<Diagnostic>,
) {
    let is_named_tuple = rhs_cls.bases.iter().any(|b| b == "NamedTuple");
    let is_frozen_dc = rhs_cls.is_dataclass && rhs_cls.is_dataclass_frozen;

    // Check if protocol has property setters.
    for (method_name, decorators) in &proto_cls.method_decorators {
        let is_setter = decorators.iter().any(|d| d.contains("setter"));
        if !is_setter {
            continue;
        }

        if is_named_tuple {
            emit_protocol_error(
                proto_name,
                rhs_name,
                &format!(
                    "`{method_name}` requires a writable property but NamedTuple is immutable"
                ),
                var,
                path,
                diags,
            );
        } else if is_frozen_dc {
            emit_protocol_error(
                proto_name,
                rhs_name,
                &format!(
                "`{method_name}` requires a writable property but frozen dataclass is immutable"
            ),
                var,
                path,
                diags,
            );
        } else {
            // Check for read-only property (getter without setter).
            let has_property = has_decorator(rhs_cls, method_name, "property");
            let has_setter = rhs_cls
                .method_decorators
                .iter()
                .any(|(n, decs)| n == method_name && decs.iter().any(|d| d.contains("setter")));
            if has_property && !has_setter {
                emit_protocol_error(
                    proto_name,
                    rhs_name,
                    &format!(
                    "`{method_name}` requires a writable property but implementation is read-only"
                ),
                    var,
                    path,
                    diags,
                );
            }
        }
    }
}
