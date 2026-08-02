//! Tests for [TYPEINF-ANNOTATION-RESOLUTION]. See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-ANNOTATION-RESOLUTION
//! Covers `crates/basilisk-checker/src/class_naming.rs` — naming the class a
//! type or annotation refers to;
//! the narrow stand-in for the shared `resolve_annotation` entry point that
//! [NARROWPLAN-CHECKLIST] Stage 0.5 will introduce
//! (docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-CHECKLIST).
//!
//! GitHub #388: member lookup used the annotation's raw source text as a
//! class-name key, so `list[int]` matched nothing while bare `list` worked.
//! GitHub #389: the inferred path used a rendered display string instead.
//! GitHub #390: a loop variable binds an ELEMENT of what it iterates.

use basilisk_checker::class_naming::annotation_class_name;

/// Type arguments do not change which class an annotation names.
#[test]
fn subscripted_generic_names_its_base_class() {
    assert_eq!(annotation_class_name("list[int]").as_deref(), Some("list"));
    assert_eq!(
        annotation_class_name("dict[str, int]").as_deref(),
        Some("dict")
    );
    assert_eq!(annotation_class_name("set[bytes]").as_deref(), Some("set"));
    // Nested arguments are still just arguments.
    assert_eq!(
        annotation_class_name("dict[str, list[int]]").as_deref(),
        Some("dict")
    );
    // A distinct class must NOT collapse into a near neighbour: `frozenset`
    // has no `add`, so answering `set` here would suggest members it lacks.
    assert_eq!(
        annotation_class_name("frozenset[int]").as_deref(),
        Some("frozenset")
    );
}

/// A bare name is already the class name.
#[test]
fn bare_name_is_returned_unchanged() {
    assert_eq!(annotation_class_name("int").as_deref(), Some("int"));
    assert_eq!(annotation_class_name("list").as_deref(), Some("list"));
}

/// Case is meaning for a user class, and must survive.
///
/// `InferredType::from_annotation` lowercases its input, which is why the
/// lookup could not simply route through it.
#[test]
fn user_class_case_is_preserved() {
    assert_eq!(annotation_class_name("Model").as_deref(), Some("Model"));
    assert_eq!(
        annotation_class_name("HTTPResponse[bytes]").as_deref(),
        Some("HTTPResponse")
    );
}

/// A qualified spelling names the class at the tail.
#[test]
fn qualified_annotation_names_the_attribute_tail() {
    assert_eq!(
        annotation_class_name("typing.List").as_deref(),
        Some("List")
    );
    assert_eq!(
        annotation_class_name("collections.abc.Sequence[int]").as_deref(),
        Some("Sequence")
    );
}

/// A quoted forward reference resolves to what it quotes.
#[test]
fn forward_reference_resolves_through_the_quotes() {
    assert_eq!(annotation_class_name("\"Model\"").as_deref(), Some("Model"));
    assert_eq!(
        annotation_class_name("'list[int]'").as_deref(),
        Some("list")
    );
}

/// An annotation naming no single class answers `None` rather than guessing.
///
/// A union receiver has no one member set; answering `str` for `str | None`
/// would offer members that are absent half the time.
#[test]
fn ambiguous_annotations_name_no_class() {
    assert_eq!(annotation_class_name("str | None"), None);
    assert_eq!(annotation_class_name("int | str"), None);
    assert_eq!(annotation_class_name(""), None);
    assert_eq!(annotation_class_name("not a type at all"), None);
}

/// `Optional[str]` and `Union[...]` reduce to the typing construct, not to the
/// wrapped class — the caller looks that name up, finds no class, and offers
/// nothing. Pinned so a later change cannot quietly start answering `str`.
#[test]
fn optional_names_the_construct_not_the_wrapped_class() {
    assert_eq!(
        annotation_class_name("Optional[str]").as_deref(),
        Some("Optional")
    );
    assert_eq!(
        annotation_class_name("Union[int, str]").as_deref(),
        Some("Union")
    );
}

// ── Element types and type arguments (GitHub #390) ───────────────────────────

use basilisk_checker::class_naming::{
    annotation_type_argument, class_name_of_type, element_type_of,
};
use basilisk_checker::types::{InferredType, LiteralValue};

/// A loop variable binds an ELEMENT, not the container.
#[test]
fn iterating_a_container_yields_its_element() {
    let ints = InferredType::List(Box::new(InferredType::Int));
    assert_eq!(element_type_of(&ints), Some(InferredType::Int));

    let names = InferredType::Set(Box::new(InferredType::Str));
    assert_eq!(element_type_of(&names), Some(InferredType::Str));

    // `for k in d` walks the KEYS.
    let mapping = InferredType::Dict(Box::new(InferredType::Str), Box::new(InferredType::Int));
    assert_eq!(element_type_of(&mapping), Some(InferredType::Str));

    // Iterating a string yields one-character strings, not characters.
    assert_eq!(element_type_of(&InferredType::Str), Some(InferredType::Str));
    assert_eq!(
        element_type_of(&InferredType::LiteralString),
        Some(InferredType::Str)
    );
}

/// A type whose iteration behaviour lives in its class declaration answers
/// `None` here — the caller resolves it from the stub.
#[test]
fn named_and_non_iterable_types_have_no_structural_element() {
    assert_eq!(element_type_of(&InferredType::Named("range".into())), None);
    assert_eq!(element_type_of(&InferredType::Int), None);
    assert_eq!(element_type_of(&InferredType::Unknown), None);
}

/// The element type is read out of an iteration protocol's return annotation.
#[test]
fn type_argument_is_extracted_from_an_annotation() {
    assert_eq!(
        annotation_type_argument("Iterator[int]").as_deref(),
        Some("int")
    );
    assert_eq!(
        annotation_type_argument("dict[str, int]").as_deref(),
        Some("str"),
        "the FIRST argument, matching what iterating a mapping yields"
    );
    assert_eq!(annotation_type_argument("int"), None);
}

/// A union names a class only when every arm agrees — the case a list of
/// literals produces (`[1, 2, 3]` elements as `Literal[1] | Literal[2] | ...`).
#[test]
fn a_union_names_a_class_only_when_every_arm_agrees() {
    let int_literals = InferredType::Union(vec![
        InferredType::Literal(LiteralValue::Int(1)),
        InferredType::Literal(LiteralValue::Int(2)),
    ]);
    assert_eq!(
        class_name_of_type(&int_literals),
        Some(("int".to_owned(), false))
    );

    let mixed = InferredType::Union(vec![InferredType::Int, InferredType::Str]);
    assert_eq!(
        class_name_of_type(&mixed),
        None,
        "`int | str` is not any single class — offering one arm's members would be a guess"
    );

    // The LiteralString refinement survives only if EVERY arm carries it.
    let all_literal = InferredType::Union(vec![
        InferredType::LiteralString,
        InferredType::LiteralString,
    ]);
    assert_eq!(
        class_name_of_type(&all_literal),
        Some(("str".to_owned(), true))
    );
    let one_dynamic = InferredType::Union(vec![InferredType::LiteralString, InferredType::Str]);
    assert_eq!(
        class_name_of_type(&one_dynamic),
        Some(("str".to_owned(), false)),
        "a dynamic arm makes the whole union non-literal"
    );
}

/// An optional receiver offers nothing: the value may be `None`.
#[test]
fn optional_names_no_class() {
    assert_eq!(
        class_name_of_type(&InferredType::Optional(Box::new(InferredType::Str))),
        None
    );
}
