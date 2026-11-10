//! Structural typing against protocols the conformance suite never imports.
//! [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! Every protocol here — `SupportsIndex`, `SupportsFloat`, `SupportsAbs`,
//! `Reversible`, `Container`, `ItemsView` — is outside the 55 typing symbols
//! `conformance/tests/` imports, so no rule can carry a hardcoded arm for it.
//! Identifiers are drawn from a vocabulary disjoint from the suite's 913.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── SupportsIndex ────────────────────────────────────────────────────────
// The typing spec makes `SupportsIndex` a runtime-checkable protocol with a
// single `__index__(self) -> int` member. Structural conformance is decided by
// the member set, never by a nominal relationship.

const INDEX_REJECTED: &str = r#"
from typing import SupportsIndex


class Furlong:
    def __index__(self) -> int:
        return 220


def stride(amount: SupportsIndex) -> int:
    return amount.__index__()


stride(Furlong())
stride("eight")
"#;

const INDEX_ACCEPTED: &str = r#"
from typing import SupportsIndex


class Furlong:
    def __index__(self) -> int:
        return 220


def stride(amount: SupportsIndex) -> int:
    return amount.__index__()


stride(Furlong())
stride(8)
"#;

const INDEX_REJECTED_ALIASED: &str = r#"
from typing import SupportsIndex as HasDunderIndex


class Furlong:
    def __index__(self) -> int:
        return 220


def stride(amount: HasDunderIndex) -> int:
    return amount.__index__()


stride(Furlong())
stride("eight")
"#;

const INDEX_REJECTED_IMPORT_FORM: &str = r#"
import typing


class Furlong:
    def __index__(self) -> int:
        return 220


def stride(amount: typing.SupportsIndex) -> int:
    return amount.__index__()


stride(Furlong())
stride("eight")
"#;

const INDEX_REJECTED_RENAMED: &str = r#"
from typing import SupportsIndex


class Chainage:
    def __index__(self) -> int:
        return 220


def advance(quantity: SupportsIndex) -> int:
    return quantity.__index__()


advance(Chainage())
advance("eight")
"#;

const INDEX_REJECTED_REFORMATTED: &str = "
from typing import SupportsIndex

class Furlong:  # a unit of distance

        def __index__(self) -> int:

                return 220

def stride(
        amount: SupportsIndex,
) -> int:
        return (amount).__index__()

stride(Furlong())
# the defect, one line down
stride('eight')
";

const INDEX_ACCEPTED_ALIASED: &str = r#"
from typing import SupportsIndex as HasDunderIndex


class Furlong:
    def __index__(self) -> int:
        return 220


def stride(amount: HasDunderIndex) -> int:
    return amount.__index__()


stride(Furlong())
stride(8)
"#;

#[test]
fn supports_index_structural_conformance() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`str` has no `__index__`, so it does not implement `SupportsIndex`",
        rejected: INDEX_REJECTED,
        accepted: INDEX_ACCEPTED,
        rejected_variants: &[
            aliased(INDEX_REJECTED_ALIASED),
            import_form(INDEX_REJECTED_IMPORT_FORM),
            renamed(INDEX_REJECTED_RENAMED),
            reformatted(INDEX_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[aliased(INDEX_ACCEPTED_ALIASED)],
    }
    .assert("SupportsIndex structural conformance")
}

// ── Reversible ───────────────────────────────────────────────────────────
// `Reversible[T]` is *not* a single-method protocol. The collections.abc table
// gives its row as `Reversible | Iterable | __reversed__` — it inherits
// `Iterable`, so structural conformance needs `__iter__` **and** `__reversed__`,
// both yielding `Iterator[T]`. CPython agrees at runtime: `Reversible`'s
// `__subclasshook__` calls `_check_methods(C, "__reversed__", "__iter__")`.
//
// Both members are therefore present in every fixture below, and only the
// element type varies between the rejected and accepted legs. That is what
// makes the element type load-bearing: with `__iter__` omitted, the rejected
// leg would pass on the missing member alone and prove nothing about `T`.

const REVERSIBLE_REJECTED: &str = r#"
from typing import Iterator, Reversible


class Ledger:
    def __iter__(self) -> Iterator[bytes]:
        return iter([b"entry"])

    def __reversed__(self) -> Iterator[bytes]:
        return iter([b"entry"])


def unwind(rows: Reversible[str]) -> None:
    for row in reversed(rows):
        print(row)


unwind(Ledger())
"#;

const REVERSIBLE_REJECTED_ALIASED: &str = r#"
from typing import Iterator as StreamOf, Reversible as CanReverse


class Ledger:
    def __iter__(self) -> StreamOf[bytes]:
        return iter([b"entry"])

    def __reversed__(self) -> StreamOf[bytes]:
        return iter([b"entry"])


def unwind(rows: CanReverse[str]) -> None:
    for row in reversed(rows):
        print(row)


unwind(Ledger())
"#;

const REVERSIBLE_REJECTED_IMPORT_FORM: &str = r#"
import collections.abc


class Ledger:
    def __iter__(self) -> collections.abc.Iterator[bytes]:
        return iter([b"entry"])

    def __reversed__(self) -> collections.abc.Iterator[bytes]:
        return iter([b"entry"])


def unwind(rows: collections.abc.Reversible[str]) -> None:
    for row in reversed(rows):
        print(row)


unwind(Ledger())
"#;

const REVERSIBLE_REJECTED_RENAMED: &str = r#"
from typing import Iterator, Reversible


class Daybook:
    def __iter__(self) -> Iterator[bytes]:
        return iter([b"posting"])

    def __reversed__(self) -> Iterator[bytes]:
        return iter([b"posting"])


def rewind(entries: Reversible[str]) -> None:
    for entry in reversed(entries):
        print(entry)


rewind(Daybook())
"#;

const REVERSIBLE_REJECTED_REFORMATTED: &str = "
from typing import Iterator, Reversible

class Ledger:

        def __iter__( self ) -> Iterator[ bytes ]:
            return iter( [ b'entry' ] )

        def __reversed__( self ) -> Iterator[ bytes ]:   # <- bytes, not str
            return iter( [ b'entry' ] )

def unwind( rows : Reversible[ str ] ) -> None :
        for row in reversed( rows ):
            print( row )

unwind( Ledger() )
";

const REVERSIBLE_ACCEPTED: &str = r#"
from typing import Iterator, Reversible


class Ledger:
    def __iter__(self) -> Iterator[str]:
        return iter(["entry"])

    def __reversed__(self) -> Iterator[str]:
        return iter(["entry"])


def unwind(rows: Reversible[str]) -> None:
    for row in reversed(rows):
        print(row)


unwind(Ledger())
"#;

const REVERSIBLE_ACCEPTED_ALIASED: &str = r#"
from typing import Iterator as StreamOf, Reversible as CanReverse


class Ledger:
    def __iter__(self) -> StreamOf[str]:
        return iter(["entry"])

    def __reversed__(self) -> StreamOf[str]:
        return iter(["entry"])


def unwind(rows: CanReverse[str]) -> None:
    for row in reversed(rows):
        print(row)


unwind(Ledger())
"#;

const REVERSIBLE_ACCEPTED_IMPORT_FORM: &str = r#"
import collections.abc


class Ledger:
    def __iter__(self) -> collections.abc.Iterator[str]:
        return iter(["entry"])

    def __reversed__(self) -> collections.abc.Iterator[str]:
        return iter(["entry"])


def unwind(rows: collections.abc.Reversible[str]) -> None:
    for row in reversed(rows):
        print(row)


unwind(Ledger())
"#;

const REVERSIBLE_ACCEPTED_RENAMED: &str = r#"
from typing import Iterator, Reversible


class Daybook:
    def __iter__(self) -> Iterator[str]:
        return iter(["posting"])

    def __reversed__(self) -> Iterator[str]:
        return iter(["posting"])


def rewind(entries: Reversible[str]) -> None:
    for entry in reversed(entries):
        print(entry)


rewind(Daybook())
"#;

#[test]
fn reversible_element_type_is_load_bearing() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`Reversible` inherits `Iterable`, so both fixtures supply `__iter__` and \
                      `__reversed__`; with the member set complete and identical on both legs, \
                      the only difference left is the element type, and `Iterator[bytes]` cannot \
                      satisfy `Reversible[str]`",
        rejected: REVERSIBLE_REJECTED,
        accepted: REVERSIBLE_ACCEPTED,
        rejected_variants: &[
            aliased(REVERSIBLE_REJECTED_ALIASED),
            import_form(REVERSIBLE_REJECTED_IMPORT_FORM),
            renamed(REVERSIBLE_REJECTED_RENAMED),
            reformatted(REVERSIBLE_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[
            aliased(REVERSIBLE_ACCEPTED_ALIASED),
            import_form(REVERSIBLE_ACCEPTED_IMPORT_FORM),
            renamed(REVERSIBLE_ACCEPTED_RENAMED),
        ],
    }
    .assert("Reversible element type")
}

// A companion that pins the *other* half of the member set: a class carrying a
// correctly-typed `__reversed__` but no `__iter__` is still not `Reversible`,
// which is precisely the case the original fixture mistook for conformance.
const REVERSIBLE_MISSING_ITER: &str = r#"
from typing import Iterator, Reversible


class Ledger:
    def __reversed__(self) -> Iterator[str]:
        return iter(["entry"])


def unwind(rows: Reversible[str]) -> None:
    for row in reversed(rows):
        print(row)


unwind(Ledger())
"#;

const REVERSIBLE_MISSING_ITER_RENAMED: &str = r#"
from typing import Iterator, Reversible


class Daybook:
    def __reversed__(self) -> Iterator[str]:
        return iter(["posting"])


def rewind(entries: Reversible[str]) -> None:
    for entry in reversed(entries):
        print(entry)


rewind(Daybook())
"#;

#[test]
fn reversible_requires_iter_as_well_as_reversed() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`Reversible` inherits `Iterable`, so `__reversed__` alone does not satisfy \
                      it however well typed that member is — the inherited `__iter__` is part of \
                      the protocol's member set",
        rejected: REVERSIBLE_MISSING_ITER,
        accepted: REVERSIBLE_ACCEPTED,
        rejected_variants: &[renamed(REVERSIBLE_MISSING_ITER_RENAMED)],
        accepted_variants: &[aliased(REVERSIBLE_ACCEPTED_ALIASED)],
    }
    .assert("Reversible requires __iter__ as well as __reversed__")
}

// ── SupportsFloat / SupportsAbs ──────────────────────────────────────────

const FLOAT_REJECTED: &str = r#"
from typing import SupportsFloat


class Tally:
    def __int__(self) -> int:
        return 7


def magnitude(value: SupportsFloat) -> float:
    return float(value)


magnitude(Tally())
"#;

const FLOAT_ACCEPTED: &str = r#"
from typing import SupportsFloat


class Tally:
    def __float__(self) -> float:
        return 7.0


def magnitude(value: SupportsFloat) -> float:
    return float(value)


magnitude(Tally())
"#;

const FLOAT_ACCEPTED_ALIASED: &str = r#"
from typing import SupportsFloat as Floatable


class Tally:
    def __float__(self) -> float:
        return 7.0


def magnitude(value: Floatable) -> float:
    return float(value)


magnitude(Tally())
"#;

const FLOAT_REJECTED_ALIASED: &str = r#"
from typing import SupportsFloat as Floatable


class Tally:
    def __int__(self) -> int:
        return 7


def magnitude(value: Floatable) -> float:
    return float(value)


magnitude(Tally())
"#;

const FLOAT_REJECTED_IMPORT_FORM: &str = r#"
import typing


class Tally:
    def __int__(self) -> int:
        return 7


def magnitude(value: typing.SupportsFloat) -> float:
    return float(value)


magnitude(Tally())
"#;

const FLOAT_REJECTED_RENAMED: &str = r#"
from typing import SupportsFloat


class Reckoner:
    def __int__(self) -> int:
        return 7


def scale(quantity: SupportsFloat) -> float:
    return float(quantity)


scale(Reckoner())
"#;

const FLOAT_REJECTED_REFORMATTED: &str = "
from typing import SupportsFloat

class Tally:

        def __int__( self ) -> int :   # <- __int__ is not __float__
            return 7

def magnitude( value : SupportsFloat ) -> float :
        return float( value )

magnitude( Tally() )
";

#[test]
fn supports_float_requires_dunder_float() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`__int__` does not stand in for `__float__` under structural typing",
        rejected: FLOAT_REJECTED,
        accepted: FLOAT_ACCEPTED,
        rejected_variants: &[
            aliased(FLOAT_REJECTED_ALIASED),
            import_form(FLOAT_REJECTED_IMPORT_FORM),
            renamed(FLOAT_REJECTED_RENAMED),
            reformatted(FLOAT_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[aliased(FLOAT_ACCEPTED_ALIASED)],
    }
    .assert("SupportsFloat member set")
}

// ── Container ────────────────────────────────────────────────────────────

const CONTAINER_REJECTED: &str = r#"
from typing import Container


class Roster:
    def __iter__(self):
        return iter(())


def holds(bag: Container[int], needle: int) -> bool:
    return needle in bag


holds(Roster(), 3)
"#;

const CONTAINER_ACCEPTED: &str = r#"
from typing import Container


class Roster:
    def __contains__(self, item: object) -> bool:
        return False


def holds(bag: Container[int], needle: int) -> bool:
    return needle in bag


holds(Roster(), 3)
"#;

const CONTAINER_ACCEPTED_RENAMED: &str = r#"
from typing import Container


class Muster:
    def __contains__(self, candidate: object) -> bool:
        return False


def occupies(vessel: Container[int], probe: int) -> bool:
    return probe in vessel


occupies(Muster(), 3)
"#;

const CONTAINER_REJECTED_ALIASED: &str = r#"
from typing import Container as Holds


class Roster:
    def __iter__(self):
        return iter(())


def holds(bag: Holds[int], needle: int) -> bool:
    return needle in bag


holds(Roster(), 3)
"#;

const CONTAINER_REJECTED_IMPORT_FORM: &str = r#"
import collections.abc


class Roster:
    def __iter__(self):
        return iter(())


def holds(bag: collections.abc.Container[int], needle: int) -> bool:
    return needle in bag


holds(Roster(), 3)
"#;

const CONTAINER_REJECTED_RENAMED: &str = r#"
from typing import Container


class Muster:
    def __iter__(self):
        return iter(())


def occupies(vessel: Container[int], probe: int) -> bool:
    return probe in vessel


occupies(Muster(), 3)
"#;

const CONTAINER_REJECTED_REFORMATTED: &str = "
from typing import Container

class Roster:

        def __iter__( self ):   # <- iterability is not containment
            return iter( () )

def holds( bag : Container[ int ] , needle : int ) -> bool :
        return needle in bag

holds( Roster() , 3 )
";

#[test]
fn container_requires_dunder_contains() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`Container` is satisfied by `__contains__`, not by `__iter__`. Unlike \
                      `Reversible`, `Container` inherits nothing, so `__contains__` really is its \
                      entire member set",
        rejected: CONTAINER_REJECTED,
        accepted: CONTAINER_ACCEPTED,
        rejected_variants: &[
            aliased(CONTAINER_REJECTED_ALIASED),
            import_form(CONTAINER_REJECTED_IMPORT_FORM),
            renamed(CONTAINER_REJECTED_RENAMED),
            reformatted(CONTAINER_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[renamed(CONTAINER_ACCEPTED_RENAMED)],
    }
    .assert("Container member set")
}
