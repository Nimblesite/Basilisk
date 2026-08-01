//! Tests for [TYPEINF-SUBTYPING] shared subtyping context. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-SUBTYPING and
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-SUBTYPING.
//!
//! Exercises `basilisk_checker::subtyping`: the nominal walk, structural
//! Protocol satisfaction, `TypedDict` schemas, declared variance, and
//! `Callable` kinds — plus the parity table pinning the numeric-tower core
//! the rule-local helpers now delegate to, and the `Any`/`Unknown` gradual
//! and numeric-tower consistency between the annotation-text layer and
//! [`InferredType::is_assignable_to`] ([TYPEINF-SUBTYPING-IMPL]).

use std::collections::HashMap;

use basilisk_checker::subtyping::{name_subtype, SubtypingContext, TypedDictField, Variance};
use basilisk_checker::types::InferredType;

fn field(ty: &str, required: bool, read_only: bool) -> TypedDictField {
    TypedDictField {
        ty: ty.to_owned(),
        required,
        read_only,
    }
}

// ── The shared tower core (parity pin for delegating rule helpers) ──────────

/// Pins the exact accepted/rejected table of `name_subtype` — the body the
/// rule-local helpers in `narrowing_typeis`, `generics_syntax_scoping`,
/// `callables_subtyping`, `aliases_implicit`, and
/// `generics_defaults_referential` now delegate to
/// ([NARROWPLAN-SUBTYPING]). A drift here is a drift in every one of them.
#[test]
fn tower_parity_table() {
    let accepted = [
        ("int", "int"),
        ("str", "str"),
        ("MyClass", "MyClass"),
        ("bool", "int"),
        ("bool", "float"),
        ("bool", "complex"),
        ("int", "float"),
        ("int", "complex"),
        ("float", "complex"),
    ];
    let rejected = [
        ("int", "bool"),
        ("float", "int"),
        ("complex", "float"),
        ("complex", "int"),
        ("str", "int"),
        ("int", "str"),
        ("float", "bool"),
    ];
    for (sub, sup) in accepted {
        assert!(name_subtype(sub, sup), "{sub} <: {sup} must hold");
    }
    for (sub, sup) in rejected {
        assert!(!name_subtype(sub, sup), "{sub} <: {sup} must NOT hold");
    }
}

// ── Gradual + numeric-tower consistency across layers ───────────────────────

/// [TYPEINF-TARGET-GRADUAL]: `Any` and `Unknown` are bidirectionally
/// compatible at the `InferredType` layer, and the context accepts `Any`
/// on either side and `object` as top — no layer ever turns gradual
/// tolerance into an error.
#[test]
fn gradual_any_unknown_consistency() {
    for gradual in [InferredType::Any, InferredType::Unknown] {
        assert!(gradual.is_assignable_to(&InferredType::Int));
        assert!(InferredType::Int.is_assignable_to(&gradual));
    }
    let ctx = SubtypingContext::default();
    assert!(ctx.is_subtype("Any", "int"));
    assert!(ctx.is_subtype("int", "Any"));
    assert!(ctx.is_subtype("SomeClass", "object"));
}

/// The numeric tower answers the SAME way at the annotation-text layer and
/// the `InferredType` layer wherever both define the relation
/// ([TYPEINF-SUBTYPING-NOMINAL]; `complex` exists only at the text layer —
/// the annotation parser folds it to `Float`, the documented trade-off).
#[test]
fn tower_agrees_across_layers() {
    let pairs = [
        ("bool", InferredType::Bool, "int", InferredType::Int),
        ("bool", InferredType::Bool, "float", InferredType::Float),
        ("int", InferredType::Int, "float", InferredType::Float),
        ("float", InferredType::Float, "int", InferredType::Int),
        ("int", InferredType::Int, "str", InferredType::Str),
        ("str", InferredType::Str, "int", InferredType::Int),
    ];
    for (sub_name, sub_ty, sup_name, sup_ty) in pairs {
        assert_eq!(
            name_subtype(sub_name, sup_name),
            sub_ty.is_assignable_to(&sup_ty),
            "layers disagree on {sub_name} <: {sup_name}"
        );
    }
}

// ── Nominal relationships ───────────────────────────────────────────────────

/// The nominal walk follows registered bases transitively, in both the
/// direct query and the central `is_subtype` entry, and never invents an
/// edge backwards ([TYPEINF-SUBTYPING-NOMINAL]).
#[test]
fn nominal_walk_is_transitive_and_directed() {
    let mut ctx = SubtypingContext::default();
    ctx.register_class("Animal", &[]);
    ctx.register_class("Dog", &["Animal".to_owned()]);
    ctx.register_class("Puppy", &["Dog".to_owned()]);

    assert!(ctx.is_subtype("Dog", "Animal"));
    assert!(ctx.is_subtype("Puppy", "Animal"));
    assert!(!ctx.is_subtype("Animal", "Dog"));
    assert!(!ctx.is_subtype("Cat", "Animal"));
}

/// Cyclic base registrations terminate rather than recursing forever.
#[test]
fn cyclic_bases_terminate() {
    let mut ctx = SubtypingContext::default();
    ctx.register_class("A", &["B".to_owned()]);
    ctx.register_class("B", &["A".to_owned()]);
    assert!(ctx.is_subtype("A", "B"));
    assert!(!ctx.is_subtype("A", "C"));
}

// ── Protocol structural satisfaction ────────────────────────────────────────

/// A class satisfies a Protocol iff it provides every member (own or
/// inherited) with a compatible type — no inheritance edge required, and a
/// missing or incompatible member rejects ([TYPEINF-SUBTYPING-PROTOCOL]).
#[test]
fn protocol_satisfaction_is_structural() {
    let mut ctx = SubtypingContext::default();
    ctx.register_class("Drawable", &[]);
    ctx.register_protocol("Drawable");
    ctx.register_member("Drawable", "draw", "Callable");
    ctx.register_member("Drawable", "size", "float");

    ctx.register_class("Base", &[]);
    ctx.register_member("Base", "draw", "Callable");
    ctx.register_class("Circle", &["Base".to_owned()]);
    ctx.register_member("Circle", "size", "int");

    // `draw` inherited from Base, `size: int <: float` covariantly.
    assert!(ctx.is_subtype("Circle", "Drawable"));

    ctx.register_class("Square", &[]);
    ctx.register_member("Square", "draw", "Callable");
    // No `size` member at all.
    assert!(!ctx.is_subtype("Square", "Drawable"));

    ctx.register_class("Label", &[]);
    ctx.register_member("Label", "draw", "Callable");
    ctx.register_member("Label", "size", "str");
    // `size: str` is not assignable to `float`.
    assert!(!ctx.is_subtype("Label", "Drawable"));
}

/// A non-Protocol class never accepts structurally, and an empty Protocol
/// (no members registered) never accepts blindly.
#[test]
fn protocol_check_requires_a_registered_protocol() {
    let mut ctx = SubtypingContext::default();
    ctx.register_class("Plain", &[]);
    ctx.register_member("Plain", "draw", "Callable");
    ctx.register_class("Circle", &[]);
    ctx.register_member("Circle", "draw", "Callable");
    assert!(!ctx.is_subtype("Circle", "Plain"));

    ctx.register_class("Empty", &[]);
    ctx.register_protocol("Empty");
    assert!(!ctx.satisfies_protocol("Circle", "Empty"));
}

// ── TypedDict schemas ───────────────────────────────────────────────────────

/// Required target fields must exist; `NotRequired` may be absent;
/// `ReadOnly` fields check covariantly while mutable fields are invariant
/// ([TYPEINF-SUBTYPING-TYPEDDICT]).
#[test]
fn typeddict_schema_compatibility() {
    let mut ctx = SubtypingContext::default();
    ctx.register_typeddict(
        "Movie",
        HashMap::from([
            ("name".to_owned(), field("str", true, false)),
            ("year".to_owned(), field("int", true, false)),
        ]),
    );
    ctx.register_typeddict(
        "MovieBase",
        HashMap::from([("name".to_owned(), field("str", true, false))]),
    );
    ctx.register_typeddict(
        "MovieOptional",
        HashMap::from([
            ("name".to_owned(), field("str", true, false)),
            ("rating".to_owned(), field("float", false, false)),
        ]),
    );
    ctx.register_typeddict(
        "MovieReadOnly",
        HashMap::from([("year".to_owned(), field("float", true, true))]),
    );

    // Extra source fields are fine; missing required ones are not.
    assert!(ctx.is_subtype("Movie", "MovieBase"));
    assert!(!ctx.is_subtype("MovieBase", "Movie"));
    // A NotRequired target field may be absent from the source.
    assert!(ctx.is_subtype("Movie", "MovieOptional"));
    // ReadOnly is covariant: `year: int` satisfies `ReadOnly[float]`…
    assert!(ctx.is_subtype("Movie", "MovieReadOnly"));
    // …but a mutable `float` target needs `float` exactly.
    ctx.register_typeddict(
        "MovieMutable",
        HashMap::from([("year".to_owned(), field("float", true, false))]),
    );
    assert!(!ctx.is_subtype("Movie", "MovieMutable"));
}

// ── Union decomposition through the context ─────────────────────────────────

/// `A <: A | B` and `A | B <: C` iff both alternatives are
/// ([TYPEINF-SUBTYPING-UNION]), including through nominal edges.
#[test]
fn union_decomposition_uses_context() {
    let mut ctx = SubtypingContext::default();
    ctx.register_class("Dog", &["Animal".to_owned()]);
    ctx.register_class("Cat", &["Animal".to_owned()]);

    assert!(ctx.is_subtype("int", "int | str"));
    assert!(ctx.is_subtype("Dog | Cat", "Animal"));
    assert!(!ctx.is_subtype("Dog | int", "Animal"));
}

// ── Declared variance ───────────────────────────────────────────────────────

/// Covariant positions accept subtypes, contravariant accept supertypes,
/// invariant demand equivalence; unregistered classes default to invariant
/// ([TYPEINF-SUBTYPING-GENERIC]).
#[test]
fn variance_drives_generic_argument_checks() {
    let mut ctx = SubtypingContext::default();
    ctx.register_variance("Sequence", vec![Variance::Covariant]);
    ctx.register_variance("Sink", vec![Variance::Contravariant]);

    assert!(ctx.generic_args_compatible("Sequence", &["int"], &["float"]));
    assert!(!ctx.generic_args_compatible("Sequence", &["float"], &["int"]));

    assert!(ctx.generic_args_compatible("Sink", &["float"], &["int"]));
    assert!(!ctx.generic_args_compatible("Sink", &["int"], &["float"]));

    // `list` is unregistered → invariant by default.
    assert!(!ctx.generic_args_compatible("list", &["int"], &["float"]));
    assert!(ctx.generic_args_compatible("list", &["int"], &["int"]));

    // Arity mismatches never pass.
    assert!(!ctx.generic_args_compatible("Sequence", &["int"], &["int", "str"]));
}

// ── Callable kinds ──────────────────────────────────────────────────────────

/// Contravariant parameters, covariant return; the empty parameter list is
/// the gradual `Callable[..., R]` ([TYPEINF-SUBTYPING-CALLABLE]).
#[test]
fn callable_variance_and_gradual_params() {
    let mut ctx = SubtypingContext::default();
    ctx.register_class("Dog", &["Animal".to_owned()]);

    // (Animal) -> Dog  <:  (Dog) -> Animal.
    assert!(ctx.callable_assignable(&["Animal"], "Dog", &["Dog"], "Animal"));
    // (Dog) -> Animal  is NOT  <:  (Animal) -> Dog.
    assert!(!ctx.callable_assignable(&["Dog"], "Animal", &["Animal"], "Dog"));
    // Return covariance alone can reject.
    assert!(!ctx.callable_assignable(&["Animal"], "Animal", &["Animal"], "Dog"));
    // `...` params (empty list) are gradual on either side.
    assert!(ctx.callable_assignable(&[], "Dog", &["int", "str"], "Animal"));
    assert!(ctx.callable_assignable(&["int"], "Dog", &[], "Animal"));
    // Arity mismatches (both sides concrete) reject.
    assert!(!ctx.callable_assignable(&["int"], "Dog", &["int", "str"], "Animal"));
}
