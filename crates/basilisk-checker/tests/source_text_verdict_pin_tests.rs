//! Pins two defects that survive on the same mistake: **a verdict computed by
//! reading the program back out of its own source text**, and **a builtin
//! recognised by the five or six characters it is usually spelled with**.
//!
//! `ruff_python_parser` has already turned the file into an AST by the time any
//! rule runs. Every construct these sites re-derive by hand — a tuple literal,
//! an int literal, an annotation, a call to `len` — is a node the parser
//! produced, with a kind the parser decided. Re-deriving it from characters
//! means the answer moves when the source is respelled:
//!
//! * `tuple[int]` is a tuple annotation, `tuple [int]` is not;
//! * `1e3` is read as an `int` because it has no `.` in it;
//! * `x: int = 1` warns, the same assignment wrapped in parentheses does not;
//! * `len(xs)` is `builtins.len`, and so is `len` after `def len(): ...`.
//!
//! Each test below is a semantics-preserving respelling of a program the
//! checker gets RIGHT, or a piece of ordinary Python the character-level
//! reasoning gets WRONG. They stay here until the verdicts come from the AST
//! and the binding table.
//!
//! ## Read this before "fixing" a failure here
//!
//! The tree is mid-cull. Most of these currently fail at a `panic!` left by an
//! earlier deletion — today, dominantly the resolver's name-keyed TypedDict
//! walk — rather than reaching the rule under test and disagreeing with it.
//! That does not make them decoration: they name the defect, and they will
//! become sharp the moment the resolver walk is rebuilt. **Do not delete them,
//! weaken them, or mark them `#[ignore]` to get a green run.**
#![allow(clippy::expect_used, missing_docs)]

// The shared harness carries helpers this binary does not use.
#[allow(dead_code)]
mod common;

use common::run;

/// Every diagnostic the checker emits for `source`, as codes.
fn codes(source: &str) -> Vec<String> {
    run(source)
        .expect("checker ran")
        .iter()
        .map(|d| d.code.code.to_owned())
        .collect()
}

/// Count of diagnostics carrying `code`.
fn count(source: &str, code: &str) -> usize {
    codes(source).iter().filter(|c| c.as_str() == code).count()
}

// ---------------------------------------------------------------------------
// Verdicts re-derived from raw source characters
// ---------------------------------------------------------------------------

/// `assignment_compatibility`'s tuple check recognises a tuple annotation with
/// `ann_text.starts_with("tuple[")`. Python's grammar puts no constraint on
/// whitespace between a subscripted name and its `[`, so `tuple [int]` is the
/// SAME annotation — and `builtins.tuple[int]` is the same type again.
///
/// A checker that reports the arity mismatch in one spelling and stays silent
/// in the others is reporting on formatting.
#[test]
fn a_tuple_annotation_is_the_same_type_however_it_is_spaced() {
    let tight = "\
t: tuple[int] = (1,)
t = (1, 2)
";
    let spaced = "\
t: tuple [int] = (1,)
t = (1, 2)
";
    let dotted = "\
import builtins

t: builtins.tuple[int] = (1,)
t = (1, 2)
";

    let tight_errors = count(tight, "assignment_compatibility");
    assert_eq!(
        count(spaced, "assignment_compatibility"),
        tight_errors,
        "`tuple [int]` is the same annotation as `tuple[int]`; a space before the \
         subscript changes no type. Recognising the annotation with \
         `starts_with(\"tuple[\")` makes whitespace load-bearing."
    );
    assert_eq!(
        count(dotted, "assignment_compatibility"),
        tight_errors,
        "`builtins.tuple[int]` denotes exactly the same type as `tuple[int]`; \
         reaching a builtin through its module must not silence the rule."
    );
}

/// The same rule decides the RHS is a tuple with `text.starts_with('(') &&
/// text.ends_with(')')`, then counts elements by splitting the characters on
/// commas. `(1)` is not a tuple at all — it is the integer `1` in parentheses —
/// and `1, 2` IS a tuple without any parentheses.
///
/// Both are `Expr` nodes with settled kinds (`Expr::Tuple` vs `Expr::NumberLiteral`).
/// Reading them off the punctuation gets both cases backwards.
#[test]
fn parentheses_do_not_make_a_tuple_and_their_absence_does_not_unmake_one() {
    let parenthesised_int = "\
t: tuple[int] = (1,)
t = (1)
";
    let bare_tuple = "\
t: tuple[int] = (1,)
t = 1, 2
";

    assert_eq!(
        count(bare_tuple, "assignment_compatibility"),
        1,
        "`t = 1, 2` is a two-element tuple assigned to `tuple[int]` — an error. \
         Requiring a literal `(` to see a tuple misses every unparenthesised one."
    );
    assert_eq!(
        count(parenthesised_int, "assignment_compatibility"),
        1,
        "`(1)` is the integer 1, not a one-element tuple, so assigning it to \
         `tuple[int]` is an error. Treating any `(...)` text as a tuple literal \
         accepts it."
    );
}

/// `redundant_annotation` (BSK-0050) recovers the RHS literal by finding the
/// line containing the name, locating a `=` in it, and classifying whatever
/// follows. Wrapping the value in parentheses — a semantics-preserving
/// reformat that any formatter may perform — moves the value onto another line
/// and the warning disappears.
#[test]
fn wrapping_a_literal_in_parentheses_changes_no_type() {
    let one_line = "\
class C:
    n: int = 1
";
    let wrapped = "\
class C:
    n: int = (
        1
    )
";

    assert_eq!(
        count(wrapped, "BSK-0050"),
        count(one_line, "BSK-0050"),
        "`(\\n 1 \\n)` is the literal `1`. A rule that reads the assignment off \
         ONE SOURCE LINE reports on line breaks, not on types."
    );
}

/// The same line-scanner extracts the ANNOTATION by searching for `\": \"` and
/// cutting at the first `=`. Neither is part of Python's grammar: the space
/// after the colon is optional, and an `=` may appear inside the annotation
/// itself (a `Literal` string, an `Annotated` payload) long before the one that
/// introduces the value.
#[test]
fn an_annotation_is_not_the_text_between_a_colon_and_an_equals_sign() {
    let spaced = "\
class C:
    n: int = 1
";
    let unspaced = "\
class C:
    n:int = 1
";

    assert_eq!(
        count(unspaced, "BSK-0050"),
        count(spaced, "BSK-0050"),
        "`n:int = 1` and `n: int = 1` are the same declaration; PEP 8 asks for \
         the space, the grammar does not require it. Searching for the literal \
         two characters `\": \"` makes style decide whether the rule runs."
    );
}

/// A literal's type is the kind of node the parser built, not a property of its
/// characters. `1e3` is a `float`; classifying it by "starts with a digit and
/// contains no `.`" calls it an `int`.
#[test]
fn exponent_notation_is_a_float_even_without_a_decimal_point() {
    let source = "\
class C:
    x: int = 1e3
";
    assert_eq!(
        count(source, "BSK-0050"),
        0,
        "`1e3` is a `float`, so annotating it `int` is not a REDUNDANT annotation \
         (it is a wrong one). Deciding the literal's type by scanning for a `.` \
         reports `int`."
    );
}

// ---------------------------------------------------------------------------
// Builtin identity by spelling
// ---------------------------------------------------------------------------

/// `bidir::builtins::builtin_call_return` maps a BARE CALLEE NAME to a return
/// type. Python lets any of those names be rebound; after `def len(...) -> str`
/// the name `len` is the user's function, and the table still answers `int`.
///
/// CLAUDE.md is explicit that builtins are not an exception to binding
/// resolution.
#[test]
fn a_shadowed_builtin_is_not_the_builtin() {
    let source = "\
def len(seq: list[int]) -> str:
    return \"n\"


class C:
    n: str = len([1])
";
    assert_eq!(
        count(source, "BSK-0050"),
        1,
        "the module rebinds `len` to a function returning `str`, so `n: str` \
         genuinely restates the inferred type. A name-keyed builtin table \
         answers `int` and the warning never fires."
    );
}

/// The same table is reached only through `Expr::Name`, so a builtin imported
/// under any other spelling is invisible — even though it is the identical
/// object.
#[test]
fn a_builtin_reached_through_an_alias_is_still_that_builtin() {
    let direct = "\
class C:
    n: int = len([1])
";
    let aliased = "\
from builtins import len as size


class C:
    n: int = size([1])
";
    assert_eq!(
        count(aliased, "BSK-0050"),
        count(direct, "BSK-0050"),
        "`from builtins import len as size` binds `size` to `builtins.len`; the \
         call returns `int` under either name."
    );
}

/// PEP 484 §"The type of class objects": `staticmethod` is a builtin, and a
/// decorator is an arbitrary expression. `protocols_definition_2` recognises it
/// only as `Expr::Name(\"staticmethod\")`, so `@builtins.staticmethod` — an
/// `Expr::Attribute` naming the same object — decorates nothing as far as the
/// rule is concerned, and its first parameter is mistaken for a receiver.
#[test]
fn a_staticmethod_decorator_is_the_same_whether_bare_or_qualified() {
    let bare = "\
from typing import Protocol


class P(Protocol):
    @staticmethod
    def f(x: int) -> int: ...


class Impl:
    @staticmethod
    def f(x: int) -> int:
        return x


def take(p: P) -> None: ...


take(Impl())
";
    let qualified = "\
import builtins
from typing import Protocol


class P(Protocol):
    @builtins.staticmethod
    def f(x: int) -> int: ...


class Impl:
    @builtins.staticmethod
    def f(x: int) -> int:
        return x


def take(p: P) -> None: ...


take(Impl())
";

    assert_eq!(
        codes(qualified).len(),
        codes(bare).len(),
        "`@builtins.staticmethod` and `@staticmethod` are the same decorator \
         applied to the same function; the protocol is satisfied either way."
    );
}

/// `type` is an ordinary builtin name and an ordinary binding. A module that
/// defines its own `type` has not written `builtins.type`, and inference must
/// not treat a call to it as producing a class object.
#[test]
fn a_user_class_named_type_is_not_the_builtin_type() {
    let source = "\
class type:
    def __init__(self, label: str) -> None:
        self.label = label


class C:
    n: str = type(\"x\").label
";
    // The module's own `type` is what `type("x")` calls; its `.label` is `str`.
    assert_eq!(
        count(source, "BSK-0050"),
        1,
        "the module defines `type`, so `type(\"x\")` constructs THAT class and \
         `.label` is `str`. Recognising the builtin by its four characters \
         hijacks the call."
    );
}
