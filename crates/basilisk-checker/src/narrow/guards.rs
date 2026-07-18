//! Implements [TYPEINF-TARGET-NARROWING]. See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-NARROWING
//! Consumption of the resolver's collected narrowing guards
//! ([`NarrowingGuardKind`]) into positive/negative environment updates —
//! `isinstance`, `is None`, truthiness, `TypeGuard`, `TypeIs`, `assert`, and
//! `match` ([NARROWPLAN-CHECKLIST] Stage 2, "consume resolver guards").

use basilisk_resolver::{NarrowingGuard, NarrowingGuardKind};

use crate::types::InferredType;

use super::set_ops::{intersect, subtract};

/// What one guard does to one variable in each branch.
#[derive(Debug, Clone, PartialEq)]
pub struct GuardOutcome {
    /// The variable the guard narrows.
    pub variable: String,
    /// The type in the positive (guard-true) branch.
    pub positive: InferredType,
    /// The type in the negative (guard-false) branch.
    pub negative: InferredType,
    /// Whether the positive narrowing outlives any branch (the `assert` form).
    pub whole_scope: bool,
}

/// Interpret one resolver guard against the variable's current type.
///
/// Guards collected inside loops (`guard.in_loop`) return `None`: their
/// narrowing must not persist past the loop body
/// ([TYPEINF-NARROWING-SCOPE]), and Stage 2's statement-order environment
/// applies them only when flow-accurate.
#[must_use]
pub fn guard_outcomes(guard: &NarrowingGuard, current: &InferredType) -> Option<GuardOutcome> {
    if guard.in_loop {
        return None;
    }
    outcome_for_kind(&guard.kind, current, false)
}

/// Interpret a guard kind (recursing through `assert`).
fn outcome_for_kind(
    kind: &NarrowingGuardKind,
    current: &InferredType,
    whole_scope: bool,
) -> Option<GuardOutcome> {
    match kind {
        NarrowingGuardKind::IsInstance {
            variable,
            type_names,
            ..
        } => {
            let guard_type = union_of_names(type_names);
            Some(GuardOutcome {
                variable: variable.clone(),
                positive: intersect(current, &guard_type),
                negative: subtract(current, &guard_type),
                whole_scope,
            })
        }
        NarrowingGuardKind::IsNone {
            variable,
            is_positive,
            ..
        } => Some(none_outcome(variable, current, *is_positive, whole_scope)),
        NarrowingGuardKind::Truthiness { variable, .. } => Some(GuardOutcome {
            variable: variable.clone(),
            // Truthiness eliminates `None` in the truthy branch; the falsy
            // branch stays unchanged (falsy ints/strs are still ints/strs —
            // narrowing there needs literal-level falsy modelling, kept
            // conservative for now).
            positive: subtract(current, &InferredType::None_),
            negative: current.clone(),
            whole_scope,
        }),
        NarrowingGuardKind::TypeGuard {
            variable,
            guard_type,
            ..
        } => Some(GuardOutcome {
            variable: variable.clone(),
            positive: InferredType::from_annotation(guard_type),
            // PEP 647: TypeGuard narrows the positive branch ONLY.
            negative: current.clone(),
            whole_scope,
        }),
        NarrowingGuardKind::TypeIs {
            variable,
            guard_type,
            ..
        } => {
            let narrowed_to = InferredType::from_annotation(guard_type);
            Some(GuardOutcome {
                variable: variable.clone(),
                // PEP 742: TypeIs narrows BOTH branches.
                positive: intersect(current, &narrowed_to),
                negative: subtract(current, &narrowed_to),
                whole_scope,
            })
        }
        NarrowingGuardKind::Assert { inner } => outcome_for_kind(inner, current, true),
        // Assignment and match narrowing flow through dedicated paths (the
        // bidirectional engine's walrus/assign handling and per-case match
        // environments) rather than a two-branch outcome.
        NarrowingGuardKind::Assignment { .. } | NarrowingGuardKind::Match { .. } => None,
    }
}

/// `x is None` / `x is not None` in both orders of branch polarity.
fn none_outcome(
    variable: &str,
    current: &InferredType,
    is_positive: bool,
    whole_scope: bool,
) -> GuardOutcome {
    let with_none = intersect(current, &InferredType::None_);
    let without_none = subtract(current, &InferredType::None_);
    let (positive, negative) = if is_positive {
        (with_none, without_none)
    } else {
        (without_none, with_none)
    };
    GuardOutcome {
        variable: variable.to_owned(),
        positive,
        negative,
        whole_scope,
    }
}

/// The union of `isinstance`'s class-name arguments as an [`InferredType`].
fn union_of_names(type_names: &[String]) -> InferredType {
    type_names
        .iter()
        .map(|name| InferredType::from_annotation(name))
        .fold(InferredType::Never, InferredType::union)
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::expect_used,
        reason = "test-only unwrapping of guards known to produce outcomes"
    )]

    use super::*;
    use basilisk_resolver::Span;

    fn guard(kind: NarrowingGuardKind) -> NarrowingGuard {
        NarrowingGuard {
            kind,
            span: Span::new(0, 0),
            in_loop: false,
        }
    }

    fn isinstance_guard(variable: &str, names: &[&str]) -> NarrowingGuard {
        guard(NarrowingGuardKind::IsInstance {
            variable: variable.to_owned(),
            type_names: names.iter().map(|n| (*n).to_owned()).collect(),
            if_body_span: Span::new(0, 0),
            else_body_span: None,
        })
    }

    /// [TYPEINF-NARROWING-ISINSTANCE]: positive intersects, negative subtracts.
    #[test]
    fn isinstance_narrows_both_branches() {
        let current = InferredType::Union(vec![InferredType::Int, InferredType::Str]);
        let outcome = guard_outcomes(&isinstance_guard("x", &["int"]), &current)
            .expect("isinstance guard produces an outcome");
        assert_eq!(outcome.positive, InferredType::Int);
        assert_eq!(outcome.negative, InferredType::Str);
        assert!(!outcome.whole_scope);
    }

    /// `isinstance(x, (int, str))` unions the tuple of names.
    #[test]
    fn isinstance_tuple_unions_names() {
        let current = InferredType::Union(vec![
            InferredType::Int,
            InferredType::Str,
            InferredType::None_,
        ]);
        let outcome =
            guard_outcomes(&isinstance_guard("x", &["int", "str"]), &current).expect("outcome");
        assert!(InferredType::Int.is_assignable_to(&outcome.positive));
        assert!(InferredType::Str.is_assignable_to(&outcome.positive));
        assert_eq!(outcome.negative, InferredType::None_);
    }

    /// [TYPEINF-NARROWING-NONE]: `is not None` flips the branches.
    #[test]
    fn is_not_none_flips_polarity() {
        let current = InferredType::Optional(Box::new(InferredType::Int));
        let outcome = guard_outcomes(
            &guard(NarrowingGuardKind::IsNone {
                variable: "x".to_owned(),
                is_positive: false,
                if_body_span: Span::new(0, 0),
                else_body_span: None,
            }),
            &current,
        )
        .expect("outcome");
        assert_eq!(outcome.positive, InferredType::Int);
        assert_eq!(outcome.negative, InferredType::None_);
    }

    /// [TYPEINF-NARROWING-ASSERT]: assert lifts the inner guard whole-scope.
    #[test]
    fn assert_marks_whole_scope() {
        let current = InferredType::Optional(Box::new(InferredType::Int));
        let outcome = guard_outcomes(
            &guard(NarrowingGuardKind::Assert {
                inner: Box::new(NarrowingGuardKind::IsNone {
                    variable: "x".to_owned(),
                    is_positive: false,
                    if_body_span: Span::new(0, 0),
                    else_body_span: None,
                }),
            }),
            &current,
        )
        .expect("outcome");
        assert!(outcome.whole_scope);
        assert_eq!(outcome.positive, InferredType::Int);
    }

    /// PEP 647 vs PEP 742: `TypeGuard` narrows one branch, `TypeIs` both.
    #[test]
    fn typeguard_and_typeis_differ_on_the_negative_branch() {
        let current = InferredType::Union(vec![InferredType::Int, InferredType::Str]);
        let type_guard = guard_outcomes(
            &guard(NarrowingGuardKind::TypeGuard {
                variable: "x".to_owned(),
                guard_type: "int".to_owned(),
                if_body_span: Span::new(0, 0),
                else_body_span: None,
            }),
            &current,
        )
        .expect("outcome");
        assert_eq!(type_guard.positive, InferredType::Int);
        assert_eq!(
            type_guard.negative, current,
            "TypeGuard: negative unchanged"
        );

        let type_is = guard_outcomes(
            &guard(NarrowingGuardKind::TypeIs {
                variable: "x".to_owned(),
                guard_type: "int".to_owned(),
                if_body_span: Span::new(0, 0),
                else_body_span: None,
            }),
            &current,
        )
        .expect("outcome");
        assert_eq!(type_is.positive, InferredType::Int);
        assert_eq!(
            type_is.negative,
            InferredType::Str,
            "TypeIs: negative subtracts"
        );
    }

    /// Guards inside loops do not produce persistent narrowing
    /// ([TYPEINF-NARROWING-SCOPE]).
    #[test]
    fn loop_guards_are_suppressed() {
        let mut in_loop = isinstance_guard("x", &["int"]);
        in_loop.in_loop = true;
        assert!(guard_outcomes(&in_loop, &InferredType::Unknown).is_none());
    }

    /// Truthiness removes `None` positively and never invents negatively.
    #[test]
    fn truthiness_removes_none_only() {
        let current = InferredType::Optional(Box::new(InferredType::Str));
        let outcome = guard_outcomes(
            &guard(NarrowingGuardKind::Truthiness {
                variable: "x".to_owned(),
                if_body_span: Span::new(0, 0),
                else_body_span: None,
            }),
            &current,
        )
        .expect("outcome");
        assert_eq!(outcome.positive, InferredType::Str);
        assert_eq!(outcome.negative, current);
    }
}
