//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Typevar visitor functions.

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use crate::scope::{Pep695BoundViolation, Pep695BoundViolationKind, Span, TypeVarCallInfo};

use super::class_info_ext::expr_simple_name;
use super::core::text_range_to_span;
use super::typeddict::expr_is_parameterized;

pub(super) fn typevar_like_callee(expr: &Expr) -> Option<&str> {
    let Expr::Call(call) = expr else { return None };
    match call.func.as_ref() {
        Expr::Name(n) => match n.id.as_str() {
            "TypeVar" | "TypeVarTuple" | "ParamSpec" => Some(n.id.as_str()),
            _ => None,
        },
        Expr::Attribute(a) => match a.attr.as_str() {
            "TypeVar" | "TypeVarTuple" | "ParamSpec" => Some(a.attr.as_str()),
            _ => None,
        },
        _ => None,
    }
}

/// Returns `true` if an expression is a `TypeVar(...)` or `typing.TypeVar(...)` call.
pub(super) fn is_typevar_call(expr: &Expr) -> bool {
    let Expr::Call(call) = expr else { return false };
    (expr_simple_name(&call.func).as_deref() == Some("TypeVar"))
        || matches!(call.func.as_ref(), Expr::Attribute(a) if a.attr.as_str() == "TypeVar")
}

/// Builds a `TypeVarCallInfo` from a TypeVar-like call expression.
///
/// `callee` is `"TypeVar"`, `"TypeVarTuple"`, or `"ParamSpec"`.
pub(super) fn typevar_call_info_from(
    name: String,
    callee: &str,
    call: &ruff_python_ast::ExprCall,
) -> TypeVarCallInfo {
    use ruff_text_size::Ranged as _;
    let positional_args = call.arguments.args.len();
    let constraint_count = positional_args.saturating_sub(1);
    let find_kw = |name: &str| {
        call.arguments
            .keywords
            .iter()
            .find(|kw| kw.arg.as_ref().is_some_and(|a| a.as_str() == name))
    };
    let kw_is_true = |name: &str| {
        find_kw(name).is_some_and(|kw| matches!(&kw.value, Expr::BooleanLiteral(b) if b.value))
    };
    let has_default = find_kw("default").is_some();
    let has_bound = find_kw("bound").is_some();
    let has_parameterized_bound =
        find_kw("bound").is_some_and(|kw| expr_is_parameterized(&kw.value));
    let has_parameterized_constraint = call
        .arguments
        .args
        .iter()
        .skip(1)
        .any(expr_is_parameterized);
    let is_covariant = kw_is_true("covariant");
    let is_contravariant = kw_is_true("contravariant");
    let has_infer_variance = kw_is_true("infer_variance");
    // Simple name from the `bound=` keyword argument (if present and a plain Name).
    let bound_type_name = find_kw("bound").and_then(|kw| expr_simple_name(&kw.value));
    // Simple name from the `default=` keyword argument (if present and a plain Name).
    let default_type_name = find_kw("default").and_then(|kw| expr_simple_name(&kw.value));
    // Constraint type names from positional args (skip the first arg which is the TypeVar name).
    let constraint_type_names: Vec<String> = call
        .arguments
        .args
        .iter()
        .skip(1)
        .filter_map(expr_simple_name)
        .collect();
    // Extract the string value of the first positional argument (the name string).
    let string_name = call.arguments.args.first().and_then(|arg| {
        if let Expr::StringLiteral(s) = arg {
            Some(s.value.to_str().to_owned())
        } else {
            None
        }
    });
    TypeVarCallInfo {
        name,
        constraint_count,
        has_default,
        has_bound,
        has_parameterized_bound,
        has_parameterized_constraint,
        is_covariant,
        is_contravariant,
        has_infer_variance,
        span: text_range_to_span(call.range()),
        bound_type_name,
        default_type_name,
        constraint_type_names,
        is_typevartuple: callee == "TypeVarTuple",
        is_paramspec: callee == "ParamSpec",
        string_name,
    }
}

pub(super) fn collect_typevar_calls(stmts: &[Stmt]) -> Vec<TypeVarCallInfo> {
    let mut out = Vec::new();
    collect_typevar_calls_from_stmts(stmts, &mut out);
    out
}

pub(super) fn collect_typevar_calls_from_stmts(stmts: &[Stmt], out: &mut Vec<TypeVarCallInfo>) {
    for stmt in stmts {
        match stmt {
            Stmt::Assign(node) => {
                let Expr::Call(call) = node.value.as_ref() else {
                    continue;
                };
                let Some(callee) = typevar_like_callee(node.value.as_ref()) else {
                    continue;
                };
                let Some(name) = node.targets.first().and_then(expr_simple_name) else {
                    continue;
                };
                out.push(typevar_call_info_from(name, callee, call));
            }
            Stmt::AnnAssign(node) => {
                let Some(val) = node.value.as_deref() else {
                    continue;
                };
                let Expr::Call(call) = val else { continue };
                let Some(callee) = typevar_like_callee(val) else {
                    continue;
                };
                let Some(name) = expr_simple_name(&node.target) else {
                    continue;
                };
                out.push(typevar_call_info_from(name, callee, call));
            }
            // Also search inside class bodies (TypeVars declared as class attributes).
            Stmt::ClassDef(cls) => {
                collect_typevar_calls_from_stmts(&cls.body, out);
            }
            _ => {}
        }
    }
}

/// Collect all `reveal_type(...)` calls found anywhere in the module body.
pub(super) fn check_typevar_bound_expr(
    bound: &Expr,
    class_name: &str,
    type_param: &str,
    bare_names: &std::collections::HashSet<String>,
    current_typeparams: &std::collections::HashSet<String>,
    outer_typeparams: &std::collections::HashSet<String>,
    out: &mut Vec<Pep695BoundViolation>,
) {
    let make =
        |kind: Pep695BoundViolationKind, range: ruff_text_size::TextRange| Pep695BoundViolation {
            kind,
            class_name: class_name.to_owned(),
            type_param_name: type_param.to_owned(),
            span: text_range_to_span(range),
        };

    match bound {
        Expr::List(list) => {
            out.push(make(
                Pep695BoundViolationKind::ListLiteralBound,
                list.range(),
            ));
        }
        Expr::Tuple(tup) => {
            if tup.elts.is_empty() {
                out.push(make(Pep695BoundViolationKind::EmptyTuple, tup.range()));
            } else if tup.elts.len() == 1 {
                out.push(make(
                    Pep695BoundViolationKind::SingleElementTuple,
                    tup.range(),
                ));
            } else {
                // Check for invalid elements and outer-scope TypeVar references.
                let mut emitted = false;
                for elt in &tup.elts {
                    if !is_valid_constraint_element(elt) {
                        out.push(make(
                            Pep695BoundViolationKind::InvalidConstraintElement,
                            elt.range(),
                        ));
                        emitted = true;
                        break;
                    }
                }
                if !emitted {
                    for elt in &tup.elts {
                        if bound_refs_outer_typeparam(elt, current_typeparams, outer_typeparams) {
                            out.push(make(
                                Pep695BoundViolationKind::OuterScopeTypeVarInBound,
                                elt.range(),
                            ));
                            break;
                        }
                    }
                }
            }
        }
        Expr::Name(name) if bare_names.contains(name.id.as_str()) => {
            out.push(make(
                Pep695BoundViolationKind::NonLiteralConstraint,
                name.range(),
            ));
        }
        // Check if the bound itself references an outer-scope TypeVar (e.g. `T: dict[str, V]`).
        bound_expr
            if bound_refs_outer_typeparam(bound_expr, current_typeparams, outer_typeparams) =>
        {
            out.push(make(
                Pep695BoundViolationKind::OuterScopeTypeVarInBound,
                bound_expr.range(),
            ));
        }
        _ => {}
    }
}

/// Returns `true` if the expression references an outer-scope `TypeParam` or a
/// TypeVar-like name that is not in the current class's `TypeParam` set.
///
/// Used to detect cases like `class Nested[T: dict[str, V]]` where `V` is from
/// an outer class, or `class Foo[T: (list[S], str)]` where `S` is unresolved.
pub(super) fn bound_refs_outer_typeparam(
    expr: &Expr,
    current_typeparams: &std::collections::HashSet<String>,
    outer_typeparams: &std::collections::HashSet<String>,
) -> bool {
    match expr {
        Expr::Name(name) => {
            let n = name.id.as_str();
            // Explicitly an outer TypeVar, or a TypeVar-like single-letter uppercase name
            // not in the current class's TypeParam set.
            outer_typeparams.contains(n)
                || (is_typevar_like_name(n) && !current_typeparams.contains(n))
        }
        Expr::Subscript(sub) => {
            // Check the type arguments of a generic type expression, not the base type.
            // e.g. for `list[S]`, we check `S` not `list`.
            bound_refs_outer_typeparam(&sub.slice, current_typeparams, outer_typeparams)
        }
        Expr::Tuple(t) => t
            .elts
            .iter()
            .any(|e| bound_refs_outer_typeparam(e, current_typeparams, outer_typeparams)),
        Expr::BinOp(bin) => {
            bound_refs_outer_typeparam(&bin.left, current_typeparams, outer_typeparams)
                || bound_refs_outer_typeparam(&bin.right, current_typeparams, outer_typeparams)
        }
        _ => false,
    }
}

/// Returns `true` if the name looks like a `TypeVar` by the single-letter uppercase convention.
///
/// Single-letter uppercase names (e.g. `T`, `S`, `V`) are almost universally `TypeVars`.
/// Multi-letter names could be concrete types (e.g. `str`, `int`, `ForwardReference`).
pub(super) fn is_typevar_like_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() == 1
        && bytes
            .first()
            .copied()
            .is_some_and(|b| b.is_ascii_uppercase())
}

/// Returns `false` if this expression is not a valid constraint tuple element.
///
/// Valid elements are type expressions: names, subscripts, binary ops, string
/// literals (forward references), etc.
/// Invalid elements include numeric and bytes literals (not types).
pub(super) fn is_valid_constraint_element(expr: &Expr) -> bool {
    !matches!(expr, Expr::NumberLiteral(_) | Expr::BytesLiteral(_))
}

// ---------------------------------------------------------------------------
// Multiple unbounded tuple detection (for BSK-E0047)
// ---------------------------------------------------------------------------

/// Counts the number of "unbounded" components in a `tuple[...]` slice expression.
///
/// An unbounded component is one of:
/// - `*tuple[T, ...]` — a starred subscript where the inner tuple ends with `...`
/// - `*Ts` / `*<Name>` — a starred name (`TypeVarTuple` unpack)
/// - `Unpack[tuple[T, ...]]` — legacy Unpack form
///
/// Returns the count of unbounded components found.
pub(super) fn collect_typevar_bound_typeddict_violations(stmts: &[Stmt]) -> Vec<Span> {
    use ruff_text_size::Ranged as _;
    let mut out = Vec::new();
    for stmt in stmts {
        let Stmt::Assign(node) = stmt else { continue };
        let Expr::Call(call) = node.value.as_ref() else {
            continue;
        };
        if !is_typevar_call(node.value.as_ref()) {
            continue;
        }
        for kw in &call.arguments.keywords {
            let is_bound_kw = kw.arg.as_ref().is_some_and(|a| a.as_str() == "bound");
            if !is_bound_kw {
                continue;
            }
            let is_typeddict = matches!(
                &kw.value,
                Expr::Name(n) if n.id.as_str() == "TypedDict"
            );
            if is_typeddict {
                out.push(Span::from(call.range()));
            }
        }
    }
    out
}
