//! Regression obligations for the runtime type-safety operations collected in
//! [GitHub issue #417](https://github.com/Nimblesite/Basilisk/issues/417).
//!
//! These are typing obligations, not coverage exercises. Each test isolates one
//! invalid program and requires the diagnostic family that owns that semantic
//! operation. Missing rule modules are represented by their intended descriptive
//! names so the test remains an honest red pin until the behavior exists.
//!
//! Primary typing basis: [PEP 484](https://peps.python.org/pep-0484/). More
//! specific PEPs are linked on the tests that exercise annotated assignments,
//! context managers, and asynchronous operations.

#[expect(
    dead_code,
    reason = "shared test harness; this target uses only run and assert_rule_count"
)]
mod common;

use common::{assert_rule_count, messages_for, run};

macro_rules! red_pin {
    (
        $(#[$meta:meta])*
        $name:ident,
        $rule:literal,
        $source:literal,
        $obligation:literal
    ) => {
        $(#[$meta])*
        #[test]
        fn $name() -> Result<(), Box<dyn std::error::Error>> {
            let diagnostics = run($source)?;
            assert_rule_count(&diagnostics, $rule, 1, $obligation);
            Ok(())
        }
    };
}

red_pin!(
    /// Regression for [#417 case 1](https://github.com/Nimblesite/Basilisk/issues/417).
    /// [PEP 526](https://peps.python.org/pep-0526/#specification) makes the
    /// annotation the declared type of the subsequent assignment target.
    issue_417_annotated_reassignment_rejects_str_for_int,
    "assignment_compatibility",
    "x: int\nx = \"foo\"\n",
    "a str is not assignable to a variable declared as int"
);

red_pin!(
    /// Regression for [#417 case 2](https://github.com/Nimblesite/Basilisk/issues/417).
    /// See [PEP 484](https://peps.python.org/pep-0484/#callable).
    issue_417_integer_is_not_callable,
    "calls_not_callable",
    "nonsense = 123\nnonsense()\n",
    "an int value does not implement a callable type"
);

red_pin!(
    /// Regression for [#417 case 3](https://github.com/Nimblesite/Basilisk/issues/417).
    /// See [PEP 484](https://peps.python.org/pep-0484/#the-typing-module).
    issue_417_list_item_assignment_checks_element_type,
    "subscript_assignment_compatibility",
    "numbers: list[int] = [1]\nnumbers[0] = \"three\"\n",
    "list[int] accepts only int values through indexed assignment"
);

red_pin!(
    /// Regression for [#417 case 4](https://github.com/Nimblesite/Basilisk/issues/417).
    /// See [PEP 484](https://peps.python.org/pep-0484/#the-typing-module).
    issue_417_dict_item_assignment_checks_key_type,
    "subscript_assignment_compatibility",
    "config: dict[str, int] = {}\nconfig[0] = 3\n",
    "dict[str, int] rejects an int key"
);

red_pin!(
    /// Regression for [#417 case 5](https://github.com/Nimblesite/Basilisk/issues/417).
    /// See [PEP 484](https://peps.python.org/pep-0484/#callable).
    issue_417_method_call_checks_positional_argument_type,
    "calls_argument_type",
    "class C:\n    def square(self, x: int) -> int:\n        return x * x\n\nC().square(\"hello\")\n",
    "a method parameter declared int rejects a str argument"
);

red_pin!(
    /// Regression for [#417 case 6](https://github.com/Nimblesite/Basilisk/issues/417).
    /// See [PEP 484](https://peps.python.org/pep-0484/#callable).
    issue_417_builtin_len_checks_arity,
    "calls_argument_count",
    "len()\n",
    "the resolved builtins.len signature requires one argument"
);

red_pin!(
    /// Regression for [#417 case 7](https://github.com/Nimblesite/Basilisk/issues/417).
    /// See [PEP 484](https://peps.python.org/pep-0484/#the-meaning-of-annotations).
    issue_417_non_none_return_cannot_fall_through,
    "returns_implicit_none",
    "def f() -> int:\n    print(\"hello\")\n",
    "falling off a function body returns None, which is not assignable to int"
);

red_pin!(
    /// Regression for [#417 case 8](https://github.com/Nimblesite/Basilisk/issues/417).
    /// Await requires an awaitable under [PEP 492](https://peps.python.org/pep-0492/#await-expression).
    issue_417_await_requires_awaitable,
    "awaits_awaitable",
    "async def main() -> None:\n    await 1\n",
    "an int is not awaitable"
);

red_pin!(
    /// Regression for [#417 case 9](https://github.com/Nimblesite/Basilisk/issues/417).
    /// See [PEP 484](https://peps.python.org/pep-0484/#the-typing-module).
    issue_417_unpack_requires_iterable,
    "unpacking_iterable",
    "a, b = 1\n",
    "an int cannot be unpacked because it is not iterable"
);

red_pin!(
    /// Regression for [#417 case 10](https://github.com/Nimblesite/Basilisk/issues/417).
    /// See [PEP 484](https://peps.python.org/pep-0484/#the-meaning-of-annotations).
    issue_417_object_rejects_unknown_attribute_assignment,
    "members_unknown",
    "obj = object()\nobj.non_existing = 1\n",
    "object has no writable member named non_existing"
);

red_pin!(
    /// Regression for [#417 case 11](https://github.com/Nimblesite/Basilisk/issues/417).
    /// See [PEP 484](https://peps.python.org/pep-0484/#the-typing-module).
    issue_417_list_index_must_be_integer,
    "subscripts_index_type",
    "numbers: list[int] = []\nnumbers[\"zero\"] = 3\n",
    "list indices must satisfy the resolved __index__ parameter type"
);

red_pin!(
    /// Regression for [#417 case 12](https://github.com/Nimblesite/Basilisk/issues/417).
    /// See [PEP 484](https://peps.python.org/pep-0484/#the-typing-module).
    issue_417_dict_item_assignment_checks_value_type,
    "subscript_assignment_compatibility",
    "config: dict[str, int] = {}\nconfig[\"retries\"] = \"three\"\n",
    "dict[str, int] rejects a str value"
);

red_pin!(
    /// Regression for [#417 case 13](https://github.com/Nimblesite/Basilisk/issues/417).
    /// See [PEP 484](https://peps.python.org/pep-0484/#the-typing-module).
    issue_417_fixed_tuple_unpack_rejects_too_many_values,
    "unpacking_length",
    "a, b = (1, 2, 3)\n",
    "a three-element fixed tuple cannot unpack into two targets"
);

red_pin!(
    /// Regression for [#417 case 14](https://github.com/Nimblesite/Basilisk/issues/417).
    /// See [PEP 484](https://peps.python.org/pep-0484/#the-typing-module).
    issue_417_fixed_tuple_unpack_rejects_too_few_values,
    "unpacking_length",
    "a, b = (1,)\n",
    "a one-element fixed tuple cannot unpack into two targets"
);

red_pin!(
    /// Regression for [#417 case 15](https://github.com/Nimblesite/Basilisk/issues/417).
    /// See [PEP 484](https://peps.python.org/pep-0484/#the-typing-module).
    issue_417_for_loop_requires_iterable,
    "iteration_iterable",
    "nonsense = 123\nfor item in nonsense:\n    pass\n",
    "a for loop cannot iterate over int"
);

red_pin!(
    /// Regression for [#417 case 16](https://github.com/Nimblesite/Basilisk/issues/417).
    /// See [PEP 484](https://peps.python.org/pep-0484/#callable).
    issue_417_typeshed_function_checks_argument_type,
    "calls_argument_type",
    "import json\njson.loads(5)\n",
    "the resolved json.loads overloads reject int"
);

red_pin!(
    /// Regression for [#417 case 17](https://github.com/Nimblesite/Basilisk/issues/417).
    /// See [PEP 484](https://peps.python.org/pep-0484/#callable).
    issue_417_keyword_argument_checks_declared_type,
    "calls_argument_type",
    "def foo(x: int, y: int, *, z: int = 0) -> int:\n    return x * y * z\n\nfoo(1, 2, z=\"hello\")\n",
    "a keyword argument must satisfy its declared parameter type"
);

red_pin!(
    /// Regression for [#417 case 18](https://github.com/Nimblesite/Basilisk/issues/417).
    /// The context-manager protocol is defined by [PEP 343](https://peps.python.org/pep-0343/#specification-the-with-statement).
    issue_417_with_requires_context_manager,
    "context_managers_protocol",
    "class Manager:\n    pass\n\nwith Manager():\n    pass\n",
    "with requires __enter__ and __exit__ on the context expression"
);

red_pin!(
    /// Regression for [#417 case 19](https://github.com/Nimblesite/Basilisk/issues/417).
    /// See [PEP 484](https://peps.python.org/pep-0484/#the-meaning-of-annotations).
    issue_417_indexing_requires_getitem,
    "subscripts_getitem",
    "class NotSubscriptable:\n    pass\n\nvalue = NotSubscriptable()[0]\n",
    "indexed reads require a resolved __getitem__ member"
);

red_pin!(
    /// Regression for [#417 case 20](https://github.com/Nimblesite/Basilisk/issues/417).
    /// See [PEP 484](https://peps.python.org/pep-0484/#the-meaning-of-annotations).
    issue_417_indexed_assignment_requires_setitem,
    "subscripts_setitem",
    "class NoSetitem:\n    pass\n\nvalue = NoSetitem()\nvalue[0] = 0\n",
    "indexed writes require a resolved __setitem__ member"
);

red_pin!(
    /// Regression for [#417 case 21](https://github.com/Nimblesite/Basilisk/issues/417).
    /// [PEP 526](https://peps.python.org/pep-0526/#specification) governs the
    /// declared type that the assignment expression must preserve.
    issue_417_walrus_assignment_checks_declared_type,
    "assignment_compatibility",
    "x: int\n(x := \"three\")\n",
    "a named expression cannot assign str to a variable declared int"
);

red_pin!(
    /// Regression for [#417 case 22](https://github.com/Nimblesite/Basilisk/issues/417).
    /// [PEP 526](https://peps.python.org/pep-0526/#specification) governs
    /// annotated variable declarations.
    issue_417_late_annotation_must_match_existing_binding,
    "assignment_compatibility",
    "x = 1\nx: str\n",
    "a later incompatible annotation cannot silently contradict an int binding"
);

red_pin!(
    /// Regression for [#417 case 23](https://github.com/Nimblesite/Basilisk/issues/417).
    /// See [PEP 484](https://peps.python.org/pep-0484/#the-meaning-of-annotations).
    issue_417_class_binding_cannot_be_replaced_by_int,
    "assignment_compatibility",
    "class C:\n    pass\n\nC = 1\n",
    "a class binding cannot be silently replaced by an int value"
);

red_pin!(
    /// Regression for [#417 case 24](https://github.com/Nimblesite/Basilisk/issues/417).
    /// See [PEP 484](https://peps.python.org/pep-0484/#callable).
    issue_417_builtin_type_checks_arity,
    "calls_argument_count",
    "type()\n",
    "the resolved builtins.type overloads reject zero arguments"
);

red_pin!(
    /// Regression for [#417 case 25](https://github.com/Nimblesite/Basilisk/issues/417).
    /// Variadic parameter annotations are specified by [PEP 484](https://peps.python.org/pep-0484/#arbitrary-argument-lists-and-default-argument-values).
    issue_417_varargs_check_every_argument_type,
    "calls_argument_type",
    "def total(*numbers: int) -> int:\n    return len(numbers)\n\ntotal(1, 2, 3, \"hello\", 5)\n",
    "every argument captured by *numbers must be int"
);

red_pin!(
    /// Regression for [#417 case 26](https://github.com/Nimblesite/Basilisk/issues/417).
    /// Variadic parameter annotations are specified by [PEP 484](https://peps.python.org/pep-0484/#arbitrary-argument-lists-and-default-argument-values).
    issue_417_kwargs_check_every_argument_type,
    "calls_argument_type",
    "def total(**numbers: int) -> int:\n    return len(numbers)\n\ntotal(a=1, b=2, c=3, d=\"hello\", e=5)\n",
    "every value captured by **numbers must be int"
);

red_pin!(
    /// Regression for [#417 case 27](https://github.com/Nimblesite/Basilisk/issues/417).
    /// See [PEP 484](https://peps.python.org/pep-0484/#the-meaning-of-annotations).
    issue_417_read_only_property_rejects_assignment,
    "attributes_readonly",
    "class C:\n    @property\n    def immutable(self) -> str:\n        return \"fixed\"\n\nC().immutable = \"changed\"\n",
    "a property without a setter is read-only"
);

red_pin!(
    /// Regression for [#417 case 28](https://github.com/Nimblesite/Basilisk/issues/417).
    /// Generator annotations are specified by [PEP 484](https://peps.python.org/pep-0484/#annotating-generator-functions-and-coroutines).
    issue_417_yield_from_requires_iterable,
    "iteration_iterable",
    "from typing import Generator\n\ndef generate() -> Generator[None, None, None]:\n    yield from 42\n",
    "yield from requires an iterable expression"
);

red_pin!(
    /// Regression for [#417 case 29](https://github.com/Nimblesite/Basilisk/issues/417).
    /// See [PEP 484](https://peps.python.org/pep-0484/#the-typing-module).
    issue_417_slice_bounds_require_index_values,
    "subscripts_slice_type",
    "def invalid(s: str, start: float, end: float) -> str:\n    return s[start:end]\n",
    "str slice bounds must satisfy the resolved slice index type"
);

red_pin!(
    /// Regression for [#417 case 30](https://github.com/Nimblesite/Basilisk/issues/417).
    /// The async context-manager protocol is specified by [PEP 492](https://peps.python.org/pep-0492/#asynchronous-context-managers-and-async-with).
    issue_417_async_with_requires_async_context_manager,
    "context_managers_async_protocol",
    "class Manager:\n    pass\n\nasync def main() -> None:\n    async with Manager():\n        pass\n",
    "async with requires __aenter__ and __aexit__"
);

// ---------------------------------------------------------------------------
// Implicit None returns — issue #401
// ---------------------------------------------------------------------------

red_pin!(
    /// Regression for [#401](https://github.com/Nimblesite/Basilisk/issues/401).
    /// Under [PEP 484](https://peps.python.org/pep-0484/#the-meaning-of-annotations),
    /// a declared return type is a type-checking contract. Normal fall-through
    /// produces `None`, so it cannot satisfy `-> int`.
    issue_401_full_body_fallthrough_is_implicit_none,
    "returns_implicit_none",
    "def double(value: int) -> int:\n    value * 2\n",
    "a function declared -> int cannot fall off the end and implicitly return None"
);

red_pin!(
    /// Regression for [#401](https://github.com/Nimblesite/Basilisk/issues/401).
    /// [PEP 484](https://peps.python.org/pep-0484/#the-meaning-of-annotations)
    /// requires every reachable return path to satisfy the declared type.
    issue_401_partial_branch_fallthrough_is_implicit_none,
    "returns_implicit_none",
    "def positive(value: int) -> int:\n    if value > 0:\n        return value\n",
    "a reachable branch that falls through implicitly returns None, not int"
);

/// Regression guard for [#401](https://github.com/Nimblesite/Basilisk/issues/401).
/// [PEP 484](https://peps.python.org/pep-0484/#union-types) permits `None`
/// when the declared return type explicitly admits it.
#[test]
fn issue_401_none_admitting_returns_do_not_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Any

def explicit_none() -> None:
    print("ok")

def optional(value: bool) -> int | None:
    if value:
        return 1

def gradual() -> Any:
    print("unknown")
"#;
    let diagnostics = run(source)?;
    assert_rule_count(
        &diagnostics,
        "returns_implicit_none",
        0,
        "fall-through is valid when the declared return type admits None or Any",
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// reveal_type — issue #418
// ---------------------------------------------------------------------------

fn assert_single_reveal_reports(
    source: &str,
    expected_fragment: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(source)?;
    assert_rule_count(
        &diagnostics,
        "directives_reveal_type",
        1,
        "typing.reveal_type must emit the statically inferred type",
    );
    let messages = messages_for(&diagnostics, "directives_reveal_type");
    assert!(
        messages
            .iter()
            .any(|message| message.contains(expected_fragment)),
        "the reveal diagnostic must contain `{expected_fragment}`, got {messages:?}"
    );
    Ok(())
}

/// Regression for [#418](https://github.com/Nimblesite/Basilisk/issues/418).
/// The [typing directive specification](https://typing.python.org/en/latest/spec/directives.html#reveal-type)
/// built on [PEP 484](https://peps.python.org/pep-0484/) requires a diagnostic
/// that reveals the inferred static type.
#[test]
fn issue_418_reveal_type_reports_inferred_builtin_type(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_single_reveal_reports(
        "from typing import reveal_type\nvalue: int = 1\nreveal_type(value)\n",
        "int",
    )
}

/// Regression for [#418](https://github.com/Nimblesite/Basilisk/issues/418).
/// The [typing directive specification](https://typing.python.org/en/latest/spec/directives.html#reveal-type)
/// built on [PEP 484](https://peps.python.org/pep-0484/) applies to the resolved
/// `typing.reveal_type` symbol, not one local spelling.
#[test]
fn issue_418_reveal_type_resolves_aliased_import() -> Result<(), Box<dyn std::error::Error>> {
    assert_single_reveal_reports(
        "from typing import reveal_type as disclose\nvalue: str = \"x\"\ndisclose(value)\n",
        "str",
    )
}

/// Regression for [#418](https://github.com/Nimblesite/Basilisk/issues/418).
/// Qualified access to the same [typing directive](https://typing.python.org/en/latest/spec/directives.html#reveal-type),
/// part of the type system founded by [PEP 484](https://peps.python.org/pep-0484/),
/// must have the same behavior as a direct import.
#[test]
fn issue_418_reveal_type_resolves_qualified_import() -> Result<(), Box<dyn std::error::Error>> {
    assert_single_reveal_reports(
        "import typing as t\nvalue: bytes = b\"x\"\nt.reveal_type(value)\n",
        "bytes",
    )
}

// ---------------------------------------------------------------------------
// Generic call substitution — issue #419
// ---------------------------------------------------------------------------

fn assert_generic_assert_type_pair(source: &str) -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(source)?;
    assert_rule_count(
        &diagnostics,
        "directives_assert_type_2",
        1,
        "generic call substitution must accept the exact substituted result and reject the deliberately wrong expected type",
    );
    Ok(())
}

/// Regression for [#419](https://github.com/Nimblesite/Basilisk/issues/419).
/// [PEP 695](https://peps.python.org/pep-0695/#generic-functions) requires a
/// generic function call to specialize every occurrence of its inferred type
/// parameter in the return type.
#[test]
fn issue_419_substitutes_type_parameter_inside_tuple_return(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_generic_assert_type_pair(
        r#"
from typing import assert_type

class P:
    pass

def duplicate[T](value: T) -> tuple[T, T]:
    return value, value

assert_type(duplicate(P()), tuple[P, P])
assert_type(duplicate(P()), tuple[str, str])
"#,
    )
}

/// Regression for [#419](https://github.com/Nimblesite/Basilisk/issues/419).
/// [PEP 695](https://peps.python.org/pep-0695/#generic-functions) substitution
/// applies recursively inside generic return annotations.
#[test]
fn issue_419_substitutes_type_parameter_inside_list_return(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_generic_assert_type_pair(
        r#"
from typing import assert_type

def singleton[T](value: T) -> list[T]:
    return [value]

assert_type(singleton(1), list[int])
assert_type(singleton(1), list[str])
"#,
    )
}

/// Regression for [#419](https://github.com/Nimblesite/Basilisk/issues/419).
/// [PEP 695](https://peps.python.org/pep-0695/#generic-functions) substitution
/// applies to every matching arm of a union return type.
#[test]
fn issue_419_substitutes_type_parameter_inside_union_return(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_generic_assert_type_pair(
        r#"
from typing import assert_type

def include_zero[T](value: T) -> int | T:
    return value

assert_type(include_zero("x"), int | str)
assert_type(include_zero("x"), int | bytes)
"#,
    )
}

/// Regression for [#419](https://github.com/Nimblesite/Basilisk/issues/419).
/// Type-parameter identity in [PEP 695](https://peps.python.org/pep-0695/#type-parameter-scopes)
/// is semantic and places no capitalization requirement on the identifier.
#[test]
fn issue_419_lowercase_type_parameter_is_substituted(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_generic_assert_type_pair(
        r#"
from typing import assert_type

def identity[t](value: t) -> t:
    return value

assert_type(identity(1), int)
assert_type(identity(1), str)
"#,
    )
}
