//! Tests for [RESOLV-CANONICAL-BINDING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#RESOLV-CANONICAL-BINDING
//! `tuple[...]` — a bare ellipsis as the only type argument — is not a type.
//!
//! The typing spec allows `tuple[int, ...]` (homogeneous, variadic) and
//! rejects `tuple[...]`. Which class the subscript head denotes is a question
//! about the binding it resolves to, so every spelling of that one class must
//! give the same answer and a DIFFERENT class named `tuple` must give the
//! opposite one.
//!
//! The deleted recogniser compared `n.id.as_str() == "tuple"` on an
//! `Expr::Name`. It therefore missed `builtins.tuple[...]`, `typing.Tuple[...]`
//! and every aliased import, and it wrongly rejected a module's own
//! `class tuple`.

use super::common::resolve_src;

/// Number of invalid-annotation records the resolver produces for `source`.
fn invalid(source: &str) -> Result<usize, Box<dyn std::error::Error>> {
    Ok(resolve_src(source)?.invalid_string_annotations.len())
}

/// The baseline the whole file is measured against.
#[test]
fn a_bare_ellipsis_is_not_a_tuple_type_argument() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(invalid("def foo(x: tuple[...]) -> None:\n    pass\n")?, 1);
    Ok(())
}

/// `tuple[int, ...]` is the variadic homogeneous tuple and is valid.
#[test]
fn a_trailing_ellipsis_after_a_type_is_valid() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        invalid("def foo(x: tuple[int, ...]) -> None:\n    pass\n")?,
        0,
        "`tuple[int, ...]` is the homogeneous variadic tuple, not an error"
    );
    Ok(())
}

/// Whitespace between the head and its subscript is not part of the meaning.
#[test]
fn spacing_before_the_subscript_changes_nothing() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        invalid("def foo(x: tuple [...]) -> None:\n    pass\n")?,
        1,
        "`tuple [...]` and `tuple[...]` are the same annotation"
    );
    Ok(())
}

/// `builtins.tuple` is the very same class as the bare `tuple`.
#[test]
fn a_module_qualified_tuple_is_the_same_class() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        invalid("import builtins\n\ndef foo(x: builtins.tuple[...]) -> None:\n    pass\n")?,
        1,
        "`builtins.tuple` IS `tuple`"
    );
    Ok(())
}

/// An aliased import of the class binds a second name to it.
#[test]
fn an_aliased_tuple_import_is_the_same_class() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        invalid("from builtins import tuple as T\n\ndef foo(x: T[...]) -> None:\n    pass\n")?,
        1,
        "`from builtins import tuple as T` makes `T` the tuple class"
    );
    Ok(())
}

/// PEP 585's deprecated capitalised alias denotes the same class.
#[test]
fn the_typing_tuple_alias_is_the_same_class() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        invalid("from typing import Tuple\n\ndef foo(x: Tuple[...]) -> None:\n    pass\n")?,
        1,
        "`typing.Tuple` is the PEP 585 alias for the same class"
    );
    Ok(())
}

/// A class this module defines is not the builtin, whatever it is named.
#[test]
fn a_user_class_named_tuple_is_not_the_builtin() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        invalid("class tuple: ...\n\ndef foo(x: tuple[...]) -> None:\n    pass\n")?,
        0,
        "this module's own `tuple` is a different class; nothing here says its \
         subscript may not be an ellipsis"
    );
    Ok(())
}

/// The same annotation on an annotated assignment, not only on a parameter.
#[test]
fn an_annotated_assignment_is_checked_too() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(invalid("x: tuple[...]\n")?, 1);
    Ok(())
}
