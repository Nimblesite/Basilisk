//! Argument-to-parameter assignability. [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! Every argument must be assignable to the parameter it binds to — positional,
//! keyword-only, `*args` element, and `**kwargs` value alike. The obligation is
//! identical whether the callee is a user-defined method or a stdlib function
//! reached through typeshed, so both appear here.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── Bound method, positional parameter ───────────────────────────────────

#[test]
fn method_parameter_rejects_wrong_argument_type() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`str` is not assignable to the `int` parameter of `Kiln.fire`",
        rejected: r#"
class Kiln:
    def fire(self, degrees: int) -> int:
        return degrees * degrees


kiln = Kiln()
kiln.fire("hello")
"#,
        accepted: r#"
class Kiln:
    def fire(self, degrees: int) -> int:
        return degrees * degrees


kiln = Kiln()
kiln.fire(1200)
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import int as Whole


class Kiln:
    def fire(self, degrees: Whole) -> Whole:
        return degrees * degrees


kiln = Kiln()
kiln.fire("hello")
"#,
            ),
            import_form(
                r#"
import builtins


class Kiln:
    def fire(self, degrees: builtins.int) -> builtins.int:
        return degrees * degrees


kiln = Kiln()
kiln.fire("hello")
"#,
            ),
            renamed(
                r#"
class Furnace:
    def stoke(self, heat: int) -> int:
        return heat * heat


furnace = Furnace()
furnace.stoke("hello")
"#,
            ),
            reformatted(
                "
class Kiln:

        def fire(self, degrees: int) -> int:

                return degrees * degrees

kiln = Kiln()

kiln.fire(
    'hello',   # not a temperature
)
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import int as Whole


class Kiln:
    def fire(self, degrees: Whole) -> Whole:
        return degrees * degrees


kiln = Kiln()
kiln.fire(1200)
"#,
            ),
            renamed(
                r#"
class Furnace:
    def stoke(self, heat: int) -> int:
        return heat * heat


furnace = Furnace()
furnace.stoke(1200)
"#,
            ),
        ],
    }
    .assert("method parameter rejects wrong argument type")
}

// ── Keyword-only parameter ───────────────────────────────────────────────

#[test]
fn keyword_only_parameter_rejects_wrong_argument_type() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`str` is not assignable to the keyword-only `int` parameter `depth`",
        rejected: r#"
def excavate(width: int, breadth: int, *, depth: int = 0) -> int:
    return width * breadth * depth


excavate(1, 2, depth="hello")
"#,
        accepted: r#"
def excavate(width: int, breadth: int, *, depth: int = 0) -> int:
    return width * breadth * depth


excavate(1, 2, depth=3)
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import int as Whole


def excavate(width: Whole, breadth: Whole, *, depth: Whole = 0) -> Whole:
    return width * breadth * depth


excavate(1, 2, depth="hello")
"#,
            ),
            import_form(
                r#"
import builtins


def excavate(
    width: builtins.int, breadth: builtins.int, *, depth: builtins.int = 0
) -> builtins.int:
    return width * breadth * depth


excavate(1, 2, depth="hello")
"#,
            ),
            renamed(
                r#"
def quarry(span: int, reach: int, *, sink: int = 0) -> int:
    return span * reach * sink


quarry(1, 2, sink="hello")
"#,
            ),
            reformatted(
                "
def excavate(
        width: int,
        breadth: int,
        *,
        depth: int = 0,
) -> int:
        return width * breadth * depth

excavate(
    1,
    2,
    depth='hello',
)
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import int as Whole


def excavate(width: Whole, breadth: Whole, *, depth: Whole = 0) -> Whole:
    return width * breadth * depth


excavate(1, 2, depth=3)
"#,
            ),
            reformatted(
                "
def excavate(
        width: int,
        breadth: int,
        *,
        depth: int = 0,
) -> int:
        return width * breadth * depth

excavate(
    1,
    2,
    depth=3,
)
",
            ),
        ],
    }
    .assert("keyword-only parameter rejects wrong argument type")
}

// ── `*args` element type ─────────────────────────────────────────────────

#[test]
fn variadic_positional_element_type_is_enforced() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`*tallies: int` declares every positional argument `int`; `str` is not \
                      assignable",
        rejected: r#"
def aggregate(*tallies: int) -> int:
    return len(tallies)


aggregate(1, 2, 3, "hello", 5)
"#,
        accepted: r#"
def aggregate(*tallies: int) -> int:
    return len(tallies)


aggregate(1, 2, 3, 4, 5)
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import int as Whole


def aggregate(*tallies: Whole) -> Whole:
    return len(tallies)


aggregate(1, 2, 3, "hello", 5)
"#,
            ),
            import_form(
                r#"
import builtins


def aggregate(*tallies: builtins.int) -> builtins.int:
    return builtins.len(tallies)


aggregate(1, 2, 3, "hello", 5)
"#,
            ),
            renamed(
                r#"
def accumulate(*counts: int) -> int:
    return len(counts)


accumulate(1, 2, 3, "hello", 5)
"#,
            ),
            reformatted(
                "
def aggregate( * tallies : int ) -> int:
        return len(tallies)

aggregate(
    1,
    2,
    3,
    'hello',   # <- not a tally
    5,
)
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import int as Whole


def aggregate(*tallies: Whole) -> Whole:
    return len(tallies)


aggregate(1, 2, 3, 4, 5)
"#,
            ),
            renamed(
                r#"
def accumulate(*counts: int) -> int:
    return len(counts)


accumulate(1, 2, 3, 4, 5)
"#,
            ),
        ],
    }
    .assert("variadic positional element type is enforced")
}

// ── `**kwargs` value type ────────────────────────────────────────────────

#[test]
fn variadic_keyword_value_type_is_enforced() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`**tallies: int` declares every keyword argument's value `int`; `str` is not \
                      assignable",
        rejected: r#"
def collate(**tallies: int) -> int:
    return len(tallies)


collate(alpha=1, beta=2, gamma=3, delta="hello", epsilon=5)
"#,
        accepted: r#"
def collate(**tallies: int) -> int:
    return len(tallies)


collate(alpha=1, beta=2, gamma=3, delta=4, epsilon=5)
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import int as Whole


def collate(**tallies: Whole) -> Whole:
    return len(tallies)


collate(alpha=1, beta=2, gamma=3, delta="hello", epsilon=5)
"#,
            ),
            import_form(
                r#"
import builtins


def collate(**tallies: builtins.int) -> builtins.int:
    return builtins.len(tallies)


collate(alpha=1, beta=2, gamma=3, delta="hello", epsilon=5)
"#,
            ),
            renamed(
                r#"
def marshal(**counts: int) -> int:
    return len(counts)


marshal(first=1, second=2, third=3, fourth="hello", fifth=5)
"#,
            ),
            reformatted(
                "
def collate( ** tallies : int ) -> int:
        return len(tallies)

collate(
    alpha=1,
    beta=2,
    gamma=3,
    delta='hello',   # <- not a tally
    epsilon=5,
)
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import int as Whole


def collate(**tallies: Whole) -> Whole:
    return len(tallies)


collate(alpha=1, beta=2, gamma=3, delta=4, epsilon=5)
"#,
            ),
            renamed(
                r#"
def marshal(**counts: int) -> int:
    return len(counts)


marshal(first=1, second=2, third=3, fourth=4, fifth=5)
"#,
            ),
        ],
    }
    .assert("variadic keyword value type is enforced")
}

// ── Stdlib signature reached through typeshed ────────────────────────────

#[test]
fn stdlib_decoder_rejects_int_payload() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`json.loads` accepts `str | bytes | bytearray`; `int` is not assignable",
        rejected: r#"
import json

json.loads(5)
"#,
        accepted: r#"
import json

json.loads("5")
"#,
        rejected_variants: &[
            aliased(
                r#"
from json import loads as decode

decode(5)
"#,
            ),
            import_form(
                r#"
from json import loads

loads(5)
"#,
            ),
            reformatted(
                "
import json

json.loads(
    5,   # a number, not a document
)
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from json import loads as decode

decode("5")
"#,
            ),
            import_form(
                r#"
from json import loads

loads("5")
"#,
            ),
            reformatted(
                "
import json

json.loads(
    '5',
)
",
            ),
        ],
    }
    .assert("stdlib decoder rejects int payload")
}
