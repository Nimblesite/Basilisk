//! Overload definition, ordering and evaluation, judged against the typing
//! spec's `@overload` chapter. [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! `overload` is one of the 55 `typing` symbols `conformance/tests/` imports,
//! so it is **quarantined, not exempt**: it never appears bare here. Every
//! canonical source reaches it through `from typing import overload as
//! multi_signature`, and the A6/A7 variants respell it again — a different
//! alias, or `typing.overload` through the module. `Protocol` is quarantined
//! the same way. `get_overloads` is outside the suite's vocabulary entirely, so
//! no hardcoded arm can exist for it and it is used bare. Identifiers come from
//! a namespace disjoint from the suite's 913 defined names.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── An overload series needs an implementation ───────────────────────────
// Spec: in a `.py` file a run of `@overload`-decorated declarations must be
// followed by one implementation of the same name. A series with none is
// ill-formed — nothing would exist at runtime.

const NO_IMPL_REJECTED: &str = r"
from typing import overload as multi_signature
@multi_signature
def winnow(grain: int) -> int: ...
@multi_signature
def winnow(grain: str) -> str: ...
";

const NO_IMPL_ACCEPTED: &str = r"
from typing import overload as multi_signature
@multi_signature
def winnow(grain: int) -> int: ...
@multi_signature
def winnow(grain: str) -> str: ...
def winnow(grain: object) -> object: return grain
";

const NO_IMPL_REJECTED_IMPORT_FORM: &str = r"
import typing
@typing.overload
def winnow(grain: int) -> int: ...
@typing.overload
def winnow(grain: str) -> str: ...
";

const NO_IMPL_REJECTED_RENAMED: &str = r"
from typing import overload as multi_signature
@multi_signature
def thresher(sheaf: int) -> int: ...
@multi_signature
def thresher(sheaf: str) -> str: ...
";

const NO_IMPL_REJECTED_REFORMATTED: &str = r"
from typing import overload as multi_signature
# two declarations and nothing that implements them

@multi_signature
def winnow(
        grain: int,
) -> int:
        ...
@multi_signature
def winnow(grain: str) -> str:
    ...  # the series ends here
";

const NO_IMPL_ACCEPTED_ALIASED: &str = r"
from typing import overload as forked_signature
@forked_signature
def winnow(grain: int) -> int: ...
@forked_signature
def winnow(grain: str) -> str: ...
def winnow(grain: object) -> object: return grain
";

#[test]
fn overload_series_requires_an_implementation() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "an @overload series in a .py file must be followed by an implementation",
        rejected: NO_IMPL_REJECTED,
        accepted: NO_IMPL_ACCEPTED,
        rejected_variants: &[
            import_form(NO_IMPL_REJECTED_IMPORT_FORM),
            renamed(NO_IMPL_REJECTED_RENAMED),
            reformatted(NO_IMPL_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[aliased(NO_IMPL_ACCEPTED_ALIASED)],
    }
    .assert("overload implementation required")
}

// ── …and the implementation must come after the declarations ─────────────
// A definition preceding the series is an ordinary function that the
// declarations then shadow, leaving the series itself unimplemented.

const IMPL_FIRST_REJECTED: &str = r"
from typing import overload as multi_signature
def winnow(grain: object) -> object: return grain
@multi_signature
def winnow(grain: int) -> int: ...
@multi_signature
def winnow(grain: str) -> str: ...
";

#[test]
fn implementation_must_follow_the_declarations() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the implementation follows the @overload series; one that precedes it \
                      leaves the series unimplemented",
        rejected: IMPL_FIRST_REJECTED,
        accepted: NO_IMPL_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[],
    }
    .assert("overload implementation ordering")
}

// ── A series needs at least two declarations ─────────────────────────────
// Spec: at least two `@overload`-decorated definitions must be present. One
// declaration plus an implementation says nothing a plain signature could not.

const LONE_REJECTED: &str = r"
from typing import overload as multi_signature
@multi_signature
def bodkin(notch: int) -> int: ...
def bodkin(notch: object) -> object: return notch
";

const LONE_ACCEPTED: &str = r"
from typing import overload as multi_signature
@multi_signature
def bodkin(notch: int) -> int: ...
@multi_signature
def bodkin(notch: str) -> str: ...
def bodkin(notch: object) -> object: return notch
";

const LONE_REJECTED_ALIASED: &str = r"
from typing import overload as sole_signature
@sole_signature
def bodkin(notch: int) -> int: ...
def bodkin(notch: object) -> object: return notch
";

#[test]
fn a_single_overload_declaration_is_an_error() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "an overload series requires at least two @overload declarations",
        rejected: LONE_REJECTED,
        accepted: LONE_ACCEPTED,
        rejected_variants: &[aliased(LONE_REJECTED_ALIASED)],
        accepted_variants: &[],
    }
    .assert("single overload declaration")
}

// ── The implementation must accommodate every declaration ────────────────
// Spec: each overload signature must be assignable to the implementation
// signature. An implementation taking only `str` cannot serve a declaration
// that accepts `int`.

const IMPL_COMPAT_REJECTED: &str = r"
from typing import overload as multi_signature
@multi_signature
def reckon(levy: int) -> int: ...
@multi_signature
def reckon(levy: str) -> str: ...
def reckon(levy: str) -> str: return levy
";

const IMPL_COMPAT_ACCEPTED: &str = r"
from typing import overload as multi_signature
@multi_signature
def reckon(levy: int) -> int: ...
@multi_signature
def reckon(levy: str) -> str: ...
def reckon(levy: object) -> object: return levy
";

const IMPL_COMPAT_REJECTED_RENAMED: &str = r"
from typing import overload as multi_signature
@multi_signature
def tribute(burden: int) -> int: ...
@multi_signature
def tribute(burden: str) -> str: ...
def tribute(burden: str) -> str: return burden
";

#[test]
fn implementation_must_serve_every_declaration() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "every overload signature must be assignable to the implementation signature",
        rejected: IMPL_COMPAT_REJECTED,
        accepted: IMPL_COMPAT_ACCEPTED,
        rejected_variants: &[renamed(IMPL_COMPAT_REJECTED_RENAMED)],
        accepted_variants: &[],
    }
    .assert("implementation compatibility")
}

// ── Two declarations differing only in return type ───────────────────────
// Spec: a declaration fully obscured by an earlier one is unreachable — no
// call can ever select it — and is an error. Identical parameter lists with
// different return types are the pure case.

const OBSCURED_REJECTED: &str = r"
from typing import overload as multi_signature
@multi_signature
def portcullis(gate: int) -> bytes: ...
@multi_signature
def portcullis(gate: int) -> str: ...
def portcullis(gate: int) -> object: return gate
";

const OBSCURED_ACCEPTED: &str = r"
from typing import overload as multi_signature
@multi_signature
def portcullis(gate: int) -> bytes: ...
@multi_signature
def portcullis(gate: str) -> str: ...
def portcullis(gate: object) -> object: return gate
";

const OBSCURED_REJECTED_ALIASED: &str = r"
from typing import overload as branching_signature
@branching_signature
def portcullis(gate: int) -> bytes: ...
@branching_signature
def portcullis(gate: int) -> str: ...
def portcullis(gate: int) -> object: return gate
";

#[test]
fn declarations_differing_only_in_return_type() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "identical parameters make the second declaration unreachable, since \
                      selection never gets past the first",
        rejected: OBSCURED_REJECTED,
        accepted: OBSCURED_ACCEPTED,
        rejected_variants: &[aliased(OBSCURED_REJECTED_ALIASED)],
        accepted_variants: &[],
    }
    .assert("obscured overload declaration")
}

// ── A call must match some declaration ───────────────────────────────────
// Spec: declarations are tried in order and the first whose parameters accept
// the arguments wins; a call matching none is an error. `float` is neither
// `int` nor `str`, and the promotion the spec grants runs the other way.

const NO_MATCH_REJECTED: &str = r#"
from typing import overload as multi_signature
@multi_signature
def smelt(charge: int) -> bytes: ...
@multi_signature
def smelt(charge: str) -> bytes: ...
def smelt(charge: object) -> bytes: return b"slag"

smelt(3.5)
"#;

const NO_MATCH_ACCEPTED: &str = r#"
from typing import overload as multi_signature
@multi_signature
def smelt(charge: int) -> bytes: ...
@multi_signature
def smelt(charge: str) -> bytes: ...
def smelt(charge: object) -> bytes: return b"slag"

smelt(3)
"#;

const NO_MATCH_REJECTED_IMPORT_FORM: &str = r#"
import typing
@typing.overload
def smelt(charge: int) -> bytes: ...
@typing.overload
def smelt(charge: str) -> bytes: ...
def smelt(charge: object) -> bytes: return b"slag"

smelt(3.5)
"#;
