//! Implements [RESOLV-CANONICAL-RELATION].
//! See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#RESOLV-CANONICAL
//!
//! Semantic relations over resolved type expressions.
//!
//! Every answer is three-valued: `Some(true)` and `Some(false)` are verdicts
//! the typing specification licenses from the resolved structure alone;
//! `None` is honest abstention. A relation this layer does not model —
//! protocol subtyping, user classes, callable signatures — abstains rather
//! than guesses, so a rule consuming it emits a diagnostic only on
//! `Some(false)`.
//!
//! Spec sources: assignability and consistency
//! (<https://typing.python.org/en/latest/spec/concepts.html>), the
//! `float`/`complex` special cases
//! (<https://typing.python.org/en/latest/spec/special-types.html#special-cases-for-float-and-complex>),
//! and `Literal` semantics
//! (<https://typing.python.org/en/latest/spec/literal.html>).

use crate::type_node::{BuiltinClass, LiteralValue, TypeNode};

/// Is `source` assignable to `target`?
#[must_use]
pub fn assignable(source: &TypeNode, target: &TypeNode) -> Option<bool> {
    match (source, target) {
        (TypeNode::Any, _) | (_, TypeNode::Any) => Some(true),
        (TypeNode::Never, _) => Some(true),
        (_, TypeNode::Builtin(BuiltinClass::Object)) => Some(true),
        (TypeNode::Unknown, _) | (_, TypeNode::Unknown) => None,
        (_, TypeNode::Never) => Some(false),
        (TypeNode::Ellipsis, TypeNode::Ellipsis) => Some(true),
        (TypeNode::Ellipsis, _) | (_, TypeNode::Ellipsis) => None,
        (TypeNode::Union(members), _) => all3(members.iter().map(|m| assignable(m, target))),
        (_, TypeNode::Union(members)) => any3(members.iter().map(|m| assignable(source, m))),
        (TypeNode::NoneType, other) => none_assignable_to(other),
        (_, TypeNode::NoneType) => Some(false),
        (TypeNode::Literal(value), _) => literal_assignable(value, target),
        (TypeNode::LiteralString, _) => literal_string_assignable(target),
        (TypeNode::Builtin(class), _) => builtin_assignable(*class, target),
        (TypeNode::Subscript { base, args }, _) => subscript_assignable(base, args, target),
        (TypeNode::Form(form), TypeNode::Form(other)) if form == other => Some(true),
        (TypeNode::Form(_), _) => None,
    }
}

/// Are `a` and `b` equivalent — mutually assignable
/// (<https://typing.python.org/en/latest/spec/glossary.html#term-equivalent>)?
#[must_use]
pub fn equivalent(a: &TypeNode, b: &TypeNode) -> Option<bool> {
    match (assignable(a, b), assignable(b, a)) {
        (Some(false), _) | (_, Some(false)) => Some(false),
        (Some(true), Some(true)) => Some(true),
        _ => None,
    }
}

/// Three-valued AND: any false wins, all true wins, otherwise unknown.
fn all3(answers: impl Iterator<Item = Option<bool>>) -> Option<bool> {
    let mut result = Some(true);
    for answer in answers {
        match answer {
            Some(false) => return Some(false),
            Some(true) => {}
            None => result = None,
        }
    }
    result
}

/// Three-valued OR: any true wins, all false wins, otherwise unknown.
fn any3(answers: impl Iterator<Item = Option<bool>>) -> Option<bool> {
    let mut result = Some(false);
    for answer in answers {
        match answer {
            Some(true) => return Some(true),
            Some(false) => {}
            None => result = None,
        }
    }
    result
}

/// What accepts `None`: only `NoneType` itself among modelled targets;
/// abstract forms could (`None` is hashable), so they abstain.
fn none_assignable_to(target: &TypeNode) -> Option<bool> {
    match target {
        TypeNode::NoneType => Some(true),
        TypeNode::Form(_) | TypeNode::Subscript { .. } => match subscript_base_class(target) {
            Some(_) => Some(false),
            None => None,
        },
        _ => Some(false),
    }
}

/// Nominal assignability between builtin classes: identity, the numeric
/// tower's special cases, and `object` as top.
fn class_assignable(source: BuiltinClass, target: BuiltinClass) -> bool {
    if source == target || target == BuiltinClass::Object {
        return true;
    }
    matches!(
        (source, target),
        (
            BuiltinClass::Bool,
            BuiltinClass::Int | BuiltinClass::Float | BuiltinClass::Complex
        ) | (
            BuiltinClass::Int,
            BuiltinClass::Float | BuiltinClass::Complex
        ) | (BuiltinClass::Float, BuiltinClass::Complex)
    )
}

/// Assignability from a single-value literal.
fn literal_assignable(value: &LiteralValue, target: &TypeNode) -> Option<bool> {
    match target {
        TypeNode::Literal(other) => Some(value == other),
        TypeNode::LiteralString => Some(matches!(value, LiteralValue::Str(_))),
        TypeNode::Builtin(class) => Some(class_assignable(value.value_class(), *class)),
        TypeNode::Subscript { .. } => match subscript_base_class(target) {
            Some(_) => Some(false),
            None => None,
        },
        TypeNode::Form(_) => None,
        _ => None,
    }
}

/// Assignability from `LiteralString`: it sits strictly between string
/// literals and `str` (PEP 675).
fn literal_string_assignable(target: &TypeNode) -> Option<bool> {
    match target {
        TypeNode::LiteralString | TypeNode::Builtin(BuiltinClass::Str) => Some(true),
        TypeNode::Builtin(_) | TypeNode::Literal(_) => Some(false),
        TypeNode::Subscript { .. } => match subscript_base_class(target) {
            Some(_) => Some(false),
            None => None,
        },
        TypeNode::Form(_) => None,
        _ => None,
    }
}

/// Assignability from a bare builtin class. A bare container is its
/// parameterization by `Any`, so it is consistent with every
/// parameterization of the same class.
fn builtin_assignable(class: BuiltinClass, target: &TypeNode) -> Option<bool> {
    match target {
        TypeNode::Builtin(other) => Some(class_assignable(class, *other)),
        TypeNode::Literal(_) | TypeNode::LiteralString => Some(false),
        TypeNode::Subscript { .. } => subscript_base_class(target).map(|base| base == class),
        TypeNode::Form(_) => None,
        _ => None,
    }
}

/// Assignability from a parameterized type.
fn subscript_assignable(base: &TypeNode, args: &[TypeNode], target: &TypeNode) -> Option<bool> {
    match (base, target) {
        (TypeNode::Builtin(class), TypeNode::Builtin(other)) => Some(class == other),
        (
            TypeNode::Builtin(class),
            TypeNode::Subscript {
                base: tbase,
                args: targs,
            },
        ) => match tbase.as_ref() {
            TypeNode::Builtin(tclass) if class == tclass => {
                parameter_args_assignable(*class, args, targs)
            }
            TypeNode::Builtin(_) => Some(false),
            _ => None,
        },
        (
            TypeNode::Form(form),
            TypeNode::Subscript {
                base: tbase,
                args: targs,
            },
        ) => match tbase.as_ref() {
            TypeNode::Form(tform) if form == tform => {
                match all3(pairwise(args, targs, equivalent)?.into_iter()) {
                    Some(true) => Some(true),
                    _ => None,
                }
            }
            _ => None,
        },
        (TypeNode::Builtin(_), TypeNode::Literal(_) | TypeNode::LiteralString) => Some(false),
        _ => None,
    }
}

/// Relate parameter lists of one builtin class by its variance.
fn parameter_args_assignable(
    class: BuiltinClass,
    source_args: &[TypeNode],
    target_args: &[TypeNode],
) -> Option<bool> {
    let Some(pairs) = pairwise(source_args, target_args, |s, t| {
        if class_is_covariant(class) {
            assignable(s, t)
        } else {
            equivalent(s, t)
        }
    }) else {
        let variadic =
            source_args.contains(&TypeNode::Ellipsis) || target_args.contains(&TypeNode::Ellipsis);
        return if variadic { None } else { Some(false) };
    };
    all3(pairs.into_iter())
}

/// Whether a builtin generic's parameters are covariant. The mutable
/// containers are invariant; `tuple`, `frozenset`, and `type` are covariant.
fn class_is_covariant(class: BuiltinClass) -> bool {
    matches!(
        class,
        BuiltinClass::Tuple | BuiltinClass::Frozenset | BuiltinClass::Type
    )
}

/// Zip two equal-length argument lists through `relate`; `None` when the
/// lengths differ.
fn pairwise(
    left: &[TypeNode],
    right: &[TypeNode],
    relate: impl Fn(&TypeNode, &TypeNode) -> Option<bool>,
) -> Option<Vec<Option<bool>>> {
    (left.len() == right.len()).then(|| {
        left.iter()
            .zip(right.iter())
            .map(|(l, r)| relate(l, r))
            .collect()
    })
}

/// The builtin class at a subscript's base, if that is what the base is.
fn subscript_base_class(node: &TypeNode) -> Option<BuiltinClass> {
    match node {
        TypeNode::Subscript { base, .. } => match base.as_ref() {
            TypeNode::Builtin(class) => Some(*class),
            _ => None,
        },
        _ => None,
    }
}
