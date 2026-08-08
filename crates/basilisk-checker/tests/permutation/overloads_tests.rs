//! Overload definition, ordering and evaluation, judged against the typing
//! spec's `@overload` chapter. [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! `overload` is one of the 55 `typing` symbols `conformance/tests/` imports,
//! so it is **quarantined, not exempt**: it never appears bare here. Every
//! canonical source reaches it through `from typing import overload as
//! multi_signature`, and the A6/A7 variants respell it again — a different
//! alias, or `typing.overload` through the module. `Protocol` is quarantined the
//! same way. `get_overloads` is outside the suite's vocabulary entirely, so no
//! hardcoded arm can exist for it and it is used bare.
//!
//! Identifiers come from a namespace disjoint from the suite's 913 defined
//! names, so nothing here can be recognised by a fixture-shaped branch.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── An overload series needs an implementation ───────────────────────────
// Spec: in a `.py` file a run of `@overload`-decorated declarations must be
// followed by a single implementation of the same name. A series with none is
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
def winnow(grain: object) -> object:
    return grain
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
def winnow(grain: object) -> object:
    return grain
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
// Spec: the implementation follows the series. A definition preceding the
// declarations is a separate function that the declarations then shadow,
// leaving the series itself unimplemented.

const IMPL_FIRST_REJECTED: &str = r"
from typing import overload as multi_signature

def winnow(grain: object) -> object:
    return grain

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
// declaration plus an implementation expresses nothing an ordinary signature
// could not, and is an error.

const LONE_REJECTED: &str = r"
from typing import overload as multi_signature

@multi_signature
def bodkin(notch: int) -> int: ...
def bodkin(notch: object) -> object:
    return notch
";

const LONE_ACCEPTED: &str = r"
from typing import overload as multi_signature

@multi_signature
def bodkin(notch: int) -> int: ...
@multi_signature
def bodkin(notch: str) -> str: ...
def bodkin(notch: object) -> object:
    return notch
";

const LONE_REJECTED_ALIASED: &str = r"
from typing import overload as sole_signature

@sole_signature
def bodkin(notch: int) -> int: ...
def bodkin(notch: object) -> object:
    return notch
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
def reckon(levy: str) -> str:
    return levy
";

const IMPL_COMPAT_ACCEPTED: &str = r"
from typing import overload as multi_signature

@multi_signature
def reckon(levy: int) -> int: ...
@multi_signature
def reckon(levy: str) -> str: ...
def reckon(levy: object) -> object:
    return levy
";

const IMPL_COMPAT_REJECTED_RENAMED: &str = r"
from typing import overload as multi_signature

@multi_signature
def tribute(burden: int) -> int: ...
@multi_signature
def tribute(burden: str) -> str: ...
def tribute(burden: str) -> str:
    return burden
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
// Spec: an overload fully obscured by an earlier one is unreachable — no call
// can ever select it — and is reported as an error. Identical parameter lists
// with different return types are the pure case.

const OBSCURED_REJECTED: &str = r"
from typing import overload as multi_signature

@multi_signature
def portcullis(gate: int) -> bytes: ...
@multi_signature
def portcullis(gate: int) -> str: ...
def portcullis(gate: int) -> object:
    return gate
";

const OBSCURED_ACCEPTED: &str = r"
from typing import overload as multi_signature

@multi_signature
def portcullis(gate: int) -> bytes: ...
@multi_signature
def portcullis(gate: str) -> str: ...
def portcullis(gate: object) -> object:
    return gate
";

const OBSCURED_REJECTED_ALIASED: &str = r"
from typing import overload as branching_signature

@branching_signature
def portcullis(gate: int) -> bytes: ...
@branching_signature
def portcullis(gate: int) -> str: ...
def portcullis(gate: int) -> object:
    return gate
";

#[test]
fn declarations_differing_only_in_return_type() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "identical parameters make the second declaration unreachable, since \
                      selection never reaches it",
        rejected: OBSCURED_REJECTED,
        accepted: OBSCURED_ACCEPTED,
        rejected_variants: &[aliased(OBSCURED_REJECTED_ALIASED)],
        accepted_variants: &[],
    }
    .assert("obscured overload declaration")
}

// ── A call must match some declaration ───────────────────────────────────
// Spec: declarations are tried in order and the first whose parameters accept
// the arguments wins. A call matching none of them is an error. `float` is
// neither `int` nor `str`, and promotion runs the other way.

const NO_MATCH_REJECTED: &str = r#"
from typing import overload as multi_signature

@multi_signature
def smelt(charge: int) -> bytes: ...
@multi_signature
def smelt(charge: str) -> bytes: ...
def smelt(charge: object) -> bytes:
    return b"slag"

smelt(3.5)
"#;

const NO_MATCH_ACCEPTED: &str = r#"
from typing import overload as multi_signature

@multi_signature
def smelt(charge: int) -> bytes: ...
@multi_signature
def smelt(charge: str) -> bytes: ...
def smelt(charge: object) -> bytes:
    return b"slag"

smelt(3)
"#;

const NO_MATCH_REJECTED_IMPORT_FORM: &str = r#"
import typing

@typing.overload
def smelt(charge: int) -> bytes: ...
@typing.overload
def smelt(charge: str) -> bytes: ...
def smelt(charge: object) -> bytes:
    return b"slag"

smelt(3.5)
"#;

#[test]
fn a_call_matching_no_declaration_is_an_error() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`float` matches neither declared parameter type, so no overload is selected",
        rejected: NO_MATCH_REJECTED,
        accepted: NO_MATCH_ACCEPTED,
        rejected_variants: &[import_form(NO_MATCH_REJECTED_IMPORT_FORM)],
        accepted_variants: &[],
    }
    .assert("no matching overload by argument type")
}

// ── …including by argument count ─────────────────────────────────────────
// Selection is over the whole parameter list, not just types: three arguments
// match neither the one-parameter nor the two-parameter declaration.

const ARITY_REJECTED: &str = r"
from typing import overload as multi_signature

@multi_signature
def bailiwick(shire: int) -> int: ...
@multi_signature
def bailiwick(shire: int, hundred: int) -> str: ...
def bailiwick(shire: int, hundred: int = 0) -> object:
    return shire

bailiwick(1, 2, 3)
";

const ARITY_ACCEPTED: &str = r"
from typing import overload as multi_signature

@multi_signature
def bailiwick(shire: int) -> int: ...
@multi_signature
def bailiwick(shire: int, hundred: int) -> str: ...
def bailiwick(shire: int, hundred: int = 0) -> object:
    return shire

bailiwick(1, 2)
";

#[test]
fn a_call_matching_no_declared_arity_is_an_error() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "no declaration accepts three arguments, so no overload is selected",
        rejected: ARITY_REJECTED,
        accepted: ARITY_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[],
    }
    .assert("no matching overload by arity")
}

// ── The implementation signature is not the callable surface ─────────────
// Spec: callers see only the declarations. The implementation may be wider —
// here it accepts `bytes` — and that width is invisible at the call site.

const HIDDEN_REJECTED: &str = r#"
from typing import overload as multi_signature

@multi_signature
def stow(crate: int) -> int: ...
@multi_signature
def stow(crate: str) -> str: ...
def stow(crate: int | str | bytes) -> int | str | bytes:
    return crate

stow(b"canvas")
"#;

const HIDDEN_ACCEPTED: &str = r#"
from typing import overload as multi_signature

@multi_signature
def stow(crate: int) -> int: ...
@multi_signature
def stow(crate: str) -> str: ...
def stow(crate: int | str | bytes) -> int | str | bytes:
    return crate

stow("canvas")
"#;

const HIDDEN_REJECTED_REFORMATTED: &str = r"
from typing import overload as multi_signature

@multi_signature
def stow(crate: int) -> int: ...

@multi_signature
def stow(crate: str) -> str: ...

def stow(
    crate: (int | str | bytes),
) -> int | str | bytes:
    return crate

# bytes reaches the implementation but no declaration admits it
stow(b'canvas')
";

#[test]
fn implementation_width_is_invisible_to_callers() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "only the declared overloads form the callable type; the implementation \
                      accepting `bytes` does not admit a `bytes` argument",
        rejected: HIDDEN_REJECTED,
        accepted: HIDDEN_ACCEPTED,
        rejected_variants: &[reformatted(HIDDEN_REJECTED_REFORMATTED)],
        accepted_variants: &[],
    }
    .assert("implementation signature not callable surface")
}

// ── The result type comes from the declaration that matched ──────────────
// Spec: the call evaluates to the return type of the selected declaration. A
// `str` argument selects the second, whose return is `bool`; binding that to
// the first declaration's `bytes` is ill-typed.

const SELECTED_RETURN_REJECTED: &str = r#"
from typing import overload as multi_signature

@multi_signature
def appraise(cask: int) -> bytes: ...
@multi_signature
def appraise(cask: str) -> bool: ...
def appraise(cask: object) -> object:
    return cask

verdict: bytes = appraise("silver")
"#;

const SELECTED_RETURN_ACCEPTED: &str = r#"
from typing import overload as multi_signature

@multi_signature
def appraise(cask: int) -> bytes: ...
@multi_signature
def appraise(cask: str) -> bool: ...
def appraise(cask: object) -> object:
    return cask

verdict: bool = appraise("silver")
"#;

const SELECTED_RETURN_REJECTED_RENAMED: &str = r#"
from typing import overload as multi_signature

@multi_signature
def assay(firkin: int) -> bytes: ...
@multi_signature
def assay(firkin: str) -> bool: ...
def assay(firkin: object) -> object:
    return firkin

ruling: bytes = assay("silver")
"#;

const SELECTED_RETURN_ACCEPTED_IMPORT_FORM: &str = r#"
import typing

@typing.overload
def appraise(cask: int) -> bytes: ...
@typing.overload
def appraise(cask: str) -> bool: ...
def appraise(cask: object) -> object:
    return cask

verdict: bool = appraise("silver")
"#;

#[test]
fn result_type_comes_from_the_selected_declaration() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a `str` argument selects the second declaration, so the call has type \
                      `bool`, not the first declaration's `bytes`",
        rejected: SELECTED_RETURN_REJECTED,
        accepted: SELECTED_RETURN_ACCEPTED,
        rejected_variants: &[renamed(SELECTED_RETURN_REJECTED_RENAMED)],
        accepted_variants: &[import_form(SELECTED_RETURN_ACCEPTED_IMPORT_FORM)],
    }
    .assert("selected declaration return type")
}

// ── Protocol members declare, they do not implement ──────────────────────
// Spec: the implementation requirement is waived inside a protocol body, whose
// members are declarations by construction. The same series in an ordinary
// class body is still missing its implementation. This is the false-positive
// direction — demanding a body here would reject correct code.

const METHOD_NO_IMPL_REJECTED: &str = r"
from typing import overload as multi_signature

class Sluice:
    @multi_signature
    def decant(self, volume: int) -> int: ...
    @multi_signature
    def decant(self, volume: str) -> str: ...
";

const PROTOCOL_MEMBER_ACCEPTED: &str = r"
from typing import Protocol as StructuralContract
from typing import overload as multi_signature

class Sluice(StructuralContract):
    @multi_signature
    def decant(self, volume: int) -> int: ...
    @multi_signature
    def decant(self, volume: str) -> str: ...
";

const PROTOCOL_MEMBER_ACCEPTED_IMPORT_FORM: &str = r"
import typing

class Sluice(typing.Protocol):
    @typing.overload
    def decant(self, volume: int) -> int: ...
    @typing.overload
    def decant(self, volume: str) -> str: ...
";

#[test]
fn protocol_members_need_no_implementation() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a protocol body declares members, so an overload series there needs no \
                      implementation; an ordinary class body still does",
        rejected: METHOD_NO_IMPL_REJECTED,
        accepted: PROTOCOL_MEMBER_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[import_form(PROTOCOL_MEMBER_ACCEPTED_IMPORT_FORM)],
    }
    .assert("protocol overload members")
}

// ── The overload registry accessor ───────────────────────────────────────
// `get_overloads` is outside the conformance suite's vocabulary, so it is used
// bare. Its parameter is a callable; an `int` argument is ill-typed.

const REGISTRY_REJECTED: &str = r"
from typing import get_overloads
from typing import overload as multi_signature

@multi_signature
def quernstone(freight: int) -> int: ...
@multi_signature
def quernstone(freight: str) -> str: ...
def quernstone(freight: object) -> object:
    return freight

get_overloads(220)
";

const REGISTRY_ACCEPTED: &str = r"
from typing import get_overloads
from typing import overload as multi_signature

@multi_signature
def quernstone(freight: int) -> int: ...
@multi_signature
def quernstone(freight: str) -> str: ...
def quernstone(freight: object) -> object:
    return freight

get_overloads(quernstone)
";

#[test]
fn overload_registry_lookup_takes_a_callable() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`get_overloads` declares a callable parameter, which `220` is not",
        rejected: REGISTRY_REJECTED,
        accepted: REGISTRY_ACCEPTED,
        rejected_variants: &[],
        accepted_variants: &[],
    }
    .assert("get_overloads argument type")
}
