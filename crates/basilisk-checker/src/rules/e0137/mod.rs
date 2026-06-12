//! BSK-E0137: Generic protocol violations.
//!
//! Detects violations related to generic protocol usage:
//!
//! 1. **Protocol[T] combined with Generic[T]**: The `Protocol[T, S, ...]` shorthand
//!    is already equivalent to `Protocol, Generic[T, S, ...]`. It is an error to
//!    combine the shorthand with an explicit `Generic[...]` base.
//!
//! 2. **Incompatible generic protocol assignment**: When a module-level variable
//!    is annotated with a concrete generic protocol specialisation like
//!    `Proto[int, str]` and the RHS is a concrete class, the concrete class's
//!    method signatures must be compatible with the substituted type arguments.
//!
//! 3. **Self-typed protocol method incompatibility**: When a protocol declares
//!    methods using a `self: T` annotation (making the return type depend on the
//!    concrete receiver), concrete classes that implement those methods with
//!    incompatible signatures are flagged.
//!
//! ```python
//! from typing import Generic, Protocol, TypeVar
//!
//! T_co = TypeVar("T_co", covariant=True)
//!
//! class Proto2(Protocol[T_co], Generic[T_co]):  # E — shorthand + Generic
//!     ...
//! ```
//!
//! PEP 544: <https://typing.readthedocs.io/en/latest/spec/protocol.html#generic-protocols>

mod helpers;

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, FunctionInfo, ResolvedModule};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;
use crate::rules::shared::parse_subscript_annotation;
use helpers::{
    check_self_typed_method_incompatibility, check_typevar_methods_consistency,
    extract_constructor_name, find_method_mismatch, get_self_typevar_name, method_has_typed_self,
};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0137",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0137",
};

/// Emits BSK-E0137 for generic protocol violations.
pub(crate) struct GenericProtocolViolation;

impl Rule for GenericProtocolViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let source = &module.source;
        let path = &module.path;

        // Build a lookup map from class name to ClassInfo.
        let class_map = super::shared::class_name_map(&module.classes);

        // Build a map from class name -> list of methods.
        let class_methods: HashMap<&str, Vec<&FunctionInfo>> = module
            .classes
            .iter()
            .map(|cls| {
                let methods: Vec<&FunctionInfo> = module
                    .functions
                    .iter()
                    .filter(|f| f.class_name.as_deref() == Some(cls.name.as_str()))
                    .collect();
                (cls.name.as_str(), methods)
            })
            .collect();

        // Build TypeVar variance info.
        let typevar_info: HashMap<&str, bool> = module
            .typevar_calls
            .iter()
            .map(|tv| (tv.name.as_str(), tv.is_covariant || tv.is_contravariant))
            .collect();

        // --- Check 1: Protocol[T] combined with Generic[T] ---
        check_protocol_generic_combined(module, path, diagnostics);

        // --- Check 2: Generic protocol assignment type arg mismatch ---
        check_generic_protocol_assignments(
            module,
            source,
            path,
            &class_map,
            &class_methods,
            &typevar_info,
            diagnostics,
        );

        // --- Check 3: Self-typed protocol method incompatibility ---
        check_self_typed_protocol_assignments(
            module,
            source,
            path,
            &class_map,
            &class_methods,
            diagnostics,
        );
    }
}

// ---------------------------------------------------------------------------
// Check 1: Protocol[T] shorthand combined with Generic[T]
// ---------------------------------------------------------------------------

/// Detect classes that inherit from both `Protocol[T]` (subscript) and `Generic[T]`.
///
/// Per PEP 544, `Protocol[T, S, ...]` is a shorthand for `Protocol, Generic[T, S, ...]`.
/// Combining both is a redundant and invalid form.
fn check_protocol_generic_combined(
    module: &ResolvedModule,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for class in &module.classes {
        // Only check classes that have Protocol as a base (either plain or subscripted).
        let has_protocol_in_bases = class.bases.iter().any(|b| b == "Protocol")
            || class
                .base_subscripts
                .iter()
                .any(|bs| bs.base_name == "Protocol");
        if !has_protocol_in_bases {
            continue;
        }

        // Check if any base subscript uses Protocol[T] (subscript form).
        let protocol_is_subscripted = class
            .base_subscripts
            .iter()
            .any(|bs| bs.base_name == "Protocol");

        if !protocol_is_subscripted {
            continue;
        }

        // Check if Generic also appears as a base (subscript or plain).
        let has_generic_base = class.bases.iter().any(|b| b == "Generic")
            || class
                .base_subscripts
                .iter()
                .any(|bs| bs.base_name == "Generic");

        if has_generic_base {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Protocol class `{}` combines `Protocol[T]` shorthand with explicit `Generic[T]` base",
                    class.name
                ),
                class.name_span,
                path,
                Some(
                    "Remove the explicit `Generic[T]` base; `Protocol[T]` already implies \
                     `Generic[T]`"
                        .to_owned(),
                ),
                Some(
                    "PEP 544: `Protocol[T, S, ...]` is shorthand for `Protocol, Generic[T, S, ...]`. \
                     Combining the two is redundant and invalid."
                        .to_owned(),
                ),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Check 2: Generic protocol assignment with wrong type arguments
// ---------------------------------------------------------------------------

/// Check module-level annotated assignments where:
/// - The annotation is a subscripted generic protocol (`Proto[A, B]`)
/// - The RHS is a concrete class constructor call
/// - The concrete class's methods are incompatible with the substituted type args.
fn check_generic_protocol_assignments(
    module: &ResolvedModule,
    source: &str,
    path: &str,
    class_map: &HashMap<&str, &ClassInfo>,
    class_methods: &HashMap<&str, Vec<&FunctionInfo>>,
    typevar_info: &HashMap<&str, bool>,
    diagnostics: &mut Vec<Diagnostic>,
) {
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

        let Some(ann_text) = slice_span(source, ann_span) else {
            continue;
        };
        let ann_text = ann_text.trim();

        // Only process subscripted annotations like `Proto[T1, T2]`.
        let Some((proto_name, type_args)) = parse_subscript_annotation(ann_text) else {
            continue;
        };

        // Look up the protocol class.
        let Some(proto_class) = class_map.get(proto_name) else {
            continue;
        };

        // Only process Protocol classes.
        let is_protocol = proto_class.bases.iter().any(|b| b == "Protocol")
            || proto_class
                .base_subscripts
                .iter()
                .any(|bs| bs.base_name == "Protocol");
        if !is_protocol {
            continue;
        }

        // Get the protocol's type parameters in order.
        let proto_type_params: Vec<&str> =
            basilisk_resolver::collect_names(&proto_class.generic_params);

        if proto_type_params.is_empty() || type_args.len() != proto_type_params.len() {
            continue;
        }

        // Build the substitution map: TypeVar name -> concrete type.
        let substitution: HashMap<&str, &str> = proto_type_params
            .iter()
            .zip(type_args.iter())
            .map(|(&tv, arg)| (tv, arg.as_str()))
            .collect();

        // Extract RHS class name.
        let Some(rhs_text) = slice_span(source, rhs_span) else {
            continue;
        };
        let rhs_text = rhs_text.trim();
        let Some(rhs_class_name) = extract_constructor_name(rhs_text) else {
            continue;
        };

        // If same class, skip.
        if rhs_class_name == proto_name {
            continue;
        }

        // Check that the concrete class's methods are compatible with the substituted types.
        let Some(rhs_methods) = class_methods.get(rhs_class_name) else {
            continue;
        };

        // For each method in the protocol, check the concrete class's signature.
        let Some(proto_methods) = class_methods.get(proto_name) else {
            continue;
        };

        if let Some(mismatch_details) = find_method_mismatch(
            proto_methods,
            rhs_methods,
            source,
            &substitution,
            typevar_info,
        ) {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Class `{rhs_class_name}` is incompatible with `{ann_text}`: {mismatch_details}"
                ),
                var.name_span,
                path,
                Some(format!(
                    "The concrete class `{rhs_class_name}` does not satisfy the \
                     type constraints of `{ann_text}`"
                )),
                Some(
                    "Generic protocols require that the implementing class's method signatures \
                     are compatible with the substituted type arguments"
                        .to_owned(),
                ),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Check 3: Self-typed protocol method incompatibility
// ---------------------------------------------------------------------------

/// Check module-level annotated assignments for protocols that use `self: T`
/// typed parameters, where the concrete class doesn't correctly implement them.
///
/// When a protocol declares `def f(self: T) -> T` and a concrete class is
/// assigned to a variable of that protocol type, the concrete class must
/// implement `f` with a compatible self-typed signature.
fn check_self_typed_protocol_assignments(
    module: &ResolvedModule,
    source: &str,
    path: &str,
    class_map: &HashMap<&str, &ClassInfo>,
    class_methods: &HashMap<&str, Vec<&FunctionInfo>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
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

        let Some(ann_text) = slice_span(source, ann_span) else {
            continue;
        };
        let ann_name = ann_text.trim();

        // Skip subscripted annotations (handled by Check 2).
        if ann_name.contains('[') {
            continue;
        }

        // Look up the protocol class — must be a Protocol.
        let Some(proto_class) = class_map.get(ann_name) else {
            continue;
        };
        if !is_protocol_class(proto_class) {
            continue;
        }

        // Extract RHS class name.
        let Some(rhs_text) = slice_span(source, rhs_span) else {
            continue;
        };
        let Some(rhs_class_name) = extract_constructor_name(rhs_text.trim()) else {
            continue;
        };
        if rhs_class_name == ann_name {
            continue;
        }

        let Some(proto_methods) = class_methods.get(ann_name) else {
            continue;
        };
        let Some(concrete_methods) = class_methods.get(rhs_class_name) else {
            continue;
        };

        check_self_typed_methods(
            &SelfTypedCtx {
                proto_methods,
                concrete_methods,
                source,
                ann_name,
                rhs_class_name,
                path,
                name_span: var.name_span,
            },
            diagnostics,
        );
    }
}

/// Returns `true` if a class is a Protocol.
fn is_protocol_class(cls: &ClassInfo) -> bool {
    cls.bases.iter().any(|b| b == "Protocol")
        || cls
            .base_subscripts
            .iter()
            .any(|bs| bs.base_name == "Protocol")
}

/// Context for checking a single self-typed protocol assignment.
struct SelfTypedCtx<'a> {
    proto_methods: &'a [&'a FunctionInfo],
    concrete_methods: &'a [&'a FunctionInfo],
    source: &'a str,
    ann_name: &'a str,
    rhs_class_name: &'a str,
    path: &'a str,
    name_span: basilisk_resolver::Span,
}

/// Check self-typed and TypeVar-referencing methods for a single assignment.
fn check_self_typed_methods(ctx: &SelfTypedCtx<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let self_typed: Vec<&FunctionInfo> = ctx
        .proto_methods
        .iter()
        .filter(|m| method_has_typed_self(m))
        .copied()
        .collect();

    if self_typed.is_empty() {
        return;
    }

    let tv_name = self_typed
        .iter()
        .find_map(|m| get_self_typevar_name(m, ctx.source));

    // Check each self-typed protocol method.
    for proto_method in &self_typed {
        let Some(concrete_method) = ctx
            .concrete_methods
            .iter()
            .find(|m| m.name == proto_method.name)
        else {
            continue;
        };

        let Some(tv) = get_self_typevar_name(proto_method, ctx.source) else {
            continue;
        };

        if let Some(detail) =
            check_self_typed_method_incompatibility(proto_method, concrete_method, &tv, ctx.source)
        {
            emit_self_typed_diagnostic(diagnostics, &detail, ctx, &proto_method.name);
            return;
        }
    }

    // Check non-self-typed methods that reference the self-TypeVar.
    if let Some(tv) = &tv_name {
        if let Some(detail) = check_typevar_methods_consistency(
            ctx.proto_methods,
            ctx.concrete_methods,
            tv,
            ctx.source,
        ) {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Class `{}` is incompatible with protocol `{}`: {detail}",
                    ctx.rhs_class_name, ctx.ann_name
                ),
                ctx.name_span,
                ctx.path,
                Some(format!(
                    "All methods in `{}` referencing TypeVar `{tv}` \
                     must use consistent types matching the protocol `{}`",
                    ctx.rhs_class_name, ctx.ann_name
                )),
                Some(
                    "PEP 544: when a protocol binds `self: T`, all uses of `T` in \
                     other methods must be satisfied by the implementing class"
                        .to_owned(),
                ),
            ));
        }
    }
}

/// Emit a diagnostic for a self-typed method incompatibility.
fn emit_self_typed_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    detail: &str,
    ctx: &SelfTypedCtx<'_>,
    method_name: &str,
) {
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Class `{}` is incompatible with protocol `{}`: {detail}",
            ctx.rhs_class_name, ctx.ann_name
        ),
        ctx.name_span,
        ctx.path,
        Some(format!(
            "Method `{method_name}` in `{}` must match the self-typed \
             signature declared in protocol `{}`",
            ctx.rhs_class_name, ctx.ann_name
        )),
        Some(
            "PEP 544: methods with `self: T` annotations in protocols require \
             that implementing classes use compatible self-typed or `Self` signatures"
                .to_owned(),
        ),
    ));
}
