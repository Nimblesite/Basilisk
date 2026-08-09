//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Typevar visitor functions.

use ruff_python_ast::{Expr, ExprCall, Stmt};
use ruff_text_size::Ranged;

use crate::canonical::{BindingTable, TypingForm};
use crate::scope::{Pep695BoundViolation, Pep695BoundViolationKind, Span, TypeVarCallInfo};

use super::class_info_ext::expr_simple_name;
use super::core::text_range_to_span;

/// The type-parameter factory form a call expression's callee denotes, if any.
///
/// The question is the callee **node**, resolved through the module's
/// bindings; `from typing import TypeVar as TV` answers identically to
/// `typing.TypeVar`, and a module defining its own `TypeVar` answers not at
/// all. Implements [RESOLV-CANONICAL-BINDING].
fn factory_form<'expr>(
    bindings: &BindingTable,
    expr: &'expr Expr,
) -> Option<(TypingForm, &'expr ExprCall)> {
    let Expr::Call(call) = expr else { return None };
    bindings
        .form_of(&call.func)
        .filter(|form| form.is_type_parameter_factory())
        .map(|form| (form, call))
}

/// Collect every module-level `TypeVar`/`TypeVarTuple`/`ParamSpec` call,
/// including those declared as class attributes.
pub(super) fn collect_typevar_calls(
    bindings: &BindingTable,
    stmts: &[Stmt],
) -> Vec<TypeVarCallInfo> {
    let mut out = Vec::new();
    collect_typevar_calls_from_stmts(bindings, stmts, &mut out);
    out
}

fn collect_typevar_calls_from_stmts(
    bindings: &BindingTable,
    stmts: &[Stmt],
    out: &mut Vec<TypeVarCallInfo>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Assign(node) => {
                let Some((form, call)) = factory_form(bindings, node.value.as_ref()) else {
                    continue;
                };
                let Some(name) = node.targets.first().and_then(expr_simple_name) else {
                    continue;
                };
                let info = typevar_call_info_from(bindings, name, form, call, out);
                out.push(info);
            }
            Stmt::AnnAssign(node) => {
                let Some((form, call)) = node
                    .value
                    .as_deref()
                    .and_then(|value| factory_form(bindings, value))
                else {
                    continue;
                };
                let Some(name) = expr_simple_name(&node.target) else {
                    continue;
                };
                let info = typevar_call_info_from(bindings, name, form, call, out);
                out.push(info);
            }
            // Type parameters are also declared as class attributes.
            Stmt::ClassDef(cls) => {
                collect_typevar_calls_from_stmts(bindings, &cls.body, out);
            }
            // Compound statements whose bodies execute in the module frame:
            // a declaration under `if TYPE_CHECKING:`, in `try:`, a loop, a
            // `with`, or a `match` case binds the module name like any other
            // assignment (PEP 484 declares type variables by assignment;
            // Python's execution model does not care about the nesting).
            Stmt::If(node) => {
                collect_typevar_calls_from_stmts(bindings, &node.body, out);
                for clause in &node.elif_else_clauses {
                    collect_typevar_calls_from_stmts(bindings, &clause.body, out);
                }
            }
            Stmt::Try(node) => {
                collect_typevar_calls_from_stmts(bindings, &node.body, out);
                for handler in &node.handlers {
                    let ruff_python_ast::ExceptHandler::ExceptHandler(handler) = handler;
                    collect_typevar_calls_from_stmts(bindings, &handler.body, out);
                }
                collect_typevar_calls_from_stmts(bindings, &node.orelse, out);
                collect_typevar_calls_from_stmts(bindings, &node.finalbody, out);
            }
            Stmt::With(node) => collect_typevar_calls_from_stmts(bindings, &node.body, out),
            Stmt::For(node) => {
                collect_typevar_calls_from_stmts(bindings, &node.body, out);
                collect_typevar_calls_from_stmts(bindings, &node.orelse, out);
            }
            Stmt::While(node) => {
                collect_typevar_calls_from_stmts(bindings, &node.body, out);
                collect_typevar_calls_from_stmts(bindings, &node.orelse, out);
            }
            Stmt::Match(node) => {
                for case in &node.cases {
                    collect_typevar_calls_from_stmts(bindings, &case.body, out);
                }
            }
            _ => {}
        }
    }
}

/// The keyword argument named `name` on a call, if present.
///
/// Keyword-argument names at a call site need no import and decide nothing
/// about typing, so reading them is permitted.
fn find_keyword<'a>(call: &'a ExprCall, name: &str) -> Option<&'a ruff_python_ast::Keyword> {
    call.arguments
        .keywords
        .iter()
        .find(|kw| kw.arg.as_ref().is_some_and(|arg| arg.as_str() == name))
}

/// Build a [`TypeVarCallInfo`] from a resolved type-parameter factory call.
///
/// `form` is the resolved [`TypingForm`] of the callee — never a spelling.
fn typevar_call_info_from(
    bindings: &BindingTable,
    name: String,
    form: TypingForm,
    call: &ExprCall,
    known_typevars: &[TypeVarCallInfo],
) -> TypeVarCallInfo {
    let kw_is_true = |kw_name: &str| {
        find_keyword(call, kw_name)
            .is_some_and(|kw| matches!(&kw.value, Expr::BooleanLiteral(b) if b.value))
    };
    let bound = find_keyword(call, "bound");
    TypeVarCallInfo {
        name,
        constraint_count: call.arguments.args.len().saturating_sub(1),
        has_default: find_keyword(call, "default").is_some(),
        has_bound: bound.is_some(),
        has_parameterized_constraint: call
            .arguments
            .args
            .iter()
            .skip(1)
            .any(|arg| expr_parameterized_by_typevar(bindings, arg, known_typevars)),
        has_parameterized_bound: bound
            .is_some_and(|kw| expr_parameterized_by_typevar(bindings, &kw.value, known_typevars)),
        is_covariant: kw_is_true("covariant"),
        is_contravariant: kw_is_true("contravariant"),
        has_infer_variance: kw_is_true("infer_variance"),
        span: text_range_to_span(call.range()),
        bound_type_name: bound.and_then(|kw| expr_simple_name(&kw.value)),
        default_type_name: find_keyword(call, "default").and_then(|kw| expr_simple_name(&kw.value)),
        constraint_type_names: call
            .arguments
            .args
            .iter()
            .skip(1)
            .filter_map(expr_simple_name)
            .collect(),
        is_typevartuple: form == TypingForm::TypeVarTuple,
        is_paramspec: form == TypingForm::ParamSpec,
        string_name: call.arguments.args.first().and_then(|arg| {
            if let Expr::StringLiteral(s) = arg {
                Some(s.value.to_str().to_owned())
            } else {
                None
            }
        }),
    }
}

/// Whether a generic type expression is parameterized by a type variable
/// (e.g. `list[T]` where `T` was declared earlier in this module).
///
/// PEP 484 permits fully-concrete generic bounds and constraints such as
/// `dict[str, int]`; only type arguments referencing a type variable make the
/// bound or constraint invalid. A name counts as a type variable only when
/// this walk collected its declaration — never from the shape of the name.
fn expr_parameterized_by_typevar(
    bindings: &BindingTable,
    expr: &Expr,
    known_typevars: &[TypeVarCallInfo],
) -> bool {
    match expr {
        // A BARE type variable as the whole bound/constraint (`bound=T`) is
        // the simplest forbidden case (typing spec, generics: bounds must
        // not contain type variables).
        Expr::Name(_) => expr_references_known_typevar(bindings, expr, known_typevars),
        Expr::Subscript(sub) => expr_references_known_typevar(bindings, &sub.slice, known_typevars),
        Expr::BinOp(bin) => {
            expr_parameterized_by_typevar(bindings, &bin.left, known_typevars)
                || expr_parameterized_by_typevar(bindings, &bin.right, known_typevars)
        }
        Expr::Tuple(tup) => tup
            .elts
            .iter()
            .any(|elt| expr_parameterized_by_typevar(bindings, elt, known_typevars)),
        _ => false,
    }
}

/// Whether a type-argument expression references a collected type variable.
fn expr_references_known_typevar(
    bindings: &BindingTable,
    expr: &Expr,
    known_typevars: &[TypeVarCallInfo],
) -> bool {
    match expr {
        // REBUILT from `tv.name == name.id.as_str()`. Comparing the AST name
        // token against a `TypeVar`'s bound name is spelling identity: after
        // `T = int` the name still "looked like" the TypeVar, while
        // `Alias = T` — one more name for the same TypeVar object — did not.
        // `local_value_binding` resolves the name at its own offset, follows
        // assignment aliases, and yields the range of the EXPRESSION the
        // assignment bound, which for `T = TypeVar("T")` is exactly
        // `TypeVarCallInfo::span` ([RESOLV-CANONICAL-BINDING]).
        Expr::Name(_) => bindings
            .local_value_binding(expr)
            .map(text_range_to_span)
            .is_some_and(|site| known_typevars.iter().any(|tv| tv.span == site)),
        Expr::Subscript(sub) => expr_references_known_typevar(bindings, &sub.slice, known_typevars),
        Expr::Starred(starred) => {
            expr_references_known_typevar(bindings, &starred.value, known_typevars)
        }
        Expr::BinOp(bin) => {
            expr_references_known_typevar(bindings, &bin.left, known_typevars)
                || expr_references_known_typevar(bindings, &bin.right, known_typevars)
        }
        Expr::Tuple(tup) => tup
            .elts
            .iter()
            .any(|elt| expr_references_known_typevar(bindings, elt, known_typevars)),
        // `Callable[[X, Y], Z]` argument lists.
        Expr::List(list) => list
            .elts
            .iter()
            .any(|elt| expr_references_known_typevar(bindings, elt, known_typevars)),
        _ => false,
    }
}

/// Spans of `TypeVar(...)` calls whose `bound=` is the bare `TypedDict` special
/// form, which PEP 484 forbids as a bound — a special form is not a type.
///
/// Both the callee and the bound value resolve through the module's bindings.
pub(super) fn collect_typevar_bound_typeddict_violations(
    bindings: &BindingTable,
    stmts: &[Stmt],
) -> Vec<Span> {
    let mut out = Vec::new();
    for stmt in stmts {
        let Stmt::Assign(node) = stmt else { continue };
        let Some((_, call)) = factory_form(bindings, node.value.as_ref()) else {
            continue;
        };
        let bound_is_typeddict_form = find_keyword(call, "bound")
            .is_some_and(|kw| bindings.is_form(&kw.value, TypingForm::TypedDict));
        if bound_is_typeddict_form {
            out.push(text_range_to_span(call.range()));
        }
    }
    out
}

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
            let _ = current_typeparams;
            outer_typeparams.contains(name.id.as_str())
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

/// Returns `false` if this expression is not a valid constraint tuple element.
///
/// Valid elements are type expressions: names, subscripts, binary ops, string
/// literals (forward references), etc.
/// Invalid elements include numeric and bytes literals (not types).
pub(super) fn is_valid_constraint_element(expr: &Expr) -> bool {
    !matches!(expr, Expr::NumberLiteral(_) | Expr::BytesLiteral(_))
}
