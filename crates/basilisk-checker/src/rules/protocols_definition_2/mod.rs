//! Implements [`protocols_definition_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `protocols_definition_2`: Protocol conformance violation in annotated assignment.
//!
//! Detects errors in annotated assignments at module level:
//!
//! 1. **Missing protocol members**: the annotation names a Protocol class and the
//!    RHS constructs a class that does not implement all required methods.
//!
//! 2. **Non-protocol structural assignment**: the annotation names a class that
//!    inherits from a Protocol but does *not* itself include `Protocol` in its
//!    bases (i.e. it is a concrete/abstract class, not a protocol).  In this case
//!    structural subtyping does not apply and only nominal subclasses are allowed.
//!
//! 3. **Member-kind mismatch** (see [`conformance`]): a member is present but in
//!    an incompatible *form* — a read-write protocol property satisfied by a
//!    read-only/immutable member, or a writable protocol instance variable
//!    satisfied by a `ClassVar`, read-only property, or wrong-typed attribute.
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

use std::collections::{HashMap, HashSet};

use basilisk_resolver::ResolvedModule;

use super::Rule;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

mod ast_index;
mod call_args;
mod conformance;

use ast_index::AstIndex;
use conformance::{
    check_instance_var_conformance, check_method_signature_conformance,
    check_property_method_conformance, check_readwrite_property_conformance,
};

pub(super) const CODE: ErrorCode = ErrorCode {
    code: "protocols_definition_2",
    docs_url: "https://www.basilisk-python.dev/errors/protocols_definition_2",
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

/// Emits `protocols_definition_2` for protocol conformance violations in annotated assignments.
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

        // Parse the AST once for structural checks the resolver does not retain:
        // parameter kinds (method-signature conformance) and `self.<attr>`
        // assignments (instance-variable presence). Both are optional — if the
        // source fails to re-parse, those refinements are simply skipped.
        // The AST-dependent refinements (parameter kinds, `self.<attr>`
        // instance variables, protocol-typed call arguments) all require a
        // locally defined `Protocol` class. Skip the re-parse entirely when none
        // exists, keeping protocol-free files cheap.
        let has_protocol_class = module
            .classes
            .iter()
            .any(|cls| cls.bases.iter().any(|base| base == "Protocol"));
        let parsed = has_protocol_class
            .then(|| super::shared::parse_module(module))
            .flatten();
        let ast_index = parsed.as_ref().map(|p| AstIndex::build(&p.ast.body));
        let self_attrs: HashMap<&str, HashSet<String>> = parsed
            .as_ref()
            .map(|p| ast_index::self_attrs_by_class(&p.ast.body))
            .unwrap_or_default();

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
                        ConformanceArgs {
                            protocol_name: ann_name,
                            protocol_class: ann_class,
                            rhs_class_name,
                            class_map: &class_map,
                            class_methods: &class_methods,
                            module,
                            var,
                            path,
                            ast_index: ast_index.as_ref(),
                            rhs_self_attrs: self_attrs.get(rhs_class_name),
                        },
                        diagnostics,
                    );
                } else if class_inherits_protocol(ann_name, &class_map) {
                    // Case 2: annotation inherits from a Protocol but is not itself
                    // a Protocol. No structural subtyping — flag it.
                    report_non_protocol_assignment(
                        ann_name,
                        rhs_class_name,
                        var,
                        path,
                        diagnostics,
                    );
                }
            }
        }

        // Check protocol-typed function-call arguments (e.g. passing a list of
        // built-in literals where an `Iterable[SomeProtocol]` is expected).
        if let Some(parsed) = parsed.as_ref() {
            call_args::check_protocol_call_args(module, &parsed.ast.body, &class_map, diagnostics);
        }
    }
}

/// Report assignment to a non-protocol class that merely inherits from a
/// Protocol: structural subtyping does not apply, so only nominal subclasses fit.
fn report_non_protocol_assignment(
    ann_name: &str,
    rhs_class_name: &str,
    var: &basilisk_resolver::VariableInfo,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
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

/// Check if `rhs_class` is a nominal subclass of `target_class`.
fn is_nominal_subclass(
    rhs_class: &str,
    target_class: &str,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
) -> bool {
    class_map.get(rhs_class).copied().is_some_and(|rhs_info| {
        crate::rules::shared::any_base_name_matches(
            rhs_info,
            &|base| class_map.get(base).copied(),
            &|base| base == target_class,
        )
    })
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
pub(super) fn collect_protocol_required_methods(
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

/// Bundled context for one protocol-conformance check, threaded through the
/// member-presence and member-kind sub-checks.
#[derive(Clone, Copy)]
struct ConformanceArgs<'a> {
    protocol_name: &'a str,
    protocol_class: &'a basilisk_resolver::ClassInfo,
    rhs_class_name: &'a str,
    class_map: &'a HashMap<&'a str, &'a basilisk_resolver::ClassInfo>,
    class_methods: &'a HashMap<&'a str, Vec<&'a str>>,
    module: &'a ResolvedModule,
    var: &'a basilisk_resolver::VariableInfo,
    path: &'a str,
    /// AST index for parameter-kind aware method-signature comparison.
    ast_index: Option<&'a AstIndex<'a>>,
    /// Names assigned via `self.<attr>` in the RHS class's methods.
    rhs_self_attrs: Option<&'a HashSet<String>>,
}

/// Check if a concrete class satisfies a protocol's structural requirements.
#[expect(
    clippy::too_many_lines,
    reason = "orchestrates every protocol member-presence and member-kind sub-check"
)]
fn check_protocol_conformance(args: ConformanceArgs<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let ConformanceArgs {
        protocol_name,
        protocol_class,
        rhs_class_name,
        class_map,
        class_methods,
        module,
        var,
        path,
        ast_index,
        rhs_self_attrs,
    } = args;

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

    // A protocol's writable instance variables must also be present (as a class
    // attribute, a `self.<attr>` assignment, or a property). Absence is a
    // separate violation from the wrong-kind/wrong-type cases handled below.
    check_missing_instance_vars(
        protocol_name,
        protocol_class,
        rhs_class_name,
        &module.source,
        &rhs_methods,
        &rhs_attributes,
        rhs_self_attrs,
        var,
        path,
        diagnostics,
    );

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

    // A read-write protocol instance variable requires a writable, same-type
    // implementation attribute (not a `ClassVar`, read-only property, or
    // wrong-typed attribute).
    check_instance_var_conformance(
        protocol_name,
        protocol_class,
        rhs_class_name,
        class_map,
        &module.source,
        var,
        path,
        diagnostics,
    );

    // A protocol property member must not be satisfied by a plain method.
    check_property_method_conformance(
        protocol_name,
        protocol_class,
        rhs_class_name,
        class_map,
        var,
        path,
        diagnostics,
    );

    // A protocol method's signature (receiver kind, parameter names, and
    // parameter calling convention) must be compatible with the implementation.
    check_method_signature_conformance(
        protocol_name,
        protocol_class,
        rhs_class_name,
        module,
        ast_index,
        var,
        path,
        diagnostics,
    );
}

/// Report each writable protocol instance variable that the implementation does
/// not provide in any form: not a class-body attribute, not a `self.<attr>`
/// assignment, and not a (property) method of the same name.
#[expect(
    clippy::too_many_arguments,
    reason = "missing-instance-var check needs the full protocol/impl context"
)]
fn check_missing_instance_vars(
    protocol_name: &str,
    protocol_class: &basilisk_resolver::ClassInfo,
    rhs_class_name: &str,
    source: &str,
    rhs_methods: &[&str],
    rhs_attributes: &[&str],
    rhs_self_attrs: Option<&HashSet<String>>,
    var: &basilisk_resolver::VariableInfo,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for member in protocol_instance_var_names(protocol_class, source) {
        let provided = rhs_attributes.contains(&member)
            || rhs_methods.contains(&member)
            || rhs_self_attrs.is_some_and(|set| set.contains(member));
        if provided {
            continue;
        }
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Class `{rhs_class_name}` is incompatible with protocol `{protocol_name}`: \
                 missing instance variable `{member}`"
            ),
            var.name_span,
            path,
            Some(format!(
                "Declare `{member}` in `{rhs_class_name}` (as a class attribute, a \
                 `self.{member}` assignment, or a property)"
            )),
            Some(
                "Protocol instance variables must be provided by the implementation; a \
                 class that declares none of them does not satisfy the protocol"
                    .to_owned(),
            ),
        ));
    }
}

/// Names of a protocol's writable instance variables: class-body attributes
/// whose annotation is **not** `ClassVar` (those are covered by `classes_classvar`).
fn protocol_instance_var_names<'a>(
    protocol_class: &'a basilisk_resolver::ClassInfo,
    source: &str,
) -> Vec<&'a str> {
    protocol_class
        .attributes
        .iter()
        .filter(|attr| {
            attr.annotation_span
                .and_then(|sp| slice_span(source, sp))
                .map(str::trim)
                .is_some_and(|ann| !conformance::is_classvar_ann(ann))
        })
        .map(|attr| attr.name.as_str())
        .collect()
}
