//! Type aliases: PEP 695 `type` statements, `TypeAliasType`, forward references.
//! [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! The `type` statement carries **no import at all**, so this area offers no
//! library spelling for a rule to key on: every obligation below is decided
//! from the alias's *value expression* and the scope its type parameters open,
//! never from the text to the right of the `=`. The one quarantined symbol the
//! area cannot avoid — `TypeAliasType` — appears only under an alias
//! (`as ExplicitAliasKind`) or through `import typing`, never bare. Supporting
//! symbols come from outside the 55 the conformance suite imports
//! (`MutableSequence`), and every identifier is outside the suite's 913.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── RHS must be a type expression: list display ──────────────────────────
// The spec requires the value of a `type` statement to be a valid type
// expression. A list display is one only in the first argument of `Callable`;
// standing alone it denotes no type.

const LIST_DISPLAY_REJECTED: &str = r"
from typing import MutableSequence

type Sluice = [int, MutableSequence[str]]

def gauge(outfall: Sluice) -> int:
    return len(outfall)
";

const LIST_DISPLAY_ACCEPTED: &str = r"
from typing import MutableSequence

type Sluice = tuple[int, MutableSequence[str]]

def gauge(outfall: Sluice) -> int:
    return len(outfall)
";

const LIST_DISPLAY_REJECTED_ALIASED: &str = r"
from typing import MutableSequence as RowSeries

type Sluice = [int, RowSeries[str]]

def gauge(outfall: Sluice) -> int:
    return len(outfall)
";

const LIST_DISPLAY_REJECTED_IMPORT_FORM: &str = r"
import collections.abc

type Sluice = [int, collections.abc.MutableSequence[str]]

def gauge(outfall: Sluice) -> int:
    return len(outfall)
";

const LIST_DISPLAY_REJECTED_REFORMATTED: &str = r"
from typing import MutableSequence
type Sluice = ([int, MutableSequence[str]])  # parens change nothing
def gauge(
        outfall: Sluice,
) -> int:
        return len((outfall))
";

#[test]
fn alias_value_may_not_be_a_list_display() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a list display is not a type expression, so it cannot be an alias value",
        rejected: LIST_DISPLAY_REJECTED,
        accepted: LIST_DISPLAY_ACCEPTED,
        rejected_variants: &[
            aliased(LIST_DISPLAY_REJECTED_ALIASED),
            import_form(LIST_DISPLAY_REJECTED_IMPORT_FORM),
            reformatted(LIST_DISPLAY_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[],
    }
    .assert("alias value: list display")
}

// ── RHS must be a type expression: call ──────────────────────────────────
// A call produces a value; nothing in the spec turns a call result into a
// type. `eval` is the sharpest case — the string it is handed does name a
// type, and the alias is ill-formed regardless.

const CALL_REJECTED: &str = r#"
type Kiln = eval("int")

def fire(vessel: Kiln) -> int:
    return vessel
"#;

const CALL_ACCEPTED: &str = r"
type Kiln = int

def fire(vessel: Kiln) -> int:
    return vessel
";

const CALL_REJECTED_RENAMED: &str = r#"
type Trivet = eval("int")

def bake(crock: Trivet) -> int:
    return crock
"#;

#[test]
fn alias_value_may_not_be_a_call() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a call is not a type expression, even when it evaluates to a type",
        rejected: CALL_REJECTED,
        accepted: CALL_ACCEPTED,
        rejected_variants: &[renamed(CALL_REJECTED_RENAMED)],
        accepted_variants: &[],
    }
    .assert("alias value: call")
}

// ── RHS must be a type expression: arithmetic ────────────────────────────
// `|` between two types is the one binary operator the spec lifts into the
// type-expression grammar. `+` is not, so the alias names nothing.

const ARITHMETIC_REJECTED: &str = r"
type Grommet = 1 + 1

def seat(pin: Grommet) -> int:
    return pin
";

const ARITHMETIC_ACCEPTED: &str = r"
type Grommet = int

def seat(pin: Grommet) -> int:
    return pin
";

const ARITHMETIC_REJECTED_REFORMATTED: &str = r"
type Grommet = (
    1
    + 1
)  # a sum, not a type
def seat(pin: Grommet) -> int:
        return pin
";

#[test]
fn alias_value_may_not_be_arithmetic() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`+` is not part of the type-expression grammar; only `|` forms a union",
        rejected: ARITHMETIC_REJECTED,
        accepted: ARITHMETIC_ACCEPTED,
        rejected_variants: &[reformatted(ARITHMETIC_REJECTED_REFORMATTED)],
        accepted_variants: &[],
    }
    .assert("alias value: arithmetic")
}

// ── RHS must be a type expression: attribute of a specialisation ─────────
// `list[int]` is a type expression; a member lookup applied to one is not.
// The spec admits a dotted name that resolves to a type, never an attribute
// read off a specialised generic.

const ATTRIBUTE_REJECTED: &str = r"
type Bailiwick = list[int].denominator

def survey(ashlar: Bailiwick) -> int:
    return len(ashlar)
";

const ATTRIBUTE_ACCEPTED: &str = r"
type Bailiwick = list[int]

def survey(ashlar: Bailiwick) -> int:
    return len(ashlar)
";

#[test]
fn alias_value_may_not_be_an_attribute() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "attribute access on a specialised generic does not denote a type",
        rejected: ATTRIBUTE_REJECTED,
        accepted: ATTRIBUTE_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[],
    }
    .assert("alias value: attribute of a specialisation")
}

// ── Generic alias arity ──────────────────────────────────────────────────
// A `type` statement's type-parameter list fixes the alias's arity; supplying
// a different number of arguments is an error, exactly as for a generic class.

const ARITY_REJECTED: &str = r"
from typing import MutableSequence

type Tanner[TRadix] = MutableSequence[TRadix]

def tabulate(rows: Tanner[int, str]) -> int:
    return len(rows)
";

const ARITY_ACCEPTED: &str = r"
from typing import MutableSequence

type Tanner[TRadix] = MutableSequence[TRadix]

def tabulate(rows: Tanner[int]) -> int:
    return len(rows)
";

const ARITY_REJECTED_ALIASED: &str = r"
from typing import MutableSequence as RowSeries

type Tanner[TRadix] = RowSeries[TRadix]

def tabulate(rows: Tanner[int, str]) -> int:
    return len(rows)
";

const ARITY_REJECTED_RENAMED: &str = r"
from typing import MutableSequence

type Cooper[TQuarry] = MutableSequence[TQuarry]

def stocktake(entries: Cooper[int, str]) -> int:
    return len(entries)
";

const ARITY_ACCEPTED_IMPORT_FORM: &str = r"
import collections.abc

type Tanner[TRadix] = collections.abc.MutableSequence[TRadix]

def tabulate(rows: Tanner[int]) -> int:
    return len(rows)
";

#[test]
fn generic_alias_arity_is_fixed() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "an alias declared with one type parameter accepts exactly one argument",
        rejected: ARITY_REJECTED,
        accepted: ARITY_ACCEPTED,
        rejected_variants: &[
            aliased(ARITY_REJECTED_ALIASED),
            renamed(ARITY_REJECTED_RENAMED),
        ],
        accepted_variants: &[import_form(ARITY_ACCEPTED_IMPORT_FORM)],
    }
    .assert("generic alias arity")
}

// ── Recursion is legal ───────────────────────────────────────────────────
// An alias value is evaluated lazily, so an alias may name itself. It must
// still be a real type afterwards: `str` lies outside `int | list[...]`, and
// the call is rejected on that ground, not for being recursive.

const RECURSIVE_REJECTED: &str = r#"
type Wapentake = int | list[Wapentake]

def reckon(ashlar: Wapentake) -> int:
    if isinstance(ashlar, int):
        return ashlar
    tally_sum = 0
    for segment in ashlar:
        tally_sum += reckon(segment)
    return tally_sum

reckon("shire")
"#;

const RECURSIVE_ACCEPTED: &str = r"
type Wapentake = int | list[Wapentake]

def reckon(ashlar: Wapentake) -> int:
    if isinstance(ashlar, int):
        return ashlar
    tally_sum = 0
    for segment in ashlar:
        tally_sum += reckon(segment)
    return tally_sum

reckon(7)
";

const RECURSIVE_ACCEPTED_RENAMED: &str = r"
type Purlin = int | list[Purlin]

def tally_stones(quoin: Purlin) -> int:
    if isinstance(quoin, int):
        return quoin
    running_sum = 0
    for piece in quoin:
        running_sum += tally_stones(piece)
    return running_sum

tally_stones(7)
";

#[test]
fn a_type_alias_may_refer_to_itself() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "alias values are evaluated lazily, so recursion is legal and still typed",
        rejected: RECURSIVE_REJECTED,
        accepted: RECURSIVE_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[renamed(RECURSIVE_ACCEPTED_RENAMED)],
    }
    .assert("recursive alias")
}

// ── Forward reference must parse as a type expression ────────────────────
// A string in a type position is a forward reference and its contents must
// parse as a type expression. `Hogshead |` is a union missing its right
// operand; that the class it names exists is beside the point.

const FORWARD_REF_REJECTED: &str = r#"
type Firkin = "Hogshead |"

class Hogshead:
    staves: int = 31

def coop(vessel: Firkin) -> int:
    return vessel.staves
"#;

const FORWARD_REF_ACCEPTED: &str = r#"
type Firkin = "Hogshead"

class Hogshead:
    staves: int = 31

def coop(vessel: Firkin) -> int:
    return vessel.staves
"#;

const FORWARD_REF_ACCEPTED_REFORMATTED: &str = "
type Firkin = 'Hogshead'  # forward reference to the class below
class Hogshead:

        staves: int = 31

def coop(vessel: Firkin) -> int:
        return (vessel).staves
";

#[test]
fn forward_reference_must_parse_as_a_type() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a forward-reference string must parse as a type expression",
        rejected: FORWARD_REF_REJECTED,
        accepted: FORWARD_REF_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[reformatted(FORWARD_REF_ACCEPTED_REFORMATTED)],
    }
    .assert("forward reference parses")
}

// ── An alias is not a value ──────────────────────────────────────────────
// A `type` statement binds an alias object, not the type it names. That
// object is not a constructor, so instantiating through the alias is an error
// even where the aliased type is instantiable.

const NOT_CALLABLE_REJECTED: &str = r"
type Trebuchet = dict[str, int]

def restock(store: Trebuchet) -> Trebuchet:
    fresh = Trebuchet()
    fresh.update(store)
    return fresh
";

const NOT_CALLABLE_ACCEPTED: &str = r"
type Trebuchet = dict[str, int]

def restock(store: Trebuchet) -> Trebuchet:
    fresh: Trebuchet = {}
    fresh.update(store)
    return fresh
";

const NOT_CALLABLE_REJECTED_RENAMED: &str = r"
type Windlass = dict[str, int]

def replenish(cargo: Windlass) -> Windlass:
    bilge = Windlass()
    bilge.update(cargo)
    return bilge
";

#[test]
fn a_type_alias_is_not_callable() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a `type` statement binds an alias object, which is not a constructor",
        rejected: NOT_CALLABLE_REJECTED,
        accepted: NOT_CALLABLE_ACCEPTED,
        rejected_variants: &[renamed(NOT_CALLABLE_REJECTED_RENAMED)],
        accepted_variants: &[],
    }
    .assert("alias is not callable")
}

// ── An alias to a non-class cannot be a base class ───────────────────────
// A base class must be a class. An alias whose value is a union names none,
// so it is rejected there while remaining usable in an annotation.

const BASE_CLASS_REJECTED: &str = r"
type Anvil = int | str

class Adze(Anvil):
    pass

def strike(blank: Anvil) -> Adze:
    return Adze()
";

const BASE_CLASS_ACCEPTED: &str = r"
type Anvil = int | str

class Ferrule:
    pass

class Adze(Ferrule):
    pass

def strike(blank: Anvil) -> Adze:
    return Adze()
";

#[test]
fn an_alias_to_a_union_is_not_a_base_class() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a union denotes no class object, so an alias to one cannot be inherited",
        rejected: BASE_CLASS_REJECTED,
        accepted: BASE_CLASS_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[],
    }
    .assert("alias in a base-class list")
}

// ── TypeAliasType names itself ───────────────────────────────────────────
// The explicit form's first argument is the alias's own name; disagreement
// with the target of the assignment is an error. Quarantined symbol, so it
// appears aliased and dotted only.

const EXPLICIT_NAME_REJECTED: &str = r#"
from typing import TypeAliasType as ExplicitAliasKind

Bodkin = ExplicitAliasKind("Trivet", list[int])

def pierce(cargo: Bodkin) -> int:
    return len(cargo)
"#;

const EXPLICIT_NAME_ACCEPTED: &str = r#"
from typing import TypeAliasType as ExplicitAliasKind

Bodkin = ExplicitAliasKind("Bodkin", list[int])

def pierce(cargo: Bodkin) -> int:
    return len(cargo)
"#;

const EXPLICIT_NAME_REJECTED_IMPORT_FORM: &str = r#"
import typing

Bodkin = typing.TypeAliasType("Trivet", list[int])

def pierce(cargo: Bodkin) -> int:
    return len(cargo)
"#;

const EXPLICIT_NAME_REJECTED_RENAMED: &str = r#"
from typing import TypeAliasType as ExplicitAliasKind

Mandrel = ExplicitAliasKind("Trivet", list[int])

def bore(stone: Mandrel) -> int:
    return len(stone)
"#;

#[test]
fn explicit_alias_name_must_match_its_binding() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the first argument of the explicit alias form is the name it is bound to",
        rejected: EXPLICIT_NAME_REJECTED,
        accepted: EXPLICIT_NAME_ACCEPTED,
        rejected_variants: &[
            import_form(EXPLICIT_NAME_REJECTED_IMPORT_FORM),
            renamed(EXPLICIT_NAME_REJECTED_RENAMED),
        ],
        accepted_variants: &[],
    }
    .assert("explicit alias name")
}
