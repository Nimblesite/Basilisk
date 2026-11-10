//! Attribute qualifiers — `Final`, `ClassVar`, `Annotated` — under permutation.
//! [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! All four qualifier spellings this file exercises (`Final`, `final`,
//! `ClassVar`, `Annotated`) are inside the 55 `typing` symbols
//! `conformance/tests/` imports, so every one of them is **quarantined**: it
//! appears only under an alias (`Final as Riveted`, `final as keystone`,
//! `ClassVar as Wardwide`, `Annotated as Girded`) or through an alternate
//! import form (`typing.Final`), never bare.
//!
//! The types the qualifiers wrap are drawn from outside that vocabulary
//! entirely — `MutableMapping`, `SupportsBytes`, `Counter`, `Deque`,
//! `weakref.ReferenceType`, `functools.cached_property` — so no rule can carry
//! a hardcoded arm for them, and the identifiers are disjoint from the 913
//! names the suite defines.

use super::harness::{
    aliased, analyse, assert_accepted, assert_rejected, import_form, reformatted, renamed,
    SpecObligation,
};

#[test]
fn final_declaration_requires_a_value() -> Result<(), Box<dyn std::error::Error>> {
    let case = "Final without a value";
    let reason = "a `Final` name is initialized where it is declared, or not at all";
    assert_rejected(case, reason, FINAL_NO_VALUE_TYPED)?;
    SpecObligation {
        spec_reason: reason,
        rejected: FINAL_NO_VALUE_REJECTED,
        accepted: FINAL_NO_VALUE_ACCEPTED,
        rejected_variants: &[
            aliased(FINAL_NO_VALUE_REJECTED_ALIASED),
            import_form(FINAL_NO_VALUE_REJECTED_IMPORT_FORM),
            reformatted(FINAL_NO_VALUE_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[],
    }
    .assert(case)
}

// ── ClassVar: parameter position ─────────────────────────────────────────
// `ClassVar` is a declaration qualifier for class-body variables. It carries no
// meaning in a parameter annotation and the spec forbids it there outright,
// whatever type it wraps.

const CLASSVAR_PARAM_REJECTED: &str = r"
from typing import ClassVar as Wardwide, Counter

class Chandlery:
    berths: Wardwide[int] = 0

    def stow(self, ledger: Wardwide[Counter[str]]) -> int:
        return len(ledger) + self.berths
";

const CLASSVAR_PARAM_ACCEPTED: &str = r"
from typing import ClassVar as Wardwide, Counter

class Chandlery:
    berths: Wardwide[int] = 0

    def stow(self, ledger: Counter[str]) -> int:
        return len(ledger) + self.berths
";

const CLASSVAR_PARAM_REJECTED_IMPORT_FORM: &str = r"
import collections
import typing

class Chandlery:
    berths: typing.ClassVar[int] = 0

    def stow(self, ledger: typing.ClassVar[collections.Counter[str]]) -> int:
        return len(ledger) + self.berths
";

const CLASSVAR_PARAM_ACCEPTED_IMPORT_FORM: &str = r"
import collections
import typing

class Chandlery:
    berths: typing.ClassVar[int] = 0

    def stow(self, ledger: collections.Counter[str]) -> int:
        return len(ledger) + self.berths
";

#[test]
fn classvar_is_rejected_in_a_parameter_annotation() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`ClassVar` may not appear in a function parameter annotation",
        rejected: CLASSVAR_PARAM_REJECTED,
        accepted: CLASSVAR_PARAM_ACCEPTED,
        rejected_variants: &[import_form(CLASSVAR_PARAM_REJECTED_IMPORT_FORM)],
        accepted_variants: &[import_form(CLASSVAR_PARAM_ACCEPTED_IMPORT_FORM)],
    }
    .assert("ClassVar in a parameter annotation")
}

// ── ClassVar: assignment through an instance ─────────────────────────────
// A `ClassVar` may be read through an instance but only written through the
// class object. The repair changes the assignment target and nothing else.

const CLASSVAR_INSTANCE_REJECTED: &str = r"
from typing import ClassVar as Wardwide

class Scriptorium:
    quires: Wardwide[int] = 0

def audit(room: Scriptorium) -> int:
    room.quires = 12
    return room.quires
";

const CLASSVAR_INSTANCE_ACCEPTED: &str = r"
from typing import ClassVar as Wardwide

class Scriptorium:
    quires: Wardwide[int] = 0

def audit(room: Scriptorium) -> int:
    Scriptorium.quires = 12
    return room.quires
";

const CLASSVAR_INSTANCE_REJECTED_IMPORT_FORM: &str = r"
import typing

class Scriptorium:
    quires: typing.ClassVar[int] = 0

def audit(room: Scriptorium) -> int:
    room.quires = 12
    return room.quires
";

const CLASSVAR_INSTANCE_REJECTED_RENAMED: &str = r"
from typing import ClassVar as Wardwide

class Almonry:
    doles: Wardwide[int] = 0

def sift(hall: Almonry) -> int:
    hall.doles = 12
    return hall.doles
";

const CLASSVAR_INSTANCE_REJECTED_REFORMATTED: &str = r"
from typing import ClassVar as Wardwide


class Scriptorium:  # one count, shared by every reading room
        quires: Wardwide[int] = 0


def audit(room: Scriptorium) -> int:
        room.quires = 12   # written through the instance
        return (room).quires
";

#[test]
fn classvar_may_not_be_written_through_an_instance() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a `ClassVar` is assignable through the class, never through an instance",
        rejected: CLASSVAR_INSTANCE_REJECTED,
        accepted: CLASSVAR_INSTANCE_ACCEPTED,
        rejected_variants: &[
            import_form(CLASSVAR_INSTANCE_REJECTED_IMPORT_FORM),
            renamed(CLASSVAR_INSTANCE_REJECTED_RENAMED),
            reformatted(CLASSVAR_INSTANCE_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[],
    }
    .assert("ClassVar assignment through an instance")
}

// ── ClassVar: outside a class body ───────────────────────────────────────
// The qualifier is only meaningful on a class-body variable declaration.
// Module scope and function scope are both errors; moving the declaration into
// a class body repairs it.

const CLASSVAR_MODULE_REJECTED: &str = r"
from typing import ClassVar as Wardwide

MOORAGE: Wardwide[int] = 3
";

const CLASSVAR_MODULE_ACCEPTED: &str = r"
from typing import ClassVar as Wardwide

class Bollard:
    moorage: Wardwide[int] = 3
";

const CLASSVAR_MODULE_REJECTED_IMPORT_FORM: &str = r"
import typing

MOORAGE: typing.ClassVar[int] = 3
";

const CLASSVAR_LOCAL_REJECTED: &str = r"
from typing import ClassVar as Wardwide

def dredge() -> int:
    depth: Wardwide[int] = 3
    return depth
";

#[test]
fn classvar_is_confined_to_class_bodies() -> Result<(), Box<dyn std::error::Error>> {
    let case = "ClassVar outside a class body";
    let reason = "`ClassVar` qualifies a class-body declaration and no other kind";
    assert_rejected(case, reason, CLASSVAR_LOCAL_REJECTED)?;
    SpecObligation {
        spec_reason: reason,
        rejected: CLASSVAR_MODULE_REJECTED,
        accepted: CLASSVAR_MODULE_ACCEPTED,
        rejected_variants: &[import_form(CLASSVAR_MODULE_REJECTED_IMPORT_FORM)],
        accepted_variants: &[],
    }
    .assert(case)
}

// ── Annotated: the payload decides ───────────────────────────────────────
// `Annotated[X, ...]` is `X` for every typing purpose. `float` has no
// `__bytes__`, so it fails `SupportsBytes` whether or not the annotation is
// wrapped, and whatever the metadata claims.

const BYTES_PLAIN_REJECTED: &str = r#"
from typing import SupportsBytes

class Tanner:
    def __bytes__(self) -> bytes:
        return b"hide"

def brine(hide: SupportsBytes) -> bytes:
    return hide.__bytes__()

brine(Tanner())
brine(3.5)
"#;

const BYTES_PLAIN_ACCEPTED: &str = r#"
from typing import SupportsBytes

class Tanner:
    def __bytes__(self) -> bytes:
        return b"hide"

def brine(hide: SupportsBytes) -> bytes:
    return hide.__bytes__()

brine(Tanner())
"#;

const BYTES_GIRDED_REJECTED: &str = r#"
from typing import Annotated as Girded, SupportsBytes

class Tanner:
    def __bytes__(self) -> bytes:
        return b"hide"

def brine(hide: Girded[SupportsBytes, "salted"]) -> bytes:
    return hide.__bytes__()

brine(Tanner())
brine(3.5)
"#;

const BYTES_GIRDED_ACCEPTED: &str = r#"
from typing import Annotated as Girded, SupportsBytes

class Tanner:
    def __bytes__(self) -> bytes:
        return b"hide"

def brine(hide: Girded[SupportsBytes, "salted"]) -> bytes:
    return hide.__bytes__()

brine(Tanner())
"#;

/// Metadata that contradicts the payload. It is data, not a type, so the
/// verdict must be exactly the unwrapped one.
const BYTES_GIRDED_LYING_METADATA: &str = r#"
import typing

class Tanner:
    def __bytes__(self) -> bytes:
        return b"hide"

def brine(hide: typing.Annotated[typing.SupportsBytes, float, "any float is fine", 3]) -> bytes:
    return hide.__bytes__()

brine(Tanner())
brine(3.5)
"#;

const BYTES_GIRDED_REJECTED_RENAMED: &str = r#"
from typing import Annotated as Girded, SupportsBytes

class Cordwainer:
    def __bytes__(self) -> bytes:
        return b"welt"

def cure(pelt: Girded[SupportsBytes, "salted"]) -> bytes:
    return pelt.__bytes__()

cure(Cordwainer())
cure(3.5)
"#;

#[test]
fn annotated_payload_decides_the_verdict() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`Annotated[SupportsBytes, ...]` is `SupportsBytes`, and `float` is not one",
        rejected: BYTES_GIRDED_REJECTED,
        accepted: BYTES_GIRDED_ACCEPTED,
        rejected_variants: &[renamed(BYTES_GIRDED_REJECTED_RENAMED)],
        accepted_variants: &[],
    }
    .assert("Annotated payload conformance")
}

#[test]
fn annotated_metadata_never_moves_a_verdict() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        verdict(BYTES_PLAIN_REJECTED)?,
        verdict(BYTES_GIRDED_REJECTED)?,
        "wrapping a parameter annotation in `Annotated` changed the verdict. \
         The typing spec makes `Annotated[X, ...]` equivalent to `X`, so the \
         metadata must be inert. See [PERMTEST-FAMILY-B]."
    );
    assert_eq!(
        verdict(BYTES_PLAIN_ACCEPTED)?,
        verdict(BYTES_GIRDED_ACCEPTED)?,
        "`Annotated` metadata introduced a diagnostic on a well-typed program."
    );
    assert_eq!(
        verdict(BYTES_PLAIN_REJECTED)?,
        verdict(BYTES_GIRDED_LYING_METADATA)?,
        "metadata naming `float` changed the verdict. Metadata is data, never a \
         type, and can neither widen nor narrow the annotation it accompanies."
    );
    Ok(())
}

// ── Annotated: qualifier composition ─────────────────────────────────────
// `Final[Annotated[X, ...]]` is a `Final` declaration of `X`. The metadata must
// not disable the reassignment obligation.

const FINAL_ANNOTATED_REJECTED: &str = r#"
from typing import Annotated as Girded, Final as Riveted

BERTH_DEPTH: Riveted[Girded[float, "metres"]] = 4.5
BERTH_DEPTH = 5.5
"#;

const FINAL_ANNOTATED_ACCEPTED: &str = r#"
from typing import Annotated as Girded, Final as Riveted

BERTH_DEPTH: Riveted[Girded[float, "metres"]] = 4.5
SPAR_CLEARANCE: Girded[float, "metres"] = 5.5
"#;

const FINAL_ANNOTATED_REJECTED_IMPORT_FORM: &str = r#"
import typing

BERTH_DEPTH: typing.Final[typing.Annotated[float, "metres"]] = 4.5
BERTH_DEPTH = 5.5
"#;

const FINAL_ANNOTATED_REJECTED_REFORMATTED: &str = "
from typing import Annotated as Girded, Final as Riveted

# depth at the wharf, surveyed once

BERTH_DEPTH: Riveted[Girded[float, 'metres']] = 4.5
BERTH_DEPTH = 5.5   # the defect
";

#[test]
fn final_survives_an_annotated_payload() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`Final[Annotated[X, ...]]` freezes the name exactly as `Final[X]` does",
        rejected: FINAL_ANNOTATED_REJECTED,
        accepted: FINAL_ANNOTATED_ACCEPTED,
        rejected_variants: &[
            import_form(FINAL_ANNOTATED_REJECTED_IMPORT_FORM),
            reformatted(FINAL_ANNOTATED_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[],
    }
    .assert("Final over an Annotated payload")
}

// ── Annotated: arity ─────────────────────────────────────────────────────
// PEP 593 requires at least two arguments — a type and one or more metadata
// values. `Annotated[X]` is malformed, however legal `X` alone would be.

const ANNOTATED_ARITY_REJECTED: &str = r"
from typing import Annotated as Girded, Deque

def lade(cargo: Girded[Deque[str]]) -> int:
    return len(cargo)
";

const ANNOTATED_ARITY_ACCEPTED: &str = r#"
from typing import Annotated as Girded, Deque

def lade(cargo: Girded[Deque[str], "fifo"]) -> int:
    return len(cargo)
"#;

const ANNOTATED_ARITY_REJECTED_IMPORT_FORM: &str = r"
import collections
import typing

def lade(cargo: typing.Annotated[collections.deque[str]]) -> int:
    return len(cargo)
";

const ANNOTATED_ARITY_ACCEPTED_IMPORT_FORM: &str = r#"
import collections
import typing

def lade(cargo: typing.Annotated[collections.deque[str], "fifo"]) -> int:
    return len(cargo)
"#;

#[test]
fn annotated_requires_metadata() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`Annotated` takes a type plus at least one metadata value",
        rejected: ANNOTATED_ARITY_REJECTED,
        accepted: ANNOTATED_ARITY_ACCEPTED,
        rejected_variants: &[import_form(ANNOTATED_ARITY_REJECTED_IMPORT_FORM)],
        accepted_variants: &[import_form(ANNOTATED_ARITY_ACCEPTED_IMPORT_FORM)],
    }
    .assert("Annotated arity")
}

// ── Annotated: an accepted program stays accepted ────────────────────────
// The false-positive side. A `Deque[str]` argument satisfies the parameter
// whether or not the annotation carries metadata, and a `Counter[str]` does
// not — the metadata is irrelevant to both judgements.

const ANNOTATED_CALL_ACCEPTED: &str = r#"
from typing import Annotated as Girded, Deque

def lade(cargo: Girded[Deque[str], "fifo", 12]) -> int:
    return len(cargo)

lade(Deque[str]())
"#;

const ANNOTATED_CALL_REJECTED: &str = r#"
from typing import Annotated as Girded, Counter, Deque

def lade(cargo: Girded[Deque[str], "fifo", 12]) -> int:
    return len(cargo)

lade(Counter[str]())
"#;

#[test]
fn annotated_argument_compatibility_ignores_metadata() -> Result<(), Box<dyn std::error::Error>> {
    let case = "Annotated argument compatibility";
    let reason = "metadata cannot make an incompatible argument fit, or a fitting one fail";
    assert_accepted(case, reason, ANNOTATED_CALL_ACCEPTED)?;
    assert_rejected(case, reason, ANNOTATED_CALL_REJECTED)
}
