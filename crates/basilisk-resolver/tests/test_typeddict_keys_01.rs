#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Tests for resolver: `test_typeddict_keys_01`.

mod common;

use common::resolve_src;

#[test]
fn typeddict_key_violation_invalid_subscript_key() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "    year: int\n",
        "movie: Movie = {'name': 'x', 'year': 1}\n",
        "movie['invalid_key'] = 'test'\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.typeddict_key_violations.is_empty(),
        "subscript with invalid key must produce a violation"
    );
    Ok(())
}

#[test]
fn typeddict_key_violation_invalid_dict_literal() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "    year: int\n",
        "movie: Movie = {'name': 'test', 'invalid': 'val'}\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.typeddict_key_violations.is_empty(),
        "dict literal with invalid key must produce a violation"
    );
    Ok(())
}

#[test]
fn typeddict_key_violation_missing_required_key() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "    year: int\n",
        "movie: Movie = {'name': 'test'}\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.typeddict_key_violations.is_empty(),
        "dict literal missing required key must produce a violation"
    );
    Ok(())
}

#[test]
fn typeddict_key_violation_subscript_read_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "movie: Movie = {'name': 'x'}\n",
        "x = movie['invalid_key']\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.typeddict_key_violations.is_empty(),
        "reading with invalid key must produce a violation"
    );
    Ok(())
}

#[test]
fn typeddict_disallowed_method_call() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "movie: Movie = {'name': 'x'}\n",
        "movie.clear()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.typeddict_key_violations.is_empty(),
        "calling clear() on TypedDict must produce a violation"
    );
    Ok(())
}

#[test]
fn typeddict_key_violation_non_literal_dict_key() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "key = 'name'\n",
        "def process() -> None:\n",
        "    movie: Movie = {key: 'test'}\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.typeddict_key_violations.is_empty(),
        "non-literal key in TypedDict dict must produce a violation"
    );
    Ok(())
}

#[test]
fn typeddict_valid_keys_no_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "    year: int\n",
        "movie: Movie = {'name': 'test', 'year': 2024}\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.typeddict_key_violations.is_empty(),
        "valid dict literal must not produce violations"
    );
    Ok(())
}

#[test]
fn typeddict_delete_subscript_total() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "movie: Movie = {'name': 'x'}\n",
        "del movie['name']\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.typeddict_key_violations.is_empty(),
        "del on total TypedDict subscript must produce a violation"
    );
    Ok(())
}

#[test]
fn typeddict_wrong_value_type_subscript_assign() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "    year: int\n",
        "movie: Movie = {'name': 'x', 'year': 1}\n",
        "movie['year'] = 'not_int'\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.typeddict_key_violations.is_empty(),
        "assigning wrong type to TypedDict field must produce a violation"
    );
    Ok(())
}

#[test]
fn typeddict_wrong_value_type_regular_assign() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "    year: int\n",
        "movie: Movie = {'name': 'x', 'year': 1}\n",
        "movie = {'name': 'test', 'year': 'wrong'}\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.typeddict_key_violations.is_empty(),
        "wrong value type in regular dict assign must produce a violation"
    );
    Ok(())
}

#[test]
fn typeddict_regular_assign_invalid_key() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "def process() -> None:\n",
        "    movie: Movie = {'name': 'test'}\n",
        "    movie = {'bad_key': 'test'}\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.typeddict_key_violations.is_empty());
    Ok(())
}

#[test]
fn typeddict_key_violation_in_function_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "def process() -> None:\n",
        "    movie: Movie = {'name': 'test'}\n",
        "    movie['invalid'] = 'bad'\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.typeddict_key_violations.is_empty());
    Ok(())
}

#[test]
fn typeddict_pop_method_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "movie: Movie = {'name': 'x'}\n",
        "movie.pop('name')\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.typeddict_key_violations.is_empty());
    Ok(())
}

#[test]
fn typeddict_update_method_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "movie: Movie = {'name': 'x'}\n",
        "movie.update({'name': 'y'})\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.typeddict_key_violations.is_empty());
    Ok(())
}

#[test]
fn generator_invalid_return_type_detected() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def gen() -> int:\n    yield 1\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.generator_violations.is_empty());
    Ok(())
}

#[test]
fn async_generator_invalid_return_type() -> Result<(), Box<dyn std::error::Error>> {
    let src = "async def gen() -> int:\n    yield 1\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.generator_violations.is_empty());
    Ok(())
}

#[test]
fn async_generator_with_async_generator_return_no_violation(
) -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import AsyncGenerator\nasync def gen() -> AsyncGenerator[int, None]:\n    yield 1\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.generator_violations.is_empty());
    Ok(())
}

#[test]
fn generator_no_return_annotation_no_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def gen():\n    yield 1\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.generator_violations.is_empty());
    Ok(())
}

#[test]
fn generator_user_defined_type_no_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def gen() -> MyCustom:\n    yield 1\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.generator_violations.is_empty());
    Ok(())
}

#[test]
fn protocol_instantiation_in_with_block() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import Protocol\nclass P(Protocol):\n    def m(self) -> None: ...\nwith open('f') as fh:\n    P()\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.protocol_instantiation_violations.is_empty());
    Ok(())
}

#[test]
fn protocol_instantiation_in_try_block() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import Protocol\nclass P(Protocol):\n    def m(self) -> None: ...\ntry:\n    P()\nexcept Exception:\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.protocol_instantiation_violations.is_empty());
    Ok(())
}

#[test]
fn protocol_instantiation_in_except_handler() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import Protocol\nclass P(Protocol):\n    def m(self) -> None: ...\ntry:\n    pass\nexcept Exception:\n    P()\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.protocol_instantiation_violations.is_empty());
    Ok(())
}

#[test]
fn protocol_instantiation_in_finally_block() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import Protocol\nclass P(Protocol):\n    def m(self) -> None: ...\ntry:\n    pass\nexcept Exception:\n    pass\nfinally:\n    P()\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.protocol_instantiation_violations.is_empty());
    Ok(())
}

#[test]
fn protocol_instantiation_in_orelse_block() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import Protocol\nclass P(Protocol):\n    def m(self) -> None: ...\ntry:\n    pass\nexcept Exception:\n    pass\nelse:\n    P()\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.protocol_instantiation_violations.is_empty());
    Ok(())
}

#[test]
fn protocol_instantiation_subscript_call() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "from typing import Protocol\nclass P(Protocol):\n    def m(self) -> None: ...\nP[int]()\n"
            .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.protocol_instantiation_violations.is_empty());
    Ok(())
}

#[test]
fn final_instance_augmented_assign_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import Final\nclass C:\n    x: Final[int] = 10\n    def modify(self) -> None:\n        self.x += 1\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.final_violations.is_empty());
    Ok(())
}

#[test]
fn unconditional_self_assigns_in_if_else() -> Result<(), Box<dyn std::error::Error>> {
    let src = "class C:\n    def __init__(self, c: bool) -> None:\n        if c:\n            self.x = 1\n        else:\n            self.x = 2\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.classes.iter().any(|c| c.name == "C"));
    Ok(())
}

#[test]
fn typeddict_subscript_read_in_binop() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class TD(TypedDict):\n",
        "    x: int\n",
        "def foo(td: TD) -> int:\n",
        "    return td[\"x\"] + 1\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.functions.len() == 1);
    Ok(())
}

#[test]
fn typeddict_subscript_read_in_call_arg() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class TD(TypedDict):\n",
        "    x: int\n",
        "def foo(td: TD) -> None:\n",
        "    print(td[\"x\"])\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.functions.len() == 1);
    Ok(())
}

#[test]
fn assert_type_with_literal_args() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import assert_type\ndef foo() -> None:\n    assert_type(42, int)\n    assert_type('hello', str)\n    assert_type(True, bool)\n    assert_type(b'x', bytes)\n    assert_type(None, None)\n    assert_type(3.14, float)\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.assert_type_calls.is_empty());
    Ok(())
}

#[test]
fn pep695_outer_typevar_in_constraint_tuple() -> Result<(), Box<dyn std::error::Error>> {
    let src = "class Outer[V]:\n    class Inner[T: (list[V], str)]:\n        pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.pep695_bound_violations.is_empty());
    Ok(())
}

#[test]
fn pep695_outer_typevar_in_binop_bound() -> Result<(), Box<dyn std::error::Error>> {
    let src = "class Outer[V]:\n    class Inner[T: V | str]:\n        pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.pep695_bound_violations.is_empty());
    Ok(())
}

#[test]
fn string_refs_from_tuple_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import Union\nx: Union[\"int\", \"str\"]\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.module_vars.is_empty());
    Ok(())
}

#[test]
fn string_refs_from_binop_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import TypeAlias\nX: TypeAlias = \"int\" | \"str\"\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.module_vars.is_empty());
    Ok(())
}

#[test]
fn return_name_refs_simple_name() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: int) -> int:\n    return x\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.functions[0]
        .return_name_refs
        .iter()
        .any(|(n, _)| n == "x"));
    Ok(())
}

#[test]
fn return_name_refs_collected_for_complex() -> Result<(), Box<dyn std::error::Error>> {
    // return_name_refs tracks all name references in return expressions,
    // including those inside complex expressions (binop, call, subscript, etc.)
    // per Python LEGB scoping: names must be resolved in ALL expressions.
    let src = "def foo(x: int, y: int) -> int:\n    return x + y\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.functions[0]
        .return_name_refs
        .iter()
        .any(|(n, _)| n == "x"));
    assert!(resolved.functions[0]
        .return_name_refs
        .iter()
        .any(|(n, _)| n == "y"));
    Ok(())
}

#[test]
fn return_name_refs_in_if_branch() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo(x: int) -> int:\n",
        "    if x > 0:\n",
        "        return x\n",
        "    return x\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.functions[0].return_name_refs.len() >= 2);
    Ok(())
}

#[test]
fn types_match_quoted_forward_ref() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "from typing import assert_type\ndef foo(x: \"int\") -> None:\n    assert_type(x, int)\n"
            .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.assert_type_calls.is_empty());
    Ok(())
}
