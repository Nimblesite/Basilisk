//! Implements [TYPEINF-TARGET-TYPELEVEL] — the GHC-style acceptance
//! conditions (Paterson/Coverage analogues) for recursive type aliases.
//! See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-TYPELEVEL
//!
//! Type-level computation here is Turing-complete (Roth,
//! <https://arxiv.org/abs/2208.14755>), so definitions are admitted only
//! when termination is evident from their shape — mirroring how GHC's
//! Paterson and Coverage Conditions admit only structurally-decreasing
//! instances (GHC User's Guide §6.8.8):
//!
//! 1. **Guardedness** (contractivity): every self-reference must sit under
//!    at least one type *constructor* (`list[..]`, `dict[..]`, `tuple[..]`,
//!    `set[..]`, any `Named[..]` subscript — including the argument
//!    positions of alias applications, which unfold lazily). Union arms and
//!    conditional-type positions do NOT guard: a `type X = X` or
//!    `type X = int | X` arm unfolds at the same level forever and has no
//!    weak head normal form. This is the conformance-mandated boundary —
//!    upstream `aliases_type_statement.py` requires an error on
//!    `type R3 = R3` and `type R4[T] = T | R4[str]`, while
//!    `type R1[T] = T | list[R1[T]]` must be clean.
//! 2. **Regularity** (the Paterson/Coverage analogue): every
//!    self-application's arguments must be non-growing — each argument is
//!    either a bare parameter reference (Coverage: the parameter is
//!    "covered" exactly as declared) or completely parameter- and
//!    self-free (Paterson: no constructor growth around the recursive
//!    call). `type R[T] = set[R[T]]` and `type A[T] = list[A[int]]` pass;
//!    `type R[T] = set[R[list[T]]]` grows a fresh instantiation per unfold
//!    and is rejected.
//!
//! Rejected definitions can still be admitted through
//! [`super::AliasEnv::insert_undecidable`] — the opt-in "undecidable"
//! escape hatch — where the evaluator's fuel/depth bounds take over and
//! truncation projects to the gradual `Unknown` ([TYPEINF-TARGET-GRADUAL]).

use super::term::{AliasDef, CondTerm, TypeTerm};

/// The verdict of the acceptance conditions for one alias definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acceptance {
    /// Termination is evident: admitted with full trust.
    Accepted,
    /// A self-reference occurs outside every constructor (`type X = X`,
    /// `type X = int | X`): no whnf exists — a genuine circular definition.
    Unguarded,
    /// Guarded, but a self-application's arguments grow (non-regular
    /// recursion): infinitely many distinct instantiations are reachable.
    NonRegular,
}

/// Classify `def` (named `name`) against the acceptance conditions.
#[must_use]
pub fn classify(name: &str, def: &AliasDef) -> Acceptance {
    if !guarded(name, &def.body, false) {
        return Acceptance::Unguarded;
    }
    if !regular(name, &def.body) {
        return Acceptance::NonRegular;
    }
    Acceptance::Accepted
}

/// Guardedness: every self-reference sits beneath at least one constructor.
///
/// `under` tracks whether the walk has passed through a constructor.
/// Subscript *argument* positions count as guarded — they unfold lazily, so
/// recursion through them makes progress toward a head (`type A = B[A]`
/// reaches whnf as soon as `B`'s body exposes a constructor; if it never
/// does, evaluation exhausts fuel and projects to the gradual `Unknown`
/// rather than looping). Union arms and every conditional-type position
/// stay at the same level and do not guard.
fn guarded(name: &str, term: &TypeTerm, under: bool) -> bool {
    match term {
        TypeTerm::Alias(alias, args) => {
            (alias != name || under) && args.iter().all(|arg| guarded(name, arg, true))
        }
        TypeTerm::Op(alias) => alias != name || under,
        TypeTerm::Apply(head, args) => {
            guarded(name, head, under) && args.iter().all(|arg| guarded(name, arg, true))
        }
        TypeTerm::List(inner) | TypeTerm::Set(inner) => guarded(name, inner, true),
        TypeTerm::Dict(key, value) => guarded(name, key, true) && guarded(name, value, true),
        TypeTerm::Tuple(items) | TypeTerm::Named(_, items) => {
            items.iter().all(|item| guarded(name, item, true))
        }
        TypeTerm::Union(arms) => arms.iter().all(|arm| guarded(name, arm, under)),
        TypeTerm::Cond(cond) => cond_positions(cond).all(|part| guarded(name, part, under)),
        TypeTerm::Ground(_) | TypeTerm::Param(_) => true,
    }
}

/// Regularity: every self-application's arguments are non-growing — each is
/// a bare [`TypeTerm::Param`] or completely parameter- and self-free.
fn regular(name: &str, term: &TypeTerm) -> bool {
    let self_app_ok = |args: &[TypeTerm]| {
        args.iter()
            .all(|arg| matches!(arg, TypeTerm::Param(_)) || is_closed(name, arg))
    };
    match term {
        TypeTerm::Alias(alias, args) => {
            (alias != name || self_app_ok(args)) && args.iter().all(|arg| regular(name, arg))
        }
        TypeTerm::Apply(head, args) => {
            let applies_self = matches!(&**head, TypeTerm::Op(alias) if alias == name);
            (!applies_self || self_app_ok(args))
                && regular(name, head)
                && args.iter().all(|arg| regular(name, arg))
        }
        TypeTerm::List(inner) | TypeTerm::Set(inner) => regular(name, inner),
        TypeTerm::Dict(key, value) => regular(name, key) && regular(name, value),
        TypeTerm::Tuple(items) | TypeTerm::Union(items) | TypeTerm::Named(_, items) => {
            items.iter().all(|item| regular(name, item))
        }
        TypeTerm::Cond(cond) => cond_positions(cond).all(|part| regular(name, part)),
        TypeTerm::Ground(_) | TypeTerm::Param(_) | TypeTerm::Op(_) => true,
    }
}

/// Is `term` free of parameters AND of references to `name`? Such an
/// argument cannot grow the instantiation set (Paterson: it contributes a
/// fixed, finite term).
fn is_closed(name: &str, term: &TypeTerm) -> bool {
    match term {
        TypeTerm::Param(_) => false,
        TypeTerm::Ground(_) => true,
        TypeTerm::Op(alias) => alias != name,
        TypeTerm::Alias(alias, args) => {
            alias != name && args.iter().all(|arg| is_closed(name, arg))
        }
        TypeTerm::Apply(head, args) => {
            is_closed(name, head) && args.iter().all(|arg| is_closed(name, arg))
        }
        TypeTerm::List(inner) | TypeTerm::Set(inner) => is_closed(name, inner),
        TypeTerm::Dict(key, value) => is_closed(name, key) && is_closed(name, value),
        TypeTerm::Tuple(items) | TypeTerm::Union(items) | TypeTerm::Named(_, items) => {
            items.iter().all(|item| is_closed(name, item))
        }
        TypeTerm::Cond(cond) => cond_positions(cond).all(|part| is_closed(name, part)),
    }
}

/// The four positions of a conditional type, for uniform traversal.
fn cond_positions(cond: &CondTerm) -> impl Iterator<Item = &TypeTerm> {
    [
        &cond.scrutinee,
        &cond.against,
        &cond.then_arm,
        &cond.else_arm,
    ]
    .into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::InferredType;

    fn int() -> TypeTerm {
        TypeTerm::Ground(InferredType::Int)
    }

    fn self_ref(args: Vec<TypeTerm>) -> TypeTerm {
        TypeTerm::Alias("X".to_owned(), args)
    }

    fn classify_body(arity: usize, body: TypeTerm) -> Acceptance {
        classify("X", &AliasDef { arity, body })
    }

    /// Guarded recursion in every constructor is accepted — the #371
    /// boundary: `list`, `dict`, `set`, `tuple`, and arbitrary `Named`
    /// subscripts all guard.
    #[test]
    fn guarded_recursion_is_accepted() {
        let cases = [
            TypeTerm::List(Box::new(self_ref(Vec::new()))),
            TypeTerm::Union(vec![int(), TypeTerm::List(Box::new(self_ref(Vec::new())))]),
            TypeTerm::Dict(
                Box::new(TypeTerm::Ground(InferredType::Str)),
                Box::new(self_ref(Vec::new())),
            ),
            TypeTerm::Set(Box::new(self_ref(Vec::new()))),
            TypeTerm::Tuple(vec![self_ref(Vec::new()), int()]),
            TypeTerm::Named("Sequence".to_owned(), vec![self_ref(Vec::new())]),
        ];
        for body in cases {
            assert_eq!(
                classify_body(0, body.clone()),
                Acceptance::Accepted,
                "{body:?}"
            );
        }
    }

    /// Unguarded self-references — bare, or through union arms — are the
    /// genuine circular definitions and are rejected.
    #[test]
    fn unguarded_recursion_is_rejected() {
        let cases = [
            self_ref(Vec::new()),
            TypeTerm::Union(vec![int(), self_ref(Vec::new())]),
            TypeTerm::Union(vec![TypeTerm::Param(0), self_ref(vec![int()])]),
        ];
        for body in cases {
            assert_eq!(
                classify_body(1, body.clone()),
                Acceptance::Unguarded,
                "{body:?}"
            );
        }
    }

    /// Regular self-applications — identity parameters or closed arguments
    /// — are accepted (Coverage/Paterson satisfied).
    #[test]
    fn regular_self_applications_are_accepted() {
        let identity = TypeTerm::Set(Box::new(self_ref(vec![TypeTerm::Param(0)])));
        let closed = TypeTerm::List(Box::new(self_ref(vec![int()])));
        assert_eq!(classify_body(1, identity), Acceptance::Accepted);
        assert_eq!(classify_body(1, closed), Acceptance::Accepted);
    }

    /// Growing self-applications — a parameter under a constructor, or a
    /// nested self-reference, in argument position — are non-regular.
    #[test]
    fn growing_self_applications_are_non_regular() {
        let growing_param = TypeTerm::Set(Box::new(self_ref(vec![TypeTerm::List(Box::new(
            TypeTerm::Param(0),
        ))])));
        let nested_self = TypeTerm::Set(Box::new(self_ref(vec![TypeTerm::Union(vec![
            TypeTerm::Param(0),
            self_ref(vec![TypeTerm::Param(0)]),
        ])])));
        assert_eq!(classify_body(1, growing_param), Acceptance::NonRegular);
        assert_eq!(classify_body(1, nested_self), Acceptance::NonRegular);
    }

    /// Conditional-type positions do not guard: a self-reference in an arm
    /// (even the lazily-evaluated one) is statically unguarded, because the
    /// taken arm unfolds at the same level.
    #[test]
    fn conditional_positions_do_not_guard() {
        let cond = TypeTerm::Cond(Box::new(CondTerm {
            scrutinee: TypeTerm::Param(0),
            against: int(),
            then_arm: int(),
            else_arm: self_ref(vec![TypeTerm::Param(0)]),
        }));
        assert_eq!(classify_body(1, cond), Acceptance::Unguarded);

        let guarded_cond = TypeTerm::Cond(Box::new(CondTerm {
            scrutinee: TypeTerm::Param(0),
            against: int(),
            then_arm: int(),
            else_arm: TypeTerm::List(Box::new(self_ref(vec![TypeTerm::Param(0)]))),
        }));
        assert_eq!(classify_body(1, guarded_cond), Acceptance::Accepted);
    }

    /// Operator references participate: an unapplied self-`Op` at the top
    /// is unguarded; applying self through `Apply` with growing arguments
    /// is non-regular.
    #[test]
    fn operator_forms_are_classified() {
        assert_eq!(
            classify_body(1, TypeTerm::Op("X".to_owned())),
            Acceptance::Unguarded
        );
        let apply_growing = TypeTerm::List(Box::new(TypeTerm::Apply(
            Box::new(TypeTerm::Op("X".to_owned())),
            vec![TypeTerm::List(Box::new(TypeTerm::Param(0)))],
        )));
        assert_eq!(classify_body(1, apply_growing), Acceptance::NonRegular);
    }
}
