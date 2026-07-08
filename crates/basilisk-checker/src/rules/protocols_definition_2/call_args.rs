//! Implements [`protocols_definition_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! Protocol conformance for function-call arguments (`protocols_definition_2`).
//!
//! When a module-level function declares a parameter typed as an iterable of a
//! `Protocol` (e.g. `things: Iterable[SupportsClose]`), passing a container
//! literal whose elements are built-in scalars that cannot satisfy that protocol
//! is a conformance violation:
//!
//! ```python
//! class SupportsClose(Protocol):
//!     def close(self) -> None: ...
//!
//! def close_all(things: Iterable[SupportsClose]) -> None: ...
//!
//! close_all([1])  # E — `int` has no `close` method
//! ```
//!
//! The analysis is intentionally conservative: it only fires for container
//! displays of numeric/`None` literals (whose member sets are small and fully
//! known), so a built-in element provably lacking a required protocol member can
//! never be a false positive.

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, FunctionInfo, ResolvedModule, Span};
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged as _;

use super::CODE;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic};
use crate::rules::shared::{infer_expr_literal_type, parse_subscript_annotation};

/// Generic container types whose sole type argument is the element type.
const ITERABLE_CONTAINERS: &[&str] = &[
    "Iterable",
    "Iterator",
    "Collection",
    "Container",
    "Sequence",
    "MutableSequence",
    "Reversible",
    "AbstractSet",
    "MutableSet",
    "list",
    "set",
    "frozenset",
    "tuple",
];

/// Check protocol-typed arguments at call sites in the module body.
pub(super) fn check_protocol_call_args(
    module: &ResolvedModule,
    body: &[Stmt],
    class_map: &HashMap<&str, &ClassInfo>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Index module-level functions (those without a containing class) by name.
    let functions: HashMap<&str, &FunctionInfo> = module
        .functions
        .iter()
        .filter(|func| func.class_name.is_none())
        .map(|func| (func.name.as_str(), func))
        .collect();
    if functions.is_empty() {
        return;
    }

    basilisk_resolver::walk_all_stmts(body, &mut |stmt| match stmt {
        Stmt::Expr(node) => scan_expr(
            &node.value,
            &functions,
            class_map,
            &module.path,
            diagnostics,
        ),
        Stmt::Assign(node) => {
            scan_expr(
                &node.value,
                &functions,
                class_map,
                &module.path,
                diagnostics,
            );
        }
        Stmt::AnnAssign(node) => {
            if let Some(value) = node.value.as_deref() {
                scan_expr(value, &functions, class_map, &module.path, diagnostics);
            }
        }
        Stmt::Return(node) => {
            if let Some(value) = node.value.as_deref() {
                scan_expr(value, &functions, class_map, &module.path, diagnostics);
            }
        }
        _ => {}
    });
}

/// Recursively check call expressions (including nested call arguments).
fn scan_expr(
    expr: &Expr,
    functions: &HashMap<&str, &FunctionInfo>,
    class_map: &HashMap<&str, &ClassInfo>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Expr::Call(call) = expr else { return };
    if let Expr::Name(name) = call.func.as_ref() {
        if let Some(func) = functions.get(name.id.as_str()) {
            validate_call(call, func, class_map, path, diagnostics);
        }
    }
    for arg in &call.arguments.args {
        scan_expr(arg, functions, class_map, path, diagnostics);
    }
}

/// Validate each positional argument of `call` against the matching parameter's
/// protocol-container annotation.
fn validate_call(
    call: &ruff_python_ast::ExprCall,
    func: &FunctionInfo,
    class_map: &HashMap<&str, &ClassInfo>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (arg, param) in call.arguments.args.iter().zip(&func.parameters) {
        let Some(ann) = param.annotation_text.as_deref() else {
            continue;
        };
        let Some(protocol) = protocol_element(ann, class_map) else {
            continue;
        };
        let Some(bad_type) = unsatisfying_element(arg, protocol) else {
            continue;
        };
        let range = call.range();
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Argument is incompatible with parameter `{}` of type `{ann}`: \
                 `{bad_type}` does not satisfy protocol `{}`",
                param.name, protocol.name
            ),
            Span {
                start: range.start().to_u32(),
                end: range.end().to_u32(),
            },
            path,
            Some(format!(
                "Pass elements that implement protocol `{}`",
                protocol.name
            )),
            Some(
                "A container of built-in scalars cannot satisfy an iterable of a Protocol \
                 when the scalar lacks a required protocol member"
                    .to_owned(),
            ),
        ));
    }
}

/// If `ann` is `Container[Protocol]` for a known iterable container and a locally
/// defined `Protocol` class, return that protocol's `ClassInfo`.
fn protocol_element<'a>(
    ann: &str,
    class_map: &HashMap<&str, &'a ClassInfo>,
) -> Option<&'a ClassInfo> {
    let (container, args) = parse_subscript_annotation(ann)?;
    if !ITERABLE_CONTAINERS.contains(&container) {
        return None;
    }
    let element = args.first()?.as_str();
    let class = class_map.get(element)?;
    class
        .bases
        .iter()
        .any(|base| base == "Protocol")
        .then_some(*class)
}

/// If `arg` is a container display with a built-in scalar element that provably
/// cannot satisfy `protocol`, return that scalar's type name.
fn unsatisfying_element(arg: &Expr, protocol: &ClassInfo) -> Option<&'static str> {
    let elements = match arg {
        Expr::List(list) => &list.elts,
        Expr::Set(set) => &set.elts,
        Expr::Tuple(tuple) => &tuple.elts,
        _ => return None,
    };
    elements
        .iter()
        .filter_map(infer_expr_literal_type)
        .find(|scalar| scalar_cannot_satisfy(scalar, protocol))
}

/// The public non-dunder members of each built-in scalar type we analyse.
///
/// These tables are intentionally exhaustive for the few scalar types covered so
/// that a required protocol member absent from the table is *definitely* absent
/// from the type — never a false positive. `str`/`bytes` are deliberately
/// excluded (their large method sets make confident exclusion impractical).
fn scalar_members(scalar: &str) -> Option<&'static [&'static str]> {
    match scalar {
        "int" | "bool" => Some(&[
            "bit_length",
            "bit_count",
            "to_bytes",
            "from_bytes",
            "as_integer_ratio",
            "conjugate",
            "numerator",
            "denominator",
            "real",
            "imag",
            "is_integer",
        ]),
        "float" => Some(&[
            "as_integer_ratio",
            "is_integer",
            "hex",
            "fromhex",
            "conjugate",
            "real",
            "imag",
        ]),
        "complex" => Some(&["conjugate", "real", "imag"]),
        "None" => Some(&[]),
        _ => None,
    }
}

/// `true` when `scalar` is a built-in we model and `protocol` requires a
/// non-dunder member the scalar does not provide.
fn scalar_cannot_satisfy(scalar: &str, protocol: &ClassInfo) -> bool {
    let Some(members) = scalar_members(scalar) else {
        return false;
    };
    let mut required = protocol
        .method_names
        .iter()
        .map(String::as_str)
        .chain(protocol.attributes.iter().map(|attr| attr.name.as_str()))
        .filter(|name| !name.starts_with("__"));
    required.any(|name| !members.contains(&name))
}
