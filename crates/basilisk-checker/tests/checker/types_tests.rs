//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
// Integration tests for the types module: `InferredType`, Display, `from_annotation`, `is_assignable_to`.

// These tests exercise type inference paths through real Python code.

use super::common::*;

#[test]
fn infers_dict_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def func() -> None:
    x: dict[str, int] = {"a": 1}
"#;
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn infers_set_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func() -> None:
    x: set[int] = {1, 2, 3}
";
    let _diags = run(source)?;
    Ok(())
}

// Exercises [TYPEINF-COLLECTIONS-TUPLES] — fixed-length tuple annotation/RHS.
#[test]
fn infers_tuple_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def func() -> None:
    x: tuple[int, str] = (1, "a")
"#;
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn infers_optional_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Optional
def func() -> None:
    x: Optional[int] = None
";
    let _diags = run(source)?;
    Ok(())
}

// Exercises [TYPEINF-SUBTYPING-UNION] — `int | str` union annotation/assignment.
#[test]
fn infers_union_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func() -> None:
    x: int | str = 1
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn infers_callable_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable
def func() -> None:
    f: Callable[[int, str], bool] = lambda x, y: True
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn infers_callable_ellipsis() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable
def func() -> None:
    f: Callable[..., int] = lambda: 42
";
    let _diags = run(source)?;
    Ok(())
}

// Exercises [TYPEINF-SPECIAL-LITERALSTRING].
#[test]
fn infers_literal_string() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import LiteralString
def func(x: LiteralString) -> None:
    pass
";
    let _diags = run(source)?;
    Ok(())
}

// Exercises [TYPEINF-SPECIAL-NEVER] — always-raises body inferred as `Never`.
#[test]
fn infers_never() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Never
def func() -> Never:
    raise RuntimeError()
";
    let _diags = run(source)?;
    Ok(())
}

// Exercises [TYPEINF-SPECIAL-ANY] — explicit `Any` parameter/return.
#[test]
fn infers_any() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Any
def func(x: Any) -> Any:
    return x
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn callable_empty_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable
def func() -> None:
    f: Callable[[], int] = lambda: 42
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn callable_multi_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable
def func() -> None:
    f: Callable[[int, str, float], bool] = lambda x, y, z: True
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn literal_multi_values() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
def func(x: Literal[1, 2, 3]) -> None:
    pass
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn literal_negative_int() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
def func(x: Literal[-1]) -> None:
    pass
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn literal_bool() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
def func(x: Literal[True, False]) -> None:
    pass
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn literal_none() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
def func(x: Literal[None]) -> None:
    pass
";
    let _diags = run(source)?;
    Ok(())
}

// A string literal containing a comma is ONE literal value, not two:
// `split_type_params` must not split inside quotes, and the lone `'` it
// produced made `parse_single_literal` panic on `val[1..0]` (issue #316).
#[test]
fn literal_string_containing_comma_does_not_panic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal

comma: Literal[','] = ','
"#;
    let diags = run(source)?;
    assert!(
        diags.is_empty(),
        "assigning ',' to Literal[','] is valid and must produce no diagnostics, got: {diags:?}"
    );
    Ok(())
}

#[test]
fn literal_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal
def func(x: Literal[b"hello"]) -> None:
    pass
"#;
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn object_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(x: object) -> None:
    pass
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn bytes_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(x: bytes) -> bytes:
    return x
";
    let _diags = run(source)?;
    Ok(())
}

#[test]
fn nested_list_dict() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def func() -> None:
    x: list[dict[str, int]] = [{"a": 1}]
"#;
    let _diags = run(source)?;
    Ok(())
}

// Test int to float widening path.
// Exercises [TYPEINF-SUBTYPING-NOMINAL] — the builtin numeric tower (int <: float).
#[test]
fn int_float_widening() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(x: float) -> None:
    pass

func(42)
";
    let _diags = run(source)?;
    Ok(())
}

// Exercise the return type mismatch via Callable.
// Exercises [TYPEINF-SUBTYPING-CALLABLE] — callable param/return variance.
#[test]
fn callable_return_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable
def apply(f: Callable[[int], str]) -> str:
    return f(1)
";
    let _diags = run(source)?;
    Ok(())
}

// Exercises [TYPEINF-SUBTYPING-NOMINAL] — plain `bool` widens through the
// builtin tower (`bool <: int <: float`, PEP 484), and never the reverse.
// Guards the `(Bool, Int | Float)` arm in `is_assignable_to`.
#[test]
fn bool_widens_through_numeric_tower() {
    use basilisk_checker::types::InferredType;
    assert!(InferredType::Bool.is_assignable_to(&InferredType::Int));
    assert!(InferredType::Bool.is_assignable_to(&InferredType::Float));
    assert!(!InferredType::Int.is_assignable_to(&InferredType::Bool));
    assert!(!InferredType::Float.is_assignable_to(&InferredType::Bool));
}

// Exercises [TYPEINF-SUBTYPING-UNION] — a union containing `None` satisfies an
// `Optional` target: union-LEFT decomposition must run before Optional-target
// unwrapping, else the `None` variant is checked against the unwrapped inner
// type and wrongly fails. Guards the match-arm ordering in `is_assignable_to`.
#[test]
fn union_with_none_satisfies_optional_target() {
    use basilisk_checker::types::InferredType;
    let int_or_none = InferredType::Union(vec![InferredType::Int, InferredType::None_]);
    let optional_int = InferredType::Optional(Box::new(InferredType::Int));
    assert!(int_or_none.is_assignable_to(&optional_int));

    let str_or_none = InferredType::Union(vec![InferredType::Str, InferredType::None_]);
    assert!(!str_or_none.is_assignable_to(&optional_int));
}

// Exercises [TYPEINF-SPECIAL-LITERALSTRING] — only literal-proven strings may
// flow into LiteralString; a dynamic plain `str` must not.
#[test]
fn dynamic_string_is_not_assignable_to_literal_string() {
    use basilisk_checker::types::{InferredType, LiteralValue};

    let literal = InferredType::Literal(LiteralValue::Str("SELECT 1".to_owned()));
    assert!(literal.is_assignable_to(&InferredType::LiteralString));
    assert!(InferredType::LiteralString.is_assignable_to(&InferredType::Str));
    assert!(!InferredType::Str.is_assignable_to(&InferredType::LiteralString));
}

// Exercises [TYPEINF-SPECIAL-LITERALSTRING] — literal expressions retain
// literal-string provenance through the engine's container synthesis.
#[test]
fn string_literal_container_infers_literal_string_elements() {
    use basilisk_checker::expr_type::infer_expression_source;
    use basilisk_checker::types::InferredType;

    let InferredType::List(element) = infer_expression_source(r#"["a", "b"]"#) else {
        panic!("a str-literal list display must synthesize as a list");
    };
    assert!(
        element.is_assignable_to(&InferredType::LiteralString),
        "list elements must keep literal-string provenance, got `{element}`"
    );
}

// Exercises [TYPEINF-SUBTYPING-GENERIC] — mutable built-in containers are
// invariant even when their element types have a scalar subtype relation.
#[test]
fn mutable_builtin_containers_are_invariant() {
    use basilisk_checker::types::InferredType;

    let int_list = InferredType::List(Box::new(InferredType::Int));
    let float_list = InferredType::List(Box::new(InferredType::Float));
    assert!(!int_list.is_assignable_to(&float_list));
    assert!(!float_list.is_assignable_to(&int_list));

    let int_set = InferredType::Set(Box::new(InferredType::Int));
    let float_set = InferredType::Set(Box::new(InferredType::Float));
    assert!(!int_set.is_assignable_to(&float_set));
    assert!(!float_set.is_assignable_to(&int_set));

    let str_int_dict = InferredType::Dict(Box::new(InferredType::Str), Box::new(InferredType::Int));
    let str_float_dict =
        InferredType::Dict(Box::new(InferredType::Str), Box::new(InferredType::Float));
    assert!(!str_int_dict.is_assignable_to(&str_float_dict));
    assert!(!str_float_dict.is_assignable_to(&str_int_dict));

    let any_list = InferredType::List(Box::new(InferredType::Any));
    assert!(any_list.is_assignable_to(&int_list));
    assert!(int_list.is_assignable_to(&any_list));
}

// Exercises [TYPEINF-SUBTYPING-CALLABLE] — a source may require fewer
// parameters than the target when its remaining parameters are optional.
#[test]
fn callable_source_may_have_fewer_required_parameters() {
    use basilisk_checker::types::{CallableInfo, InferredType};

    let source = InferredType::Callable(CallableInfo {
        param_types: vec![InferredType::Int],
        return_type: Box::new(InferredType::Bool),
    });
    let target = InferredType::Callable(CallableInfo {
        param_types: vec![InferredType::Int, InferredType::Str],
        return_type: Box::new(InferredType::Bool),
    });

    assert!(source.is_assignable_to(&target));
    assert!(!target.is_assignable_to(&source));
}
