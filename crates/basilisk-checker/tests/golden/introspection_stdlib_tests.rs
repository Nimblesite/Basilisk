//! Introspection helpers and the wider stdlib type surface.
//! [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! `get_origin`, `get_args`, `get_type_hints`, `is_typeddict`, `AnyStr`,
//! `Deque`, `Counter`, `Match`, `BinaryIO`, `TextIO`, `KeysView`, `AbstractSet`
//! and `MutableSet` all sit outside the 55 `typing`/`typing_extensions` symbols
//! `conformance/tests/` imports, so no hardcoded arm can exist for any of them.
//! `TypedDict` is quarantined and appears only aliased or via `typing.`.
//! Identifiers are drawn from a vocabulary disjoint from the suite's 913.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── get_origin ───────────────────────────────────────────────────────────
// The spec makes `get_origin` return the *unsubscripted* class of a generic
// alias: `list[int]` erases to the `list` class object, which is not a `str`.

const ORIGIN_REJECTED: &str = r"
from typing import get_origin

sluice: str = get_origin(list[int])
";

const ORIGIN_ACCEPTED: &str = r"
from typing import get_origin

sluice: type = get_origin(list[int])
";

const ORIGIN_REJECTED_ALIASED: &str = r"
from typing import get_origin as origin_of

sluice: str = origin_of(list[int])
";

const ORIGIN_REJECTED_IMPORT_FORM: &str = r"
import typing

sluice: str = typing.get_origin(list[int])
";

#[test]
fn get_origin_yields_a_class_not_a_string() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`get_origin(list[int])` is the `list` class object, which is not a `str`",
        rejected: ORIGIN_REJECTED,
        accepted: ORIGIN_ACCEPTED,
        rejected_variants: &[
            aliased(ORIGIN_REJECTED_ALIASED),
            import_form(ORIGIN_REJECTED_IMPORT_FORM),
        ],
        accepted_variants: &[],
    }
    .assert("get_origin result type")
}

// ── get_args ─────────────────────────────────────────────────────────────
// `get_args` returns the argument *tuple* of a generic alias. A tuple is not an
// `int`, however many arguments the alias happens to carry.

const ARGS_REJECTED: &str = r"
from typing import get_args

grommet: int = get_args(dict[str, int])
";

const ARGS_ACCEPTED: &str = r"
from typing import get_args

grommet: tuple[object, ...] = get_args(dict[str, int])
";

const ARGS_REJECTED_ALIASED: &str = r"
from typing import get_args as args_of

grommet: int = args_of(dict[str, int])
";

const ARGS_REJECTED_RENAMED: &str = r"
from typing import get_args

withy: int = get_args(dict[str, int])
";

#[test]
fn get_args_yields_a_tuple_not_a_scalar() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`get_args` returns the alias argument tuple, never a single `int`",
        rejected: ARGS_REJECTED,
        accepted: ARGS_ACCEPTED,
        rejected_variants: &[
            aliased(ARGS_REJECTED_ALIASED),
            renamed(ARGS_REJECTED_RENAMED),
        ],
        accepted_variants: &[],
    }
    .assert("get_args result type")
}

// ── get_type_hints ───────────────────────────────────────────────────────
// `get_type_hints` maps attribute names to their resolved annotations, so the
// result is a mapping keyed by `str` — never a bare list of names.

const HINTS_REJECTED: &str = r"
from typing import get_type_hints

class Wapentake:
    tithing: int
    reeve: str

muniments: list[str] = get_type_hints(Wapentake)
";

const HINTS_ACCEPTED: &str = r"
from typing import get_type_hints

class Wapentake:
    tithing: int
    reeve: str

muniments: dict[str, object] = get_type_hints(Wapentake)
";

const HINTS_ACCEPTED_IMPORT_FORM: &str = r"
import typing

class Wapentake:
    tithing: int
    reeve: str

muniments: dict[str, object] = typing.get_type_hints(Wapentake)
";

#[test]
fn get_type_hints_yields_a_mapping_not_a_list() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`get_type_hints` returns a name-to-annotation mapping, not `list[str]`",
        rejected: HINTS_REJECTED,
        accepted: HINTS_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[import_form(HINTS_ACCEPTED_IMPORT_FORM)],
    }
    .assert("get_type_hints result type")
}

// ── is_typeddict ─────────────────────────────────────────────────────────
// A predicate returns `bool`. `TypedDict` is quarantined vocabulary, so it is
// reached only through an alias or the `typing.` attribute path.

const TYPEDDICT_REJECTED: &str = r"
from typing import TypedDict as RecordShape, is_typeddict

class Bailiwick(RecordShape):
    hundred: str
    hides: int

quarry: str = is_typeddict(Bailiwick)
";

const TYPEDDICT_ACCEPTED: &str = r"
from typing import TypedDict as RecordShape, is_typeddict

class Bailiwick(RecordShape):
    hundred: str
    hides: int

quarry: bool = is_typeddict(Bailiwick)
";

const TYPEDDICT_ACCEPTED_IMPORT_FORM: &str = r"
import typing

class Bailiwick(typing.TypedDict):
    hundred: str
    hides: int

quarry: bool = typing.is_typeddict(Bailiwick)
";

#[test]
fn is_typeddict_yields_a_bool() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`is_typeddict` is a predicate returning `bool`, not the class name",
        rejected: TYPEDDICT_REJECTED,
        accepted: TYPEDDICT_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[import_form(TYPEDDICT_ACCEPTED_IMPORT_FORM)],
    }
    .assert("is_typeddict result type")
}

// ── AnyStr ───────────────────────────────────────────────────────────────
// `AnyStr` is a *constrained* type variable over `{str, bytes}`. The spec binds
// it to exactly one constraint per call, so the two cannot be mixed across the
// parameters of a single invocation.

const ANYSTR_REJECTED: &str = r#"
from typing import AnyStr

def splice(fore: AnyStr, aft: AnyStr) -> AnyStr:
    return fore + aft

splice("larboard", b"starboard")
"#;

const ANYSTR_ACCEPTED: &str = r#"
from typing import AnyStr

def splice(fore: AnyStr, aft: AnyStr) -> AnyStr:
    return fore + aft

splice("larboard", "starboard")
"#;

const ANYSTR_REJECTED_ALIASED: &str = r#"
from typing import AnyStr as EitherString

def splice(fore: EitherString, aft: EitherString) -> EitherString:
    return fore + aft

splice("larboard", b"starboard")
"#;

const ANYSTR_REJECTED_REFORMATTED: &str = "
from typing import AnyStr

def splice(
        fore: AnyStr,
        aft: AnyStr,
) -> AnyStr:
        # one constraint is chosen for the whole call
        return fore + aft

splice('larboard', b'starboard')
";

#[test]
fn anystr_binds_one_constraint_per_call() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a constrained type variable resolves to one constraint, so `str` and \
                      `bytes` cannot both satisfy `AnyStr` in one call",
        rejected: ANYSTR_REJECTED,
        accepted: ANYSTR_ACCEPTED,
        rejected_variants: &[
            aliased(ANYSTR_REJECTED_ALIASED),
            reformatted(ANYSTR_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[],
    }
    .assert("AnyStr constraint solution")
}

// ── Deque ────────────────────────────────────────────────────────────────
// `Deque[int]` is `collections.deque`, a sequence type unrelated to `list`.
// Sharing an element type does not make one assignable to the other.

const DEQUE_REJECTED: &str = r"
from typing import Deque

def winnow(rows: list[int]) -> int:
    return sum(rows)

def hoist(seam: Deque[int]) -> int:
    return winnow(seam)
";

const DEQUE_ACCEPTED: &str = r"
from typing import Deque

def winnow(rows: Deque[int]) -> int:
    return sum(rows)

def hoist(seam: Deque[int]) -> int:
    return winnow(seam)
";

const DEQUE_REJECTED_RENAMED: &str = r"
from typing import Deque

def gather_span(scow: list[int]) -> int:
    return sum(scow)

def empty_out(barge: Deque[int]) -> int:
    return gather_span(barge)
";

const DEQUE_ACCEPTED_IMPORT_FORM: &str = r"
import collections

def winnow(rows: collections.deque[int]) -> int:
    return sum(rows)

def hoist(seam: collections.deque[int]) -> int:
    return winnow(seam)
";
