//! Subscripting is a protocol, not syntax. [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! `obj[k]` is `type(obj).__getitem__(obj, k)` and `obj[k] = v` is
//! `type(obj).__setitem__(obj, k, v)`. A class declaring neither cannot be
//! subscripted at all, and a slice's bounds must be `SupportsIndex | None` —
//! `float` and `str` are neither.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── Reading requires `__getitem__` ───────────────────────────────────────

#[test]
fn subscript_read_requires_dunder_getitem() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a class with no `__getitem__` cannot be subscripted",
        rejected: r#"
class Trestle:
    ...


girder = Trestle()[0]
"#,
        accepted: r#"
class Trestle:
    def __getitem__(self, position: int) -> int:
        return position


girder = Trestle()[0]
"#,
        rejected_variants: &[
            renamed(
                r#"
class Gantry:
    ...


boom = Gantry()[0]
"#,
            ),
            reformatted(
                "
class Trestle:  # nothing subscriptable here
        ...

girder = Trestle()[
    0
]
",
            ),
            import_form(
                r#"
import builtins


class Trestle:
    ...


girder = Trestle()[builtins.int("0")]
"#,
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import int as Whole


class Trestle:
    def __getitem__(self, position: Whole) -> Whole:
        return position


girder = Trestle()[0]
"#,
            ),
            renamed(
                r#"
class Gantry:
    def __getitem__(self, index: int) -> int:
        return index


boom = Gantry()[0]
"#,
            ),
            reformatted(
                "
class Trestle:

        def __getitem__(self, position: int) -> int:

                return position

girder = Trestle()[
    0
]
",
            ),
        ],
    }
    .assert("subscript read requires __getitem__")
}

// ── Writing requires `__setitem__` ───────────────────────────────────────

#[test]
fn subscript_write_requires_dunder_setitem() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`__getitem__` alone does not permit subscript assignment; `__setitem__` does",
        rejected: r#"
class Cistern:
    def __getitem__(self, position: int) -> int:
        return position


cistern = Cistern()
cistern[0] = 0
"#,
        accepted: r#"
class Cistern:
    def __getitem__(self, position: int) -> int:
        return position

    def __setitem__(self, position: int, volume: int) -> None:
        self._volume = volume


cistern = Cistern()
cistern[0] = 0
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import int as Whole


class Cistern:
    def __getitem__(self, position: Whole) -> Whole:
        return position


cistern = Cistern()
cistern[0] = 0
"#,
            ),
            import_form(
                r#"
import builtins


class Cistern:
    def __getitem__(self, position: builtins.int) -> builtins.int:
        return position


cistern = Cistern()
cistern[0] = 0
"#,
            ),
            renamed(
                r#"
class Reservoir:
    def __getitem__(self, index: int) -> int:
        return index


reservoir = Reservoir()
reservoir[0] = 0
"#,
            ),
            reformatted(
                "
class Cistern:

        def __getitem__(self, position: int) -> int:

                return position

cistern = Cistern()

cistern[ 0 ] = 0   # read-only container
",
            ),
        ],
        accepted_variants: &[renamed(
            r#"
class Reservoir:
    def __getitem__(self, index: int) -> int:
        return index

    def __setitem__(self, index: int, amount: int) -> None:
        self._amount = amount


reservoir = Reservoir()
reservoir[0] = 0
"#,
        )],
    }
    .assert("subscript write requires __setitem__")
}

// ── Slice bounds must be index-like ──────────────────────────────────────

#[test]
fn slice_bounds_reject_float() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`str.__getitem__` takes `SupportsIndex | slice[int, int, int]`; `float` has \
                      no `__index__`",
        rejected: r#"
def excerpt(passage: str, opening: float, closing: float) -> str:
    return passage[opening:closing]
"#,
        accepted: r#"
def excerpt(passage: str, opening: int, closing: int) -> str:
    return passage[opening:closing]
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import float as Real, str as Text


def excerpt(passage: Text, opening: Real, closing: Real) -> Text:
    return passage[opening:closing]
"#,
            ),
            import_form(
                r#"
import builtins


def excerpt(
    passage: builtins.str, opening: builtins.float, closing: builtins.float
) -> builtins.str:
    return passage[opening:closing]
"#,
            ),
            renamed(
                r#"
def extract(prose: str, head: float, tail: float) -> str:
    return prose[head:tail]
"#,
            ),
            reformatted(
                "
def excerpt(
        passage: str,
        opening: float,   # <- fractional positions are not indices
        closing: float,
) -> str:
        return passage[ opening : closing ]
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import int as Whole, str as Text


def excerpt(passage: Text, opening: Whole, closing: Whole) -> Text:
    return passage[opening:closing]
"#,
            ),
            reformatted(
                "
def excerpt(
        passage: str,
        opening: int,
        closing: int,
) -> str:
        return passage[ opening : closing ]
",
            ),
        ],
    }
    .assert("slice bounds reject float")
}

#[test]
fn slice_bounds_reject_str() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a slice of a `str` is bounded by indices, not by substrings",
        rejected: r#"
"foo"["bar":"baz"]
"#,
        accepted: r#"
"foo"[1:2]
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import str as Text

Text("foo")["bar":"baz"]
"#,
            ),
            import_form(
                r#"
import builtins

builtins.str("foo")["bar":"baz"]
"#,
            ),
            reformatted(
                "
'foo'[
    'bar'   # <- a needle, not an offset
    :
    'baz'
]
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import str as Text

Text("foo")[1:2]
"#,
            ),
            reformatted(
                "
'foo'[
    1
    :
    2
]
",
            ),
        ],
    }
    .assert("slice bounds reject str")
}
