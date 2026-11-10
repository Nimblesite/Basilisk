//! Tests for [RESOLV-CANONICAL-BINDING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#RESOLV-CANONICAL-BINDING
//! The class hierarchy must be keyed on DEFINITION SITE, never on a rendered
//! class name.
//!
//! Every test here is a program whose meaning does not change when the base
//! class is respelled, paired with the answer the checker must give either way.
//! The deleted `class_by_name` / `walk_bases` pair failed all of them: it built
//! `HashMap<&str, &ClassInfo>` from `ClassInfo::name` and looked up
//! `ClassInfo::bases`, a `Vec<String>` the resolver fills with simple names
//! only. So a base reached through an assignment alias missed the class it
//! names, and a dotted base (`other.Movie`) was recorded as its trailing word
//! and collided with every local class spelled the same.
//!
//! These are observable end-to-end: a `TypedDict` subclass inherits its bases'
//! keys, so "is this class a `TypedDict`, and which keys does it have?" decides
//! whether an invalid-key diagnostic exists.

use std::fmt::Write as _;

use super::common::resolve_src;

/// Count of `TypedDict` key violations the resolver records for `source`.
fn key_violations(source: &str) -> Result<usize, Box<dyn std::error::Error>> {
    Ok(resolve_src(source)?.typeddict_key_violations.len())
}

/// Count of `ReadOnly` field violations the resolver records for `source`.
fn readonly_violations(source: &str) -> Result<usize, Box<dyn std::error::Error>> {
    Ok(resolve_src(source)?.readonly_violations.len())
}

// ---------------------------------------------------------------------------
// A base reached through an alias is the same class
// ---------------------------------------------------------------------------

/// `Alias = Movie` binds a second name to ONE class object. A subclass of
/// `Alias` is a subclass of `Movie`, inherits its keys, and is a `TypedDict`.
#[test]
fn a_typeddict_base_reached_through_an_alias_is_still_that_typeddict(
) -> Result<(), Box<dyn std::error::Error>> {
    let direct = "\
from typing import TypedDict

class Movie(TypedDict):
    name: str

class Film(Movie):
    year: int

f: Film
f[\"director\"] = \"x\"
";
    let aliased = "\
from typing import TypedDict

class Movie(TypedDict):
    name: str

Alias = Movie

class Film(Alias):
    year: int

f: Film
f[\"director\"] = \"x\"
";
    assert_eq!(
        key_violations(direct)?,
        1,
        "baseline: `director` is not a key of `Film`"
    );
    assert_eq!(
        key_violations(aliased)?,
        1,
        "`Alias` and `Movie` are the same class. Keying the hierarchy on \
         rendered names loses the alias, `Film` stops being a `TypedDict`, and \
         the invalid key goes unreported."
    );
    Ok(())
}

/// The inherited key must be accepted through the alias too — the rebuild must
/// not answer "TypedDict" while losing the base's schema.
#[test]
fn an_inherited_key_is_valid_through_an_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
from typing import TypedDict

class Movie(TypedDict):
    name: str

Alias = Movie

class Film(Alias):
    year: int

f: Film
f[\"name\"] = \"x\"
f[\"year\"] = 1
";
    assert_eq!(
        key_violations(source)?,
        0,
        "`name` is inherited from `Movie` through `Alias`; both keys are valid"
    );
    Ok(())
}

/// A chain of aliases resolves to the same class as a single hop.
#[test]
fn a_chain_of_aliases_reaches_the_same_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
from typing import TypedDict

class Movie(TypedDict):
    name: str

First = Movie
Second = First
Third = Second

class Film(Third):
    year: int

f: Film
f[\"director\"] = \"x\"
";
    assert_eq!(key_violations(source)?, 1);
    Ok(())
}

// ---------------------------------------------------------------------------
// A dotted base is a DIFFERENT class from a local one spelled the same
// ---------------------------------------------------------------------------

/// `other.Movie` is a class in another module. Recording the base as its
/// trailing word `Movie` makes the local `Movie` its base, which invents a
/// schema — and therefore an invalid-key error — for a class the program never
/// derived from it.
#[test]
fn a_dotted_base_is_not_the_local_class_of_that_name() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
import other
from typing import TypedDict

class Movie(TypedDict):
    name: str

class Film(other.Movie):
    year: int

f: Film
f[\"director\"] = \"x\"
";
    assert_eq!(
        key_violations(source)?,
        0,
        "`other.Movie` is a different class from the local `Movie`. Nothing in \
         this module says `Film` is a `TypedDict`, so nothing may be said about \
         its keys."
    );
    Ok(())
}

/// The same collision at its sharpest: a class whose dotted base shares its own
/// name. The trailing-word reduction made the class its own ancestor.
#[test]
fn a_class_deriving_from_a_dotted_base_of_its_own_name_is_not_its_own_ancestor(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
import other

class Movie(other.Movie):
    name: str

m: Movie
m[\"director\"] = \"x\"
";
    assert_eq!(
        key_violations(source)?,
        0,
        "`class Movie(other.Movie)` derives from a class in `other`, not from \
         itself"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Spelling mutations: identical meaning, identical answer
// ---------------------------------------------------------------------------

/// Whitespace inside the base list changes nothing.
#[test]
fn reformatting_the_base_list_changes_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let compact = "\
from typing import TypedDict

class Movie(TypedDict):
    name: str

class Film(Movie):
    year: int

f: Film
f[\"director\"] = \"x\"
";
    let spread = "\
from typing import TypedDict

class Movie(
    TypedDict,
):
    name: str

class Film  (
      Movie ,
) :
    year: int

f: Film
f[\"director\"] = \"x\"
";
    assert_eq!(key_violations(compact)?, 1);
    assert_eq!(
        key_violations(spread)?,
        1,
        "line breaks and spacing inside a base list are not part of the program's meaning"
    );
    Ok(())
}

/// `TypedDict` imported under an alias is still `typing.TypedDict`.
#[test]
fn an_aliased_typeddict_import_still_declares_a_typeddict() -> Result<(), Box<dyn std::error::Error>>
{
    let source = "\
from typing import TypedDict as TD

class Movie(TD):
    name: str

class Film(Movie):
    year: int

f: Film
f[\"director\"] = \"x\"
";
    assert_eq!(key_violations(source)?, 1);
    Ok(())
}

/// A module-qualified `typing.TypedDict` base is the same declaration.
#[test]
fn a_module_qualified_typeddict_base_declares_a_typeddict() -> Result<(), Box<dyn std::error::Error>>
{
    let source = "\
import typing

class Movie(typing.TypedDict):
    name: str

class Film(Movie):
    year: int

f: Film
f[\"director\"] = \"x\"
";
    assert_eq!(key_violations(source)?, 1);
    Ok(())
}

// ---------------------------------------------------------------------------
// Inherited class-definition keywords (PEP 728 `extra_items`)
// ---------------------------------------------------------------------------

/// A `TypedDict` declared with `extra_items` accepts unknown keys, and so does
/// a subclass of it — including one that reaches it through an alias.
#[test]
fn extra_items_is_inherited_through_an_alias() -> Result<(), Box<dyn std::error::Error>> {
    let closed = "\
from typing import TypedDict

class Base(TypedDict):
    name: str

Alias = Base

class Sub(Alias):
    year: int

s: Sub
s[\"anything\"] = 1
";
    let open = "\
from typing import TypedDict

class Base(TypedDict, extra_items=int):
    name: str

Alias = Base

class Sub(Alias):
    year: int

s: Sub
s[\"anything\"] = 1
";
    assert_eq!(
        key_violations(closed)?,
        1,
        "baseline: without `extra_items`, an unknown key is an error"
    );
    assert_eq!(
        key_violations(open)?,
        0,
        "`extra_items` on the base makes unknown keys legal on the subclass too"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Inherited field qualifiers (PEP 705 `ReadOnly`)
// ---------------------------------------------------------------------------

/// A `ReadOnly` field stays read-only in a subclass that does not redeclare it,
/// including when the base is reached through an alias.
#[test]
fn readonly_is_inherited_through_an_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
from typing import TypedDict, ReadOnly

class Base(TypedDict):
    name: ReadOnly[str]

Alias = Base

class Sub(Alias):
    year: int

s: Sub
s[\"name\"] = \"x\"
";
    assert_eq!(
        readonly_violations(source)?,
        1,
        "`name` is `ReadOnly` on `Base`, so assigning it through `Sub` is an error"
    );
    Ok(())
}

/// Redeclaring the field without `ReadOnly` in the subclass drops the
/// qualifier — the most-derived declaration wins, through the alias as well.
#[test]
fn a_subclass_redeclaration_drops_readonly_through_an_alias(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
from typing import TypedDict, ReadOnly

class Base(TypedDict):
    name: ReadOnly[str]

Alias = Base

class Sub(Alias):
    name: str

s: Sub
s[\"name\"] = \"x\"
";
    assert_eq!(
        readonly_violations(source)?,
        0,
        "`Sub` redeclares `name` without `ReadOnly`, so writing it is allowed"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Termination: depth and cycles
// ---------------------------------------------------------------------------

/// `class C0(TypedDict)` followed by `depth` single-inheritance subclasses.
fn deep_typeddict_chain(depth: usize) -> String {
    let mut src = String::from("from typing import TypedDict\nclass C0(TypedDict):\n    x: int\n");
    for level in 1..=depth {
        let _ = writeln!(src, "class C{level}(C{}):\n    pass", level - 1);
    }
    src
}

/// A 1 000-deep chain resolves without exhausting the stack, and the deepest
/// leaf still inherits the root's schema. Depth costs heap, never stack.
#[test]
fn a_thousand_deep_chain_walks_without_stack_growth() -> Result<(), Box<dyn std::error::Error>> {
    let mut src = deep_typeddict_chain(1_000);
    src.push_str("leaf: C1000\nleaf[\"nope\"] = 1\n");
    assert_eq!(
        key_violations(&src)?,
        1,
        "the 1 000th subclass of a TypedDict is still a TypedDict, and `nope` \
         is not one of its keys"
    );

    let mut valid = deep_typeddict_chain(1_000);
    valid.push_str("leaf: C1000\nleaf[\"x\"] = 1\n");
    assert_eq!(
        key_violations(&valid)?,
        0,
        "`x` is inherited from the root of the chain"
    );
    Ok(())
}

/// A class listing itself twice among its bases terminates (GitHub #398).
#[test]
fn self_referential_bases_terminate() -> Result<(), Box<dyn std::error::Error>> {
    let resolved = resolve_src("class C(C[int], C[bool]):\n    pass\n")?;
    assert!(
        resolved.typeddict_key_violations.is_empty(),
        "a self-referential class is not a TypedDict, and deciding that terminates"
    );
    Ok(())
}

/// Two classes naming each other as bases — the general cycle — terminates.
#[test]
fn mutually_recursive_bases_terminate() -> Result<(), Box<dyn std::error::Error>> {
    let resolved = resolve_src("class A(B):\n    pass\nclass B(A):\n    pass\n")?;
    assert!(resolved.typeddict_key_violations.is_empty());
    Ok(())
}

/// An alias cycle (`A = B; B = A`) terminates rather than looping forever.
#[test]
fn an_alias_cycle_terminates() -> Result<(), Box<dyn std::error::Error>> {
    let resolved = resolve_src("First = Second\nSecond = First\n\nclass Sub(First):\n    pass\n")?;
    assert!(resolved.typeddict_key_violations.is_empty());
    Ok(())
}
