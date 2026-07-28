//! Implements [`protocols_definition_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
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

    // Everything needed to decide that a member is genuinely absent rather than
    // merely invisible from here ([`visible_members`]).
    let knowledge = ClassKnowledge {
        class_map,
        self_attrs: super::ast_index::self_attrs_by_class(body),
        opaque: opaque_classes(body),
    };

    basilisk_resolver::walk_all_stmts(body, &mut |stmt| match stmt {
        Stmt::Expr(node) => scan_expr(
            &node.value,
            &functions,
            &knowledge,
            &module.path,
            diagnostics,
        ),
        Stmt::Assign(node) => {
            scan_expr(
                &node.value,
                &functions,
                &knowledge,
                &module.path,
                diagnostics,
            );
        }
        Stmt::AnnAssign(node) => {
            if let Some(value) = node.value.as_deref() {
                scan_expr(value, &functions, &knowledge, &module.path, diagnostics);
            }
        }
        Stmt::Return(node) => {
            if let Some(value) = node.value.as_deref() {
                scan_expr(value, &functions, &knowledge, &module.path, diagnostics);
            }
        }
        _ => {}
    });
}

/// What this module can prove about the classes it can see.
struct ClassKnowledge<'a> {
    class_map: &'a HashMap<&'a str, &'a ClassInfo>,
    /// `self.<name> = ...` bindings per class, from its own methods.
    self_attrs: HashMap<&'a str, std::collections::HashSet<String>>,
    /// Classes whose member set cannot be trusted ([`opaque_classes`]).
    opaque: std::collections::HashSet<String>,
}

/// Recursively check call expressions (including nested call arguments).
fn scan_expr(
    expr: &Expr,
    functions: &HashMap<&str, &FunctionInfo>,
    knowledge: &ClassKnowledge<'_>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Expr::Call(call) = expr else { return };
    if let Expr::Name(name) = call.func.as_ref() {
        if let Some(func) = functions.get(name.id.as_str()) {
            validate_call(call, func, knowledge, path, diagnostics);
        }
    }
    for arg in &call.arguments.args {
        scan_expr(arg, functions, knowledge, path, diagnostics);
    }
}

/// Validate each positional argument of `call` against the matching parameter's
/// protocol-container annotation.
fn validate_call(
    call: &ruff_python_ast::ExprCall,
    func: &FunctionInfo,
    knowledge: &ClassKnowledge<'_>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let class_map = knowledge.class_map;
    for (arg, param) in call.arguments.args.iter().zip(&func.parameters) {
        let Some(ann) = param.annotation_text.as_deref() else {
            continue;
        };
        // A parameter annotated with a bare Protocol is checked structurally,
        // the same way an annotated assignment already is.
        if let Some(protocol) = bare_protocol(ann, class_map) {
            report_non_conforming_argument(arg, param, protocol, knowledge, path, diagnostics);
        }
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

/// If `ann` names a locally defined `Protocol` class directly (not wrapped in a
/// container), return that protocol's `ClassInfo`.
fn bare_protocol<'a>(ann: &str, class_map: &HashMap<&str, &'a ClassInfo>) -> Option<&'a ClassInfo> {
    let class = class_map.get(ann.trim())?;
    class
        .bases
        .iter()
        .any(|base| base == "Protocol")
        .then_some(*class)
}

/// Report `ClassName()` passed to a bare-`Protocol` parameter when the class
/// provably lacks a required member.
///
/// Deliberately narrow, because a false positive here is worse than a miss
/// ([CHKARCH-CONFORMANCE]): it fires only for a direct constructor call of a
/// class whose every base is locally visible, so "the member is absent" is a
/// fact about the whole inheritance chain rather than about the one class body
/// this module happens to see.
fn report_non_conforming_argument(
    arg: &Expr,
    param: &basilisk_resolver::ParameterInfo,
    protocol: &ClassInfo,
    knowledge: &ClassKnowledge<'_>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let class_map = knowledge.class_map;
    let Some(class) = constructed_class(arg, class_map) else {
        return;
    };
    // A protocol argument satisfying the protocol nominally is fine regardless
    // of what this module can see of its members.
    if class.bases.contains(&protocol.name) {
        return;
    }
    let Some(available) =
        visible_members(class, class_map, &knowledge.self_attrs, &knowledge.opaque)
    else {
        return;
    };
    let required = super::collect_protocol_required_methods(protocol, class_map);
    let missing: Vec<&str> = required
        .iter()
        .map(String::as_str)
        .filter(|name| !name.starts_with("__") && !available.contains(*name))
        .collect();
    if missing.is_empty() {
        return;
    }

    let plural = if missing.len() == 1 { "" } else { "s" };
    let missing_list = missing.join("`, `");
    let range = arg.range();
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Argument is incompatible with parameter `{}` of type `{}`: class `{}` is \
             missing method{plural} `{missing_list}`",
            param.name, protocol.name, class.name
        ),
        Span {
            start: range.start().to_u32(),
            end: range.end().to_u32(),
        },
        path,
        Some(format!(
            "Add the missing method{plural} to `{}` or pass a class that implements `{}`",
            class.name, protocol.name
        )),
        Some(
            "Protocol classes use structural subtyping: the argument's class must \
             implement every member the protocol declares"
                .to_owned(),
        ),
    ));
}

/// The locally defined, non-protocol class a direct `ClassName()` call builds.
fn constructed_class<'a>(
    arg: &Expr,
    class_map: &HashMap<&str, &'a ClassInfo>,
) -> Option<&'a ClassInfo> {
    let Expr::Call(inner) = arg else { return None };
    let Expr::Name(name) = inner.func.as_ref() else {
        return None;
    };
    let class = class_map.get(name.id.as_str())?;
    (!class.bases.iter().any(|base| base == "Protocol")).then_some(*class)
}

/// Class-body statements that can bind a member where `ClassInfo` will not see
/// it: a `def` guarded by `if TYPE_CHECKING:`, a `sys.version_info` branch, a
/// `try`/`except` import fallback, and so on. A class containing any of these
/// has an unknown member set and must never be blamed for a missing member.
fn hides_members(class_def: &ruff_python_ast::StmtClassDef) -> bool {
    class_def.body.iter().any(|stmt| {
        matches!(
            stmt,
            Stmt::If(_)
                | Stmt::Try(_)
                | Stmt::With(_)
                | Stmt::For(_)
                | Stmt::While(_)
                | Stmt::Match(_)
        )
    })
}

/// Names of locally defined classes whose member set cannot be trusted.
///
/// Two ways a class earns its place here: a class-body statement that hides a
/// definition ([`hides_members`]), or a `__getattr__`/`__getattribute__` hook,
/// which can answer for any member name at all.
pub(super) fn opaque_classes(body: &[Stmt]) -> std::collections::HashSet<String> {
    let mut opaque = std::collections::HashSet::new();
    basilisk_resolver::walk_all_stmts(body, &mut |stmt| {
        if let Stmt::ClassDef(class_def) = stmt {
            let dynamic_lookup = class_def.body.iter().any(|inner| {
                matches!(inner, Stmt::FunctionDef(func)
                    if func.name.as_str() == "__getattr__"
                        || func.name.as_str() == "__getattribute__")
            });
            if dynamic_lookup || hides_members(class_def) {
                let _ = opaque.insert(class_def.name.to_string());
            }
        }
    });
    opaque
}

/// Every member name `class` provides, following its bases.
///
/// Returns `None` whenever the member set cannot be known in full, which is the
/// only false-positive-safe answer:
/// - a base that is not locally defined may supply the member;
/// - a class that hides definitions or defines `__getattr__` ([`opaque_classes`]).
///
/// `self_attrs` carries `self.<name> = ...` bindings from each class's own
/// methods — the same source the annotated-assignment path already consults, so
/// a member installed in `__init__` counts here exactly as it does there.
fn visible_members(
    class: &ClassInfo,
    class_map: &HashMap<&str, &ClassInfo>,
    self_attrs: &HashMap<&str, std::collections::HashSet<String>>,
    opaque: &std::collections::HashSet<String>,
) -> Option<std::collections::HashSet<String>> {
    let mut members: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut queue = vec![class];
    let mut seen: Vec<&str> = Vec::new();

    while let Some(current) = queue.pop() {
        if seen.contains(&current.name.as_str()) {
            continue;
        }
        if opaque.contains(current.name.as_str()) {
            return None;
        }
        seen.push(current.name.as_str());
        members.extend(current.method_names.iter().cloned());
        members.extend(current.attributes.iter().map(|attr| attr.name.clone()));
        if let Some(attrs) = self_attrs.get(current.name.as_str()) {
            members.extend(attrs.iter().cloned());
        }

        for base in &current.bases {
            if base == "object" || base == "Protocol" || base == "Generic" {
                continue;
            }
            queue.push(class_map.get(base.as_str())?);
        }
    }

    Some(members)
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
