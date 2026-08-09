//! Assignment and declaration obligations every mainstream checker enforces.
//! [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! The typing spec's assignability rules ("Type compatibility") make the
//! declared type of a name a standing obligation on every subsequent binding of
//! it. These cases are the floor, not the frontier: mypy, pyright, pyrefly and
//! ty all reject each `rejected` source below.
//!
//! Identifiers are drawn from outside the conformance suite's 913 names, and the
//! builtin types under test are reached through `builtins` aliases and
//! attribute access as well as bare, so a rule keyed to the spelling `int`
//! fails the A6/A7 legs even when it passes the canonical one.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── Declared type governs later assignment ───────────────────────────────

#[test]
fn declared_int_rejects_str_assignment() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`str` is not assignable to a name declared `int`",
        rejected: r#"
tally: int
tally = "seven"
"#,
        accepted: r#"
tally: int
tally = 7
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import int as Whole

tally: Whole
tally = "seven"
"#,
            ),
            import_form(
                r#"
import builtins

tally: builtins.int
tally = "seven"
"#,
            ),
            renamed(
                r#"
headcount: int
headcount = "seven"
"#,
            ),
            reformatted(
                "
# a running count of things
tally  :  int

tally = 'seven'   # <- the defect
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import int as Whole

tally: Whole
tally = 7
"#,
            ),
            import_form(
                r#"
import builtins

tally: builtins.int
tally = 7
"#,
            ),
            reformatted(
                "
tally  :  int

tally = (
    7
)
",
            ),
        ],
    }
    .assert("declared int rejects str assignment")
}

// ── Walrus binds through the same declaration ────────────────────────────

#[test]
fn walrus_assignment_respects_declared_type() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`:=` binds the same name, so the declared `int` still governs",
        rejected: r#"
gauge: int
(gauge := "three")
"#,
        accepted: r#"
gauge: int
(gauge := 3)
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import int as Whole

gauge: Whole
(gauge := "three")
"#,
            ),
            import_form(
                r#"
import builtins

gauge: builtins.int
(gauge := "three")
"#,
            ),
            renamed(
                r#"
dipstick: int
(dipstick := "three")
"#,
            ),
            reformatted(
                "
gauge: int
(
    gauge
    :=
    'three'
)
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import int as Whole

gauge: Whole
(gauge := 3)
"#,
            ),
            reformatted(
                "
gauge: int

( gauge := 3 )  # inline binding
",
            ),
        ],
    }
    .assert("walrus respects declared type")
}

// ── A later annotation may not contradict an established type ────────────

#[test]
fn redeclaration_may_not_contradict_inferred_type() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a name already bound to `int` cannot be redeclared `str` in the same scope",
        rejected: r#"
spindle = 1
spindle: str
"#,
        accepted: r#"
spindle = 1
spindle: int
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import str as Text

spindle = 1
spindle: Text
"#,
            ),
            import_form(
                r#"
import builtins

spindle = 1
spindle: builtins.str
"#,
            ),
            renamed(
                r#"
mandrel = 1
mandrel: str
"#,
            ),
            reformatted(
                "
spindle = 1

# second, conflicting declaration
spindle  :  str
",
            ),
        ],
        accepted_variants: &[
            renamed(
                r#"
mandrel = 1
mandrel: int
"#,
            ),
            reformatted(
                "
spindle = 1

spindle  :  int
",
            ),
        ],
    }
    .assert("redeclaration may not contradict inferred type")
}

// ── A class name is not a general-purpose variable ───────────────────────

#[test]
fn class_symbol_cannot_be_rebound_to_int() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason:
            "the name of a class is declared `type[Trestle]`; `int` is not assignable to it",
        rejected: r#"
class Trestle:
    ...


Trestle = 1
"#,
        accepted: r#"
class Trestle:
    ...


girder = Trestle
"#,
        rejected_variants: &[
            renamed(
                r#"
class Coppice:
    ...


Coppice = 1
"#,
            ),
            reformatted(
                "
class Trestle:  # a bridge support
        ...

Trestle = 1  # <- the defect
",
            ),
            import_form(
                r#"
import builtins


class Trestle:
    ...


Trestle = builtins.int("1")
"#,
            ),
        ],
        accepted_variants: &[
            renamed(
                r#"
class Coppice:
    ...


thicket = Coppice
"#,
            ),
            reformatted(
                "
class Trestle:
        ...

girder = (
    Trestle
)
",
            ),
        ],
    }
    .assert("class symbol cannot be rebound to int")
}

// ── Attribute creation on a type without that attribute ──────────────────

#[test]
fn attribute_assignment_requires_a_declared_attribute() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`object` declares no `carriageway`, so assigning it is an error",
        rejected: r#"
plinth = object()
plinth.carriageway = 1
"#,
        accepted: r#"
class Plinth:
    carriageway: int


plinth = Plinth()
plinth.carriageway = 1
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import object as Anything

plinth = Anything()
plinth.carriageway = 1
"#,
            ),
            import_form(
                r#"
import builtins

plinth = builtins.object()
plinth.carriageway = 1
"#,
            ),
            renamed(
                r#"
brazier = object()
brazier.flue = 1
"#,
            ),
            reformatted(
                "
plinth = object()

plinth . carriageway = 1  # object has no such slot
",
            ),
        ],
        accepted_variants: &[
            renamed(
                r#"
class Brazier:
    flue: int


brazier = Brazier()
brazier.flue = 1
"#,
            ),
            reformatted(
                "
class Plinth:

        carriageway : int

plinth = Plinth()
plinth . carriageway = 1
",
            ),
        ],
    }
    .assert("attribute assignment requires a declared attribute")
}

// ── Read-only property ───────────────────────────────────────────────────

#[test]
fn read_only_property_rejects_assignment() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a `property` with no setter is read-only; assigning through it is an error",
        rejected: r#"
class Weathervane:
    @property
    def bearing(self) -> str:
        return "north"


Weathervane().bearing = "south"
"#,
        accepted: r#"
class Weathervane:
    @property
    def bearing(self) -> str:
        return "north"

    @bearing.setter
    def bearing(self, value: str) -> None:
        self._bearing = value


Weathervane().bearing = "south"
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import property as ComputedAttribute


class Weathervane:
    @ComputedAttribute
    def bearing(self) -> str:
        return "north"


Weathervane().bearing = "south"
"#,
            ),
            import_form(
                r#"
import builtins


class Weathervane:
    @builtins.property
    def bearing(self) -> str:
        return "north"


Weathervane().bearing = "south"
"#,
            ),
            renamed(
                r#"
class Anemometer:
    @property
    def heading(self) -> str:
        return "north"


Anemometer().heading = "south"
"#,
            ),
            reformatted(
                "
class Weathervane:

        @property
        def bearing(self) -> str:

                return 'north'

# no setter exists, so this cannot be assigned
Weathervane().bearing = 'south'
",
            ),
        ],
        accepted_variants: &[renamed(
            r#"
class Anemometer:
    @property
    def heading(self) -> str:
        return "north"

    @heading.setter
    def heading(self, value: str) -> None:
        self._heading = value


Anemometer().heading = "south"
"#,
        )],
    }
    .assert("read-only property rejects assignment")
}
