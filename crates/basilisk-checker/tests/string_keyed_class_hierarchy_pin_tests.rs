//! Pins the STRING-KEYED CLASS HIERARCHY defect — the same defect as the
//! deleted `crate::subtyping`, re-implemented inline across ~20 rules.
//!
//! [`basilisk_resolver::ClassInfo::bases`] is `Vec<String>`: base classes are
//! recorded as RENDERED SIMPLE NAMES, "complex expressions ignored". Rules then
//! look a base up in a `HashMap<&str, &ClassInfo>` keyed on the class's
//! rendered name. Every verdict so produced moves with the SPELLING of the base
//! and not with the class it denotes.
//!
//! `shared/class_walks.rs` documents the wrongness in its own module comment
//! and ships it anyway:
//!
//! > Base names resolve to same-module classes by SIMPLE name, so `class
//! > Client(httpx.Client)` records the base as `Client` and the by-name lookup
//! > makes the class its own ancestor.
//!
//! Each test below is a semantics-preserving respelling of a program the
//! checker gets RIGHT. The checker gets the respelling WRONG. That difference
//! is the defect, and these tests exist to keep it visible until base classes
//! resolve through the binding table.
#![allow(clippy::expect_used, missing_docs)]

// The shared harness carries helpers this binary does not use.
#[allow(dead_code)]
mod common;

use common::run;

/// Diagnostics carrying `code`.
fn codes(source: &str, code: &str) -> usize {
    run(source)
        .expect("checker ran")
        .iter()
        .filter(|d| d.code.code == code)
        .count()
}

/// PEP 591: subclassing a `@final` class is an error. The class reached
/// through a module-level alias IS the same class — one binding, one object —
/// so the diagnostic must not depend on which name the base is written with.
#[test]
fn final_base_reached_through_an_alias_is_still_final() {
    let direct = "\
import typing

@typing.final
class Base: ...

class Sub(Base): ...
";
    let aliased = "\
import typing

@typing.final
class Base: ...

Alias = Base

class Sub(Alias): ...
";
    let direct_count = codes(direct, "qualifiers_final_decorator");
    assert_eq!(
        direct_count, 1,
        "baseline: subclassing a @final class must be reported"
    );
    assert_eq!(
        codes(aliased, "qualifiers_final_decorator"),
        direct_count,
        "`Alias` and `Base` are the SAME class object. The rule looks the base \
         up by its rendered name in a class map keyed on rendered names, so the \
         alias misses and the error vanishes. A @final class must stay final \
         however its name is spelled."
    );
}

/// PEP 589: a `TypedDict` may not mix `TypedDict` and non-`TypedDict` bases.
/// The rule exempts a base whose rendered name is `"object"`. A module that
/// defines its OWN class called `object` gets that exemption for free.
#[test]
fn a_user_class_named_object_is_not_the_builtin_top_type() {
    let source = "\
from typing import TypedDict

class object: ...

class Movie(TypedDict):
    name: str

class Bad(Movie, object): ...
";
    assert_eq!(
        codes(source, "typeddicts_inheritance"),
        1,
        "`object` here is a class this module defines, not `builtins.object`. \
         The rule's `EXEMPT: &[\"object\"]` list matches the SPELLING, so a \
         genuine non-TypedDict base is silently exempted. Whether a base is the \
         top type is a question about the binding it resolves to."
    );
}

/// A class listing itself as a base is an error — the name is not bound until
/// the `class` statement completes. The rule suppresses this whenever the
/// class's name happens to appear in a hard-coded `BUILTINS` spelling list, so
/// the identical program is judged differently depending on the name chosen.
#[test]
fn self_referencing_base_is_an_error_whatever_the_class_is_named() {
    let ordinary = "class Foo(Foo): ...\n";
    let named_like_a_builtin = "class ascii(ascii): ...\n";

    let ordinary_count = codes(ordinary, "names_undefined");
    assert_eq!(
        ordinary_count, 1,
        "baseline: a class cannot list itself as a base"
    );
    assert_eq!(
        codes(named_like_a_builtin, "names_undefined"),
        ordinary_count,
        "`class ascii(ascii)` is the same error as `class Foo(Foo)`: the name is \
         unbound until the statement completes. It is suppressed only because \
         `ascii` appears in a hard-coded BUILTINS whitelist of SPELLINGS. \
         CLAUDE.md: builtins are not an exception — a builtin use resolves \
         through the binding table like every other name."
    );
}

/// A base written as a dotted path is recorded as its trailing simple name, so
/// a class inheriting from an imported class of the same name becomes its own
/// ancestor. Nothing about `httpx.Client` makes `Client` a `@final` class here;
/// the checker must not invent a relationship from a shared trailing word.
#[test]
fn a_dotted_base_is_not_the_same_class_as_a_local_one_of_that_name() {
    let source = "\
import typing

@typing.final
class Client: ...

class Wrapper(Client): ...
";
    // Baseline: the local `Client` really is final, so this really is an error.
    assert_eq!(
        codes(source, "qualifiers_final_decorator"),
        1,
        "baseline: the local final class is subclassed"
    );

    let dotted = "\
import typing
import httpx

@typing.final
class Client: ...

class Wrapper(httpx.Client): ...
";
    assert_eq!(
        codes(dotted, "qualifiers_final_decorator"),
        0,
        "`httpx.Client` is a DIFFERENT class from the local `Client`. The \
         resolver records the base as the trailing name `Client`, so the \
         by-name lookup finds the local final class and invents an error about \
         a class the program never subclasses."
    );
}
