//! Tests for [TYPEINF-TARGET-CONSTRAINTS] declared-generics solving. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-CONSTRAINTS and
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-CONSTRAINTS.
//!
//! Exercises `basilisk_checker::bidir::generics`: lower/upper/constrained/
//! default/expected-return evidence collection and the deterministic solver
//! — constrained and bounded `TypeVar`s, PEP 696 defaults, `ParamSpec`, and
//! `TypeVarTuple`. Ambiguity is asserted to be *reported*, never guessed.
#![allow(clippy::indexing_slicing, clippy::panic)]

use basilisk_checker::bidir::generics::GenericVarId;
use basilisk_checker::bidir::{
    DeclaredVar, DeclaredVarKind, GenericEnv, Resolution, SolvedValue, VarDefault,
};
use basilisk_checker::types::{InferredType, LiteralValue};

fn typevar(name: &str, bound: Option<InferredType>, constraints: &[InferredType]) -> DeclaredVar {
    DeclaredVar {
        name: name.to_owned(),
        kind: DeclaredVarKind::TypeVar {
            bound,
            constraints: constraints.to_vec(),
        },
        default: None,
    }
}

fn declare(env: &mut GenericEnv, decl: DeclaredVar) -> GenericVarId {
    env.declare(decl)
}

// ── Unconstrained / bounded TypeVars ────────────────────────────────────────

/// The join of argument evidence answers, with deferred generalization: a
/// lone `Literal[1]` stays `Literal[1]`, never eagerly widened to `int`.
#[test]
fn lone_literal_lower_stays_precise() {
    let mut env = GenericEnv::default();
    let t = declare(&mut env, typevar("T", None, &[]));
    env.add_lower(t, InferredType::Literal(LiteralValue::Int(1)));
    assert_eq!(
        env.resolve(t),
        Resolution::Solved(SolvedValue::Type(InferredType::Literal(LiteralValue::Int(
            1
        ))))
    );
}

/// Multiple lowers join into a union covering each of them.
#[test]
fn multiple_lowers_join() {
    let mut env = GenericEnv::default();
    let t = declare(&mut env, typevar("T", None, &[]));
    env.add_lower(t, InferredType::Int);
    env.add_lower(t, InferredType::Str);
    let Resolution::Solved(SolvedValue::Type(join)) = env.resolve(t) else {
        panic!("expected a solved join");
    };
    assert!(InferredType::Int.is_assignable_to(&join));
    assert!(InferredType::Str.is_assignable_to(&join));
}

/// Repeating identical evidence cannot change the answer (dedup on insert).
#[test]
fn duplicate_evidence_is_idempotent() {
    let mut env = GenericEnv::default();
    let t = declare(&mut env, typevar("T", None, &[]));
    env.add_lower(t, InferredType::Int);
    let once = env.resolve(t);
    env.add_lower(t, InferredType::Int);
    assert_eq!(env.resolve(t), once);
}

/// A `bound=` is honoured: evidence within the bound solves, evidence
/// outside it is unsatisfiable with both sides reported.
#[test]
fn declared_bound_is_enforced() {
    let mut env = GenericEnv::default();
    let ok = declare(&mut env, typevar("T", Some(InferredType::Int), &[]));
    env.add_lower(ok, InferredType::Bool);
    assert_eq!(
        env.resolve(ok),
        Resolution::Solved(SolvedValue::Type(InferredType::Bool))
    );

    let bad = declare(&mut env, typevar("U", Some(InferredType::Int), &[]));
    env.add_lower(bad, InferredType::Str);
    assert_eq!(
        env.resolve(bad),
        Resolution::Unsatisfiable {
            actual: InferredType::Str,
            expected: InferredType::Int,
        }
    );
}

/// Expected-return propagation: with no argument evidence, a single demanded
/// upper answers; two distinct demands are reported ambiguous, not merged.
#[test]
fn expected_return_upper_answers_or_reports_ambiguity() {
    let mut env = GenericEnv::default();
    let t = declare(&mut env, typevar("T", None, &[]));
    env.add_upper(t, InferredType::Int);
    assert_eq!(
        env.resolve(t),
        Resolution::Solved(SolvedValue::Type(InferredType::Int))
    );

    let u = declare(&mut env, typevar("U", None, &[]));
    env.add_upper(u, InferredType::Int);
    env.add_upper(u, InferredType::Str);
    assert_eq!(
        env.resolve(u),
        Resolution::Ambiguous {
            candidates: vec![
                SolvedValue::Type(InferredType::Int),
                SolvedValue::Type(InferredType::Str),
            ],
        }
    );
}

/// An upper-only answer still respects the declared bound.
#[test]
fn upper_only_answer_respects_bound() {
    let mut env = GenericEnv::default();
    let t = declare(&mut env, typevar("T", Some(InferredType::Int), &[]));
    env.add_upper(t, InferredType::Str);
    assert_eq!(
        env.resolve(t),
        Resolution::Unsatisfiable {
            actual: InferredType::Str,
            expected: InferredType::Int,
        }
    );
}

/// Lowers must satisfy every demanded upper (argument vs expected-return
/// conflict is unsatisfiable, not silently widened away).
#[test]
fn lower_conflicting_with_upper_is_unsatisfiable() {
    let mut env = GenericEnv::default();
    let t = declare(&mut env, typevar("T", None, &[]));
    env.add_lower(t, InferredType::Str);
    env.add_upper(t, InferredType::Int);
    assert_eq!(
        env.resolve(t),
        Resolution::Unsatisfiable {
            actual: InferredType::Str,
            expected: InferredType::Int,
        }
    );
}

/// No evidence, no default: honestly unsolved — never a guess
/// ([TYPEINF-EXCEEDS-NOUNKNOWN]).
#[test]
fn no_evidence_no_default_is_unsolved() {
    let mut env = GenericEnv::default();
    let t = declare(&mut env, typevar("T", None, &[]));
    assert_eq!(env.resolve(t), Resolution::Unsolved);
}

/// Gradual posture ([TYPEINF-TARGET-GRADUAL]): `Any` evidence satisfies a
/// declared bound through the single assignability authority — removing an
/// annotation upstream never creates a new bound failure here.
#[test]
fn any_evidence_stays_gradual_under_bound() {
    let mut env = GenericEnv::default();
    let t = declare(&mut env, typevar("T", Some(InferredType::Int), &[]));
    env.add_lower(t, InferredType::Any);
    assert_eq!(
        env.resolve(t),
        Resolution::Solved(SolvedValue::Type(InferredType::Any))
    );
}

// ── Value-constrained TypeVars ──────────────────────────────────────────────

/// `TypeVar('T', int, str)` with str-shaped evidence solves to the `str`
/// constraint itself — a literal widens to the constraint, never stays
/// literal and never joins across constraints (typing spec).
#[test]
fn constrained_var_selects_single_constraint() {
    let mut env = GenericEnv::default();
    let t = declare(
        &mut env,
        typevar("T", None, &[InferredType::Int, InferredType::Str]),
    );
    env.add_lower(t, InferredType::Literal(LiteralValue::Str("a".into())));
    assert_eq!(
        env.resolve(t),
        Resolution::Solved(SolvedValue::Type(InferredType::Str))
    );
}

/// Evidence selecting *different* constraints is ambiguous — the solver
/// reports every candidate rather than inventing `int | str`.
#[test]
fn constrained_var_split_selection_is_ambiguous() {
    let mut env = GenericEnv::default();
    let t = declare(
        &mut env,
        typevar("T", None, &[InferredType::Int, InferredType::Str]),
    );
    env.add_lower(t, InferredType::Int);
    env.add_lower(t, InferredType::Str);
    assert_eq!(
        env.resolve(t),
        Resolution::Ambiguous {
            candidates: vec![
                SolvedValue::Type(InferredType::Int),
                SolvedValue::Type(InferredType::Str),
            ],
        }
    );
}

/// Evidence matching no constraint is unsatisfiable against their union.
#[test]
fn constrained_var_unmatched_evidence_is_unsatisfiable() {
    let mut env = GenericEnv::default();
    let t = declare(
        &mut env,
        typevar("T", None, &[InferredType::Int, InferredType::Str]),
    );
    env.add_lower(t, InferredType::Bytes);
    let Resolution::Unsatisfiable { actual, expected } = env.resolve(t) else {
        panic!("expected unsatisfiable");
    };
    assert_eq!(actual, InferredType::Bytes);
    assert!(InferredType::Int.is_assignable_to(&expected));
    assert!(InferredType::Str.is_assignable_to(&expected));
}

/// Expected-return propagation narrows a constrained var: only constraints
/// satisfying the demand remain, and a unique survivor answers.
#[test]
fn constrained_var_narrowed_by_upper() {
    let mut env = GenericEnv::default();
    let t = declare(
        &mut env,
        typevar("T", None, &[InferredType::Int, InferredType::Str]),
    );
    env.add_upper(t, InferredType::Str);
    assert_eq!(
        env.resolve(t),
        Resolution::Solved(SolvedValue::Type(InferredType::Str))
    );
}

/// A selected constraint that violates the demanded upper is unsatisfiable.
#[test]
fn constrained_selection_checked_against_upper() {
    let mut env = GenericEnv::default();
    let t = declare(
        &mut env,
        typevar("T", None, &[InferredType::Int, InferredType::Str]),
    );
    env.add_lower(t, InferredType::Int);
    env.add_upper(t, InferredType::Str);
    assert_eq!(
        env.resolve(t),
        Resolution::Unsatisfiable {
            actual: InferredType::Int,
            expected: InferredType::Str,
        }
    );
}

// ── PEP 696 defaults ────────────────────────────────────────────────────────

/// With no evidence the declared default answers, and is labelled as such.
#[test]
fn typevar_default_used_only_without_evidence() {
    let mut env = GenericEnv::default();
    let t = declare(
        &mut env,
        DeclaredVar {
            name: "T".to_owned(),
            kind: DeclaredVarKind::TypeVar {
                bound: None,
                constraints: Vec::new(),
            },
            default: Some(VarDefault::Type(InferredType::Str)),
        },
    );
    assert_eq!(
        env.resolve(t),
        Resolution::DefaultUsed(SolvedValue::Type(InferredType::Str))
    );

    // Evidence beats the default.
    env.add_lower(t, InferredType::Int);
    assert_eq!(
        env.resolve(t),
        Resolution::Solved(SolvedValue::Type(InferredType::Int))
    );
}

/// `ParamSpec` and `TypeVarTuple` defaults answer in their own shapes,
/// including the gradual `...` `ParamSpec` default.
#[test]
fn paramspec_and_typevartuple_defaults() {
    let mut env = GenericEnv::default();
    let p = declare(
        &mut env,
        DeclaredVar {
            name: "P".to_owned(),
            kind: DeclaredVarKind::ParamSpec,
            default: Some(VarDefault::Params(Some(vec![
                InferredType::Int,
                InferredType::Str,
            ]))),
        },
    );
    assert_eq!(
        env.resolve(p),
        Resolution::DefaultUsed(SolvedValue::Params(Some(vec![
            InferredType::Int,
            InferredType::Str,
        ])))
    );

    let gradual = declare(
        &mut env,
        DeclaredVar {
            name: "Q".to_owned(),
            kind: DeclaredVarKind::ParamSpec,
            default: Some(VarDefault::Params(None)),
        },
    );
    assert_eq!(
        env.resolve(gradual),
        Resolution::DefaultUsed(SolvedValue::Params(None))
    );

    let ts = declare(
        &mut env,
        DeclaredVar {
            name: "Ts".to_owned(),
            kind: DeclaredVarKind::TypeVarTuple,
            default: Some(VarDefault::Elements(vec![
                InferredType::Int,
                InferredType::Str,
            ])),
        },
    );
    assert_eq!(
        env.resolve(ts),
        Resolution::DefaultUsed(SolvedValue::Elements(vec![
            InferredType::Int,
            InferredType::Str,
        ]))
    );
}

// ── ParamSpec captures ──────────────────────────────────────────────────────

/// One captured signature answers; conflicting captures are reported with
/// every candidate — parameter lists are never merged.
#[test]
fn paramspec_capture_and_conflict() {
    let mut env = GenericEnv::default();
    let p = declare(
        &mut env,
        DeclaredVar {
            name: "P".to_owned(),
            kind: DeclaredVarKind::ParamSpec,
            default: None,
        },
    );
    env.add_param_capture(p, Some(vec![InferredType::Int]));
    assert_eq!(
        env.resolve(p),
        Resolution::Solved(SolvedValue::Params(Some(vec![InferredType::Int])))
    );

    env.add_param_capture(p, Some(vec![InferredType::Str, InferredType::Bool]));
    assert_eq!(
        env.resolve(p),
        Resolution::Ambiguous {
            candidates: vec![
                SolvedValue::Params(Some(vec![InferredType::Int])),
                SolvedValue::Params(Some(vec![InferredType::Str, InferredType::Bool])),
            ],
        }
    );
}

/// An uncaptured, defaultless `ParamSpec` is unsolved — never guessed.
#[test]
fn paramspec_without_evidence_is_unsolved() {
    let mut env = GenericEnv::default();
    let p = declare(
        &mut env,
        DeclaredVar {
            name: "P".to_owned(),
            kind: DeclaredVarKind::ParamSpec,
            default: None,
        },
    );
    assert_eq!(env.resolve(p), Resolution::Unsolved);
}

// ── TypeVarTuple captures ───────────────────────────────────────────────────

/// A single capture answers positionally; equal-length captures join
/// elementwise; mixed lengths have no defensible answer and say so.
#[test]
fn typevartuple_captures_join_or_report() {
    let mut env = GenericEnv::default();
    let ts = declare(
        &mut env,
        DeclaredVar {
            name: "Ts".to_owned(),
            kind: DeclaredVarKind::TypeVarTuple,
            default: None,
        },
    );
    env.add_element_capture(ts, vec![InferredType::Int, InferredType::Str]);
    assert_eq!(
        env.resolve(ts),
        Resolution::Solved(SolvedValue::Elements(vec![
            InferredType::Int,
            InferredType::Str,
        ]))
    );

    env.add_element_capture(ts, vec![InferredType::Bool, InferredType::Str]);
    let Resolution::Solved(SolvedValue::Elements(joined)) = env.resolve(ts) else {
        panic!("equal-length captures must join elementwise");
    };
    assert_eq!(joined.len(), 2);
    assert!(InferredType::Int.is_assignable_to(&joined[0]));
    assert!(InferredType::Bool.is_assignable_to(&joined[0]));
    assert_eq!(joined[1], InferredType::Str);

    env.add_element_capture(ts, vec![InferredType::Bytes]);
    assert!(matches!(
        env.resolve(ts),
        Resolution::Ambiguous { candidates } if candidates.len() == 3
    ));
}

// ── Kind discipline and lookup ──────────────────────────────────────────────

/// Evidence of the wrong kind poisons the variable: no candidate is
/// defensible, and the conflict is reported rather than ignored.
#[test]
fn wrong_kind_evidence_reports_empty_ambiguity() {
    let mut env = GenericEnv::default();
    let p = declare(
        &mut env,
        DeclaredVar {
            name: "P".to_owned(),
            kind: DeclaredVarKind::ParamSpec,
            default: None,
        },
    );
    env.add_lower(p, InferredType::Int);
    assert_eq!(
        env.resolve(p),
        Resolution::Ambiguous {
            candidates: Vec::new(),
        }
    );
}

/// `find` answers by declared name and `resolve_all` reports every variable
/// in declaration order.
#[test]
fn find_and_resolve_all_cover_every_declaration() {
    let mut env = GenericEnv::default();
    let t = declare(&mut env, typevar("T", None, &[]));
    let _ = declare(
        &mut env,
        DeclaredVar {
            name: "P".to_owned(),
            kind: DeclaredVarKind::ParamSpec,
            default: None,
        },
    );
    env.add_lower(t, InferredType::Int);

    assert_eq!(env.find("T"), Some(t));
    assert_eq!(env.find("missing"), None);

    let all = env.resolve_all();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].0, "T");
    assert_eq!(
        all[0].1,
        Resolution::Solved(SolvedValue::Type(InferredType::Int))
    );
    assert_eq!(all[1].0, "P");
    assert_eq!(all[1].1, Resolution::Unsolved);
}
