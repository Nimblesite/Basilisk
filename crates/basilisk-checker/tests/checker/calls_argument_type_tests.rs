//! Tests for [`calls_argument_type`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for calls_argument_type: Argument type mismatch at call site.

use super::common::*;

#[test]
fn str_literal_for_int_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def add(x: int, y: int) -> int:
    return x + y

result: int = add("hello", "world")
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"calls_argument_type"),
        "str literal for int param should fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn correct_arg_types_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def add(x: int, y: int) -> int:
    return x + y

result: int = add(1, 2)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"calls_argument_type"),
        "correct arg types should not fire E0012"
    );
    Ok(())
}

#[test]
fn bound_method_does_not_collide_with_same_named_function() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r#"
def consume(value: int) -> None:
    pass

class Box:
    def consume(self, value: str) -> None:
        pass

box: Box = Box()
box.consume("valid method argument")
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"calls_argument_type"),
        "a bound method must not be checked against a same-named module function"
    );
    Ok(())
}

#[test]
fn int_literal_for_str_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def greet(name: str) -> str:
    return name

result: str = greet(42)
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"calls_argument_type"),
        "int literal for str param should fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn float_literal_for_int_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def count(n: int) -> int:
    return n

result: int = count(3.14)
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"calls_argument_type"),
        "float literal for int param should fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn bytes_literal_for_str_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def process(data: str) -> str:
    return data

result: str = process(b"hello")
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"calls_argument_type"),
        "bytes literal for str param should fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn str_for_bytes_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def process(data: bytes) -> bytes:
    return data

result: bytes = process("hello")
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"calls_argument_type"),
        "str literal for bytes param should fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn none_for_type_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def register(cls: type) -> None:
    pass

register(None)
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"calls_argument_type"),
        "None for type param should fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn overloaded_function_correct_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import overload

@overload
def parse(data: str) -> str: ...

@overload
def parse(data: int) -> int: ...

def parse(data):
    return data

result: str = parse("hello")
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"calls_argument_type"),
        "correct args for overloaded function should not fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// GitHub #356: `str.join` arguments are judged by the *type* of the argument
/// expression, not by the syntactic shape of the display. A list/tuple whose
/// elements are typed `str` — through a parameter annotation, an unpacked
/// `list[str]`, or a `str` method call — is a valid `Iterable[str]`.
#[test]
fn str_join_accepts_typed_str_elements() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def f(p: list[str], s: str) -> None:
    " ".join(["a", *p])
    " ".join(["a", s])
    " ".join([s.upper()])
    " ".join(("a", s))
    " ".join(p)
    " ".join(["a", "b"])
    " ".join([f"{s}!"])
"#;
    let diags = run(source)?;
    assert_eq!(
        messages_for(&diags, "calls_argument_type"),
        Vec::<&str>::new(),
        "list/tuple displays of `str`-typed elements are valid `Iterable[str]` arguments"
    );
    Ok(())
}

/// GitHub #356: the same type-directed check catches the genuine error a
/// syntactic check missed — a bare name whose declared type is `list[int]`.
#[test]
fn str_join_rejects_list_int_argument() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def g(nums: list[int]) -> None:
    " ".join(nums)
"#;
    let diags = run(source)?;
    let messages = messages_for(&diags, "calls_argument_type");
    assert_eq!(
        messages.len(),
        1,
        "`list[int]` does not satisfy either `join` overload, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// GitHub #356: the received type renders as Python source, never as a `Debug`
/// dump of the resolver's internal expression-kind enum.
#[test]
fn str_join_mismatch_message_renders_python_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def g(nums: list[int]) -> None:
    " ".join(nums)
"#;
    let diags = run(source)?;
    let messages = messages_for(&diags, "calls_argument_type");
    let message = messages
        .first()
        .ok_or("`join` on a `list[int]` must be reported")?;
    assert!(
        message.contains("`list[int]`"),
        "the message must name the argument's Python type, got: {message}"
    );
    assert!(
        !message.contains("StrLiteral")
            && !message.contains("IntLiteral")
            && !message.contains("Other"),
        "internal `RhsKind` variants must never reach the user, got: {message}"
    );
    Ok(())
}

/// GitHub #356 negative control: a non-`str` element literal is still rejected.
#[test]
fn str_join_rejects_int_element_literals() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def h() -> None:
    " ".join([1, 2])
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"calls_argument_type"),
        "`list[int]` display must still violate both `join` overloads, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// GitHub #356: iterating a `dict` yields its keys, and iterating a `str`
/// yields strings — both are valid `join` arguments when the element is a `str`.
#[test]
fn str_join_judges_iterated_element_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def f(mapping: dict[str, int], s: str, values: set[str]) -> None:
    " ".join(mapping)
    " ".join(s)
    " ".join(values)
    " ".join({s, "a"})
    " ".join([])
"#;
    let diags = run(source)?;
    assert_eq!(
        messages_for(&diags, "calls_argument_type"),
        Vec::<&str>::new(),
        "`dict[str, _]`, `str` and `set[str]` all iterate as `Iterable[str]`"
    );
    Ok(())
}

/// GitHub #356: a `dict` whose keys are not `str` fails the same check that
/// lets a `dict[str, int]` through.
#[test]
fn str_join_rejects_non_str_dict_keys() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def f(mapping: dict[int, str]) -> None:
    " ".join(mapping)
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"calls_argument_type"),
        "iterating `dict[int, str]` yields `int` keys, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// GitHub #356: a module-level annotated variable supplies the argument type
/// just as a parameter annotation does.
#[test]
fn str_join_resolves_module_level_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
counts: list[int] = []
joined = " ".join(counts)
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"calls_argument_type"),
        "the module-level `list[int]` annotation must reach the call, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// GitHub #356: a class-level annotation binds an attribute, not a name a
/// method body can read, so it must never type a bare name inside the method.
#[test]
fn str_join_ignores_class_level_annotations() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Report:
    rows: list[int] = []

    def render(self, rows: list[str]) -> None:
        " ".join(rows)
"#;
    let diags = run(source)?;
    assert_eq!(
        messages_for(&diags, "calls_argument_type"),
        Vec::<&str>::new(),
        "`rows` in the method body is the `list[str]` parameter, not the class attribute"
    );
    Ok(())
}

/// GitHub #356: an inner function's parameter shadows the outer one.
#[test]
fn str_join_prefers_the_innermost_declaration() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def outer(values: list[int]) -> None:
    def inner(values: list[str]) -> None:
        " ".join(values)
"#;
    let diags = run(source)?;
    assert_eq!(
        messages_for(&diags, "calls_argument_type"),
        Vec::<&str>::new(),
        "the inner `list[str]` parameter shadows the outer `list[int]` one"
    );
    Ok(())
}

/// GitHub #356: name resolution is scope-aware — a parameter of an unrelated
/// function must never supply the type for a same-named parameter here.
#[test]
fn str_join_resolves_names_in_their_own_scope() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def first(values: list[int]) -> None:
    pass

def second(values: list[str]) -> None:
    " ".join(values)
"#;
    let diags = run(source)?;
    assert_eq!(
        messages_for(&diags, "calls_argument_type"),
        Vec::<&str>::new(),
        "`values` must resolve to the enclosing function's `list[str]` parameter"
    );
    Ok(())
}

#[test]
fn multiple_params_mixed_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def multi(a: int, b: str, c: float) -> None:
    pass

multi(1, 2, 3)
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"calls_argument_type"),
        "int for str param should fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
