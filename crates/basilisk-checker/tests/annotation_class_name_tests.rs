//! Tests for [TYPEINF-ANNOTATION-RESOLUTION]. See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-ANNOTATION-RESOLUTION
//! Covers `annotation_class_name` in `crates/basilisk-checker/src/types_parsing.rs`,
//! the narrow stand-in for the shared `resolve_annotation` entry point that
//! [NARROWPLAN-CHECKLIST] Stage 0.5 will introduce
//! (docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-CHECKLIST).
//!
//! GitHub #388: member lookup used the annotation's raw source text as a
//! class-name key, so `list[int]` matched nothing while bare `list` worked.

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
