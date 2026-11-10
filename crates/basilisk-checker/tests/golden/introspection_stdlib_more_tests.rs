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
#[allow(
    clippy::wildcard_imports,
    unused_imports,
    reason = "shared golden fixtures: each sibling uses the subset it references"
)]
use super::introspection_stdlib::*;

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
// `Counter.__add__` sums the per-key tallies and yields another `Counter` of
// the same key type. It never collapses to a scalar.

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

// ── BinaryIO and binary-mode open() ──────────────────────────────────────
// `BinaryIO` is `IO[bytes]`, and the `"rb"` mode literal selects the binary
// overload of `open`. Either way `read` yields `bytes`; nothing decodes it.

const BINARY_REJECTED: &str = r#"
from typing import BinaryIO

def first_octets(handle: BinaryIO) -> str:
    return handle.read(16)

def preamble(waybill: str) -> str:
    with open(waybill, "rb") as spout:
        return spout.read(16)
"#;

const BINARY_ACCEPTED: &str = r#"
from typing import BinaryIO

def first_octets(handle: BinaryIO) -> bytes:
    return handle.read(16)

def preamble(waybill: str) -> bytes:
    with open(waybill, "rb") as spout:
        return spout.read(16)
"#;

const BINARY_ACCEPTED_ALIASED: &str = r#"
from typing import BinaryIO as OctetStream

def first_octets(handle: OctetStream) -> bytes:
    return handle.read(16)

def preamble(waybill: str) -> bytes:
    with open(waybill, "rb") as spout:
        return spout.read(16)
"#;

const BINARY_ACCEPTED_REFORMATTED: &str = "
from typing import BinaryIO

def first_octets(handle: BinaryIO) -> bytes:
        return handle.read(16)

def preamble(waybill: str) -> bytes:
        # binary mode, so no decoding happens
        with open(waybill, 'rb') as spout:
                return spout.read(16)
";

#[test]
fn binary_streams_read_bytes_not_str() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`BinaryIO` is `IO[bytes]` and `open(..., \"rb\")` selects the binary \
                      overload, so `read` yields `bytes`",
        rejected: BINARY_REJECTED,
        accepted: BINARY_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[
            aliased(BINARY_ACCEPTED_ALIASED),
            reformatted(BINARY_ACCEPTED_REFORMATTED),
        ],
    }
    .assert("binary stream element type")
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
// `list` does not: there is no `list.__and__`.

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
// The read-only set protocol carries no mutators; `add` arrives only with
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
