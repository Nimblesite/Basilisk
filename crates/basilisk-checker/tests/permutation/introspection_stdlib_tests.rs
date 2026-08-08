//! Introspection helpers and the wider stdlib type surface.
//! [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! `get_origin`, `get_args`, `get_type_hints`, `is_typeddict`, `AnyStr`, `Text`,
//! `Deque`, `Counter`, `Match`, `BinaryIO`, `TextIO`, `KeysView`, `AbstractSet`
//! and `MutableSet` all sit outside the 55 `typing`/`typing_extensions` symbols
//! `conformance/tests/` imports, so no hardcoded arm can exist for any of them.
//! `TypedDict` is quarantined and appears only aliased or via `typing.`.
//! Identifiers are drawn from a vocabulary disjoint from the suite's 913.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── get_origin ───────────────────────────────────────────────────────────
// The spec makes `get_origin` return the *unsubscripted* class of a generic
// alias — `list[int]` erases to the `list` class object. A class object is not
// a `str`, so binding the result to `str` is ill-typed.

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

const ORIGIN_REJECTED_REFORMATTED: &str = "
from typing import get_origin
# erasure yields a class, whatever the annotation is spelled like
sluice: str = get_origin(
        list[int],
)
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
            reformatted(ORIGIN_REJECTED_REFORMATTED),
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
// `get_type_hints` maps attribute names to their resolved annotations, so its
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
// it to exactly one constraint per call, so `str` and `bytes` cannot be mixed
// across the parameters of a single invocation.

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

#[test]
fn deque_is_not_a_list() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`collections.deque` is not a subtype of `list`, whatever its element type",
        rejected: DEQUE_REJECTED,
        accepted: DEQUE_ACCEPTED,
        rejected_variants: &[renamed(DEQUE_REJECTED_RENAMED)],
        accepted_variants: &[import_form(DEQUE_ACCEPTED_IMPORT_FORM)],
    }
    .assert("Deque is not list")
}

// ── Counter ──────────────────────────────────────────────────────────────
// `Counter.__add__` is declared to return another `Counter` of the same key
// type. It sums the per-key tallies; it does not collapse to a scalar.

const COUNTER_REJECTED: &str = r"
from typing import Counter

def merge_tallies(nearside: Counter[str], offside: Counter[str]) -> int:
    return nearside + offside
";

const COUNTER_ACCEPTED: &str = r"
from typing import Counter

def merge_tallies(nearside: Counter[str], offside: Counter[str]) -> Counter[str]:
    return nearside + offside
";

const COUNTER_ACCEPTED_IMPORT_FORM: &str = r"
import collections

def merge_tallies(nearside: collections.Counter[str], offside: collections.Counter[str]) -> collections.Counter[str]:
    return nearside + offside
";

#[test]
fn counter_addition_yields_a_counter() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`Counter[str] + Counter[str]` is `Counter[str]`, never `int`",
        rejected: COUNTER_REJECTED,
        accepted: COUNTER_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[import_form(COUNTER_ACCEPTED_IMPORT_FORM)],
    }
    .assert("Counter addition result type")
}

// ── Match ────────────────────────────────────────────────────────────────
// `re.match` is declared `Match[str] | None`. The `None` arm has no `group`, so
// the spec requires narrowing before the attribute access.

const MATCH_REJECTED: &str = r#"
import re
from typing import Match

def leading_digits(haystack: str) -> str:
    found: Match[str] | None = re.match("[0-9]+", haystack)
    return found.group(0)
"#;

const MATCH_ACCEPTED: &str = r#"
import re
from typing import Match

def leading_digits(haystack: str) -> str:
    found: Match[str] | None = re.match("[0-9]+", haystack)
    if found is None:
        return ""
    return found.group(0)
"#;

const MATCH_REJECTED_IMPORT_FORM: &str = r#"
import re

def leading_digits(haystack: str) -> str:
    found: re.Match[str] | None = re.match("[0-9]+", haystack)
    return found.group(0)
"#;

const MATCH_REJECTED_REFORMATTED: &str = "
import re
from typing import Match

def leading_digits(haystack: str) -> str:
        # the pattern need not match at all
        found: Match[str] | None = re.match('[0-9]+', haystack)
        return (found).group(0)
";

#[test]
fn re_match_result_is_optional() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`re.match` yields `Match[str] | None`, and `None` has no `group`",
        rejected: MATCH_REJECTED,
        accepted: MATCH_ACCEPTED,
        rejected_variants: &[
            import_form(MATCH_REJECTED_IMPORT_FORM),
            reformatted(MATCH_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[],
    }
    .assert("re.match optionality")
}

// ── BinaryIO ─────────────────────────────────────────────────────────────
// `BinaryIO` is `IO[bytes]`, so `read` yields `bytes`. The byte stream is never
// implicitly decoded.

const BINARY_REJECTED: &str = r"
from typing import BinaryIO

def first_octets(handle: BinaryIO) -> str:
    return handle.read(16)
";

const BINARY_ACCEPTED: &str = r"
from typing import BinaryIO

def first_octets(handle: BinaryIO) -> bytes:
    return handle.read(16)
";

const BINARY_ACCEPTED_ALIASED: &str = r"
from typing import BinaryIO as OctetStream

def first_octets(handle: OctetStream) -> bytes:
    return handle.read(16)
";

#[test]
fn binary_stream_read_yields_bytes() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`BinaryIO` is `IO[bytes]`, so `read` yields `bytes` and not `str`",
        rejected: BINARY_REJECTED,
        accepted: BINARY_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[aliased(BINARY_ACCEPTED_ALIASED)],
    }
    .assert("BinaryIO read element type")
}

// ── open() in binary mode ────────────────────────────────────────────────
// The mode literal selects the overload: `"rb"` opens a byte stream, so the
// read result is `bytes` no matter how the call is laid out.

const OPEN_REJECTED: &str = r#"
def preamble(waybill: str) -> str:
    with open(waybill, "rb") as handle:
        return handle.read(16)
"#;

const OPEN_ACCEPTED: &str = r#"
def preamble(waybill: str) -> bytes:
    with open(waybill, "rb") as handle:
        return handle.read(16)
"#;

const OPEN_ACCEPTED_REFORMATTED: &str = "
def preamble(waybill: str) -> bytes:
        with open(waybill, 'rb') as handle:  # binary mode
                return handle.read(16)
";

#[test]
fn binary_mode_open_yields_bytes() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`open(..., \"rb\")` selects the binary overload, whose `read` yields `bytes`",
        rejected: OPEN_REJECTED,
        accepted: OPEN_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[reformatted(OPEN_ACCEPTED_REFORMATTED)],
    }
    .assert("open binary mode element type")
}

// ── TextIO ───────────────────────────────────────────────────────────────
// `TextIO` is `IO[str]`, so `write` takes `str`. Bytes are not implicitly
// encoded on the way in.

const TEXTIO_REJECTED: &str = r"
from typing import TextIO

def inscribe(sink: TextIO, payload: bytes) -> None:
    sink.write(payload)
";

const TEXTIO_ACCEPTED: &str = r"
from typing import TextIO

def inscribe(sink: TextIO, payload: str) -> None:
    sink.write(payload)
";

const TEXTIO_REJECTED_ALIASED: &str = r"
from typing import TextIO as CharStream

def inscribe(sink: CharStream, payload: bytes) -> None:
    sink.write(payload)
";

#[test]
fn text_stream_write_rejects_bytes() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`TextIO` is `IO[str]`, so `write` accepts `str` and rejects `bytes`",
        rejected: TEXTIO_REJECTED,
        accepted: TEXTIO_ACCEPTED,
        rejected_variants: &[aliased(TEXTIO_REJECTED_ALIASED)],
        accepted_variants: &[],
    }
    .assert("TextIO write parameter type")
}

// ── KeysView ─────────────────────────────────────────────────────────────
// `KeysView` derives from `AbstractSet`, so it carries the set operators. A
// `list` does not: `list.__and__` does not exist.

const KEYSVIEW_REJECTED: &str = r"
from typing import AbstractSet

def shared_labels(nearside: list[str], offside: set[str]) -> AbstractSet[str]:
    return nearside & offside
";

const KEYSVIEW_ACCEPTED: &str = r"
from typing import AbstractSet, KeysView

def shared_labels(nearside: KeysView[str], offside: set[str]) -> AbstractSet[str]:
    return nearside & offside
";

const KEYSVIEW_ACCEPTED_RENAMED: &str = r"
from typing import AbstractSet, KeysView

def probe_overlap(scow: KeysView[str], barge: set[str]) -> AbstractSet[str]:
    return scow & barge
";

#[test]
fn keys_view_supports_set_algebra() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`KeysView` is an `AbstractSet` and supports `&`; `list` defines no `__and__`",
        rejected: KEYSVIEW_REJECTED,
        accepted: KEYSVIEW_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[renamed(KEYSVIEW_ACCEPTED_RENAMED)],
    }
    .assert("KeysView set algebra")
}

// ── AbstractSet vs MutableSet ────────────────────────────────────────────
// The read-only set protocol has no mutators. `add` arrives only with
// `MutableSet`.

const MUTABLESET_REJECTED: &str = r"
from typing import AbstractSet

def enrol(roll: AbstractSet[str], moniker: str) -> None:
    roll.add(moniker)
";

const MUTABLESET_ACCEPTED: &str = r"
from typing import MutableSet

def enrol(roll: MutableSet[str], moniker: str) -> None:
    roll.add(moniker)
";

const MUTABLESET_ACCEPTED_IMPORT_FORM: &str = r"
import collections.abc

def enrol(roll: collections.abc.MutableSet[str], moniker: str) -> None:
    roll.add(moniker)
";

#[test]
fn abstract_set_has_no_add() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`AbstractSet` is the read-only set protocol; `add` belongs to `MutableSet`",
        rejected: MUTABLESET_REJECTED,
        accepted: MUTABLESET_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[import_form(MUTABLESET_ACCEPTED_IMPORT_FORM)],
    }
    .assert("AbstractSet member set")
}

// ── Text ─────────────────────────────────────────────────────────────────
// `Text` is an alias for `str`, so a `bytes` argument is as ill-typed as it
// would be against `str` spelled directly.

const TEXT_REJECTED: &str = r#"
from typing import Text

def engrave(caption: Text) -> Text:
    return caption.upper()

engrave(b"marl")
"#;

const TEXT_ACCEPTED: &str = r#"
from typing import Text

def engrave(caption: Text) -> Text:
    return caption.upper()

engrave("marl")
"#;

const TEXT_REJECTED_ALIASED: &str = r#"
from typing import Text as Legend

def engrave(caption: Legend) -> Legend:
    return caption.upper()

engrave(b"marl")
"#;

#[test]
fn text_alias_is_str_not_bytes() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`Text` is an alias of `str`, so `bytes` does not satisfy it",
        rejected: TEXT_REJECTED,
        accepted: TEXT_ACCEPTED,
        rejected_variants: &[aliased(TEXT_REJECTED_ALIASED)],
        accepted_variants: &[],
    }
    .assert("Text alias identity")
}
