//! Tests for [BSK-E0151] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    dead_code,
    missing_docs
)]

mod common;

use common::{messages_for, run};

#[test]
fn type_alias_type_invalid_forms_emit_e0151() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAliasType, TypeVar

T = TypeVar("T")
S = TypeVar("S")
params = (T,)

BadValue = TypeAliasType("BadValue", [int])
BadCircular = TypeAliasType("BadCircular", "BadCircular")
BadTypeVar = TypeAliasType("BadTypeVar", list[S], type_params=(T,))
BadParams = TypeAliasType("BadParams", int, type_params=params)
GenericPair = TypeAliasType("GenericPair", tuple[T, S], type_params=(T, S))

print(GenericPair.other)
x: GenericPair[int] = (1, 2)
"#;

    let diagnostics = run(source)?;
    let messages = messages_for(&diagnostics, "BSK-E0151");

    assert_eq!(messages.len(), 6, "{messages:#?}");
    assert!(messages
        .iter()
        .any(|message| message.contains("Invalid type expression")));
    assert!(messages
        .iter()
        .any(|message| message.contains("references itself")));
    assert!(messages
        .iter()
        .any(|message| message.contains("Type variable `S`")));
    assert!(messages
        .iter()
        .any(|message| message.contains("must be a literal tuple")));
    assert!(messages
        .iter()
        .any(|message| message.contains("has no attribute `other`")));
    assert!(messages
        .iter()
        .any(|message| message.contains("expected 2, got 1")));
    Ok(())
}

#[test]
fn type_alias_type_valid_forms_do_not_emit_e0151() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeAliasType, TypeVar

T = TypeVar("T")
Good = TypeAliasType("Good", T | "list[Good[T]]", type_params=(T,))

class Box(Generic[T]):
    Inner = TypeAliasType("Inner", list[T])

print(Good.__value__)
print(Good.__type_params__)
print(Good.__name__)
print(Good.__module__)
x: Good[int] = 1
"#;

    let diagnostics = run(source)?;
    let messages = messages_for(&diagnostics, "BSK-E0151");

    assert!(messages.is_empty(), "{messages:#?}");
    Ok(())
}
