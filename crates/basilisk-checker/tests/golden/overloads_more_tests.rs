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
#[allow(
    clippy::wildcard_imports,
    unused_imports,
    reason = "shared golden fixtures: each sibling uses the subset it references"
)]
use super::overloads::*;

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
// Selection weighs the whole parameter list, not just types: three arguments
// match neither the one-parameter nor the two-parameter declaration.

const ARITY_REJECTED: &str = r"
from typing import overload as multi_signature
@multi_signature
def bailiwick(shire: int) -> int: ...
@multi_signature
def bailiwick(shire: int, hundred: int) -> str: ...
def bailiwick(shire: int, hundred: int = 0) -> object: return shire

bailiwick(1, 2, 3)
";

const ARITY_ACCEPTED: &str = r"
from typing import overload as multi_signature
@multi_signature
def bailiwick(shire: int) -> int: ...
@multi_signature
def bailiwick(shire: int, hundred: int) -> str: ...
def bailiwick(shire: int, hundred: int = 0) -> object: return shire

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
// Callers see only the declarations. The implementation may be wider — here it
// accepts `bytes` — and that width is invisible at the call site.

const HIDDEN_REJECTED: &str = r#"
from typing import overload as multi_signature
@multi_signature
def stow(crate: int) -> int: ...
@multi_signature
def stow(crate: str) -> str: ...
def stow(crate: int | str | bytes) -> int | str | bytes: return crate

stow(b"canvas")
"#;

const HIDDEN_ACCEPTED: &str = r#"
from typing import overload as multi_signature
@multi_signature
def stow(crate: int) -> int: ...
@multi_signature
def stow(crate: str) -> str: ...
def stow(crate: int | str | bytes) -> int | str | bytes: return crate

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
        spec_reason: "only the declarations form the callable type, so an implementation \
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
def appraise(cask: object) -> object: return cask

verdict: bytes = appraise("silver")
"#;

const SELECTED_RETURN_ACCEPTED: &str = r#"
from typing import overload as multi_signature
@multi_signature
def appraise(cask: int) -> bytes: ...
@multi_signature
def appraise(cask: str) -> bool: ...
def appraise(cask: object) -> object: return cask

verdict: bool = appraise("silver")
"#;

const SELECTED_RETURN_REJECTED_RENAMED: &str = r#"
from typing import overload as multi_signature
@multi_signature
def assay(firkin: int) -> bytes: ...
@multi_signature
def assay(firkin: str) -> bool: ...
def assay(firkin: object) -> object: return firkin

ruling: bytes = assay("silver")
"#;

const SELECTED_RETURN_ACCEPTED_IMPORT_FORM: &str = r#"
import typing
@typing.overload
def appraise(cask: int) -> bytes: ...
@typing.overload
def appraise(cask: str) -> bool: ...
def appraise(cask: object) -> object: return cask

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
// members are declarations by construction; the same series in an ordinary
// class body still lacks its implementation. Demanding a body in the protocol
// would be a false positive on correct code.

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
                      implementation, while an ordinary class body still does",
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
def quernstone(freight: object) -> object: return freight

get_overloads(220)
";

const REGISTRY_ACCEPTED: &str = r"
from typing import get_overloads
from typing import overload as multi_signature
@multi_signature
def quernstone(freight: int) -> int: ...
@multi_signature
def quernstone(freight: str) -> str: ...
def quernstone(freight: object) -> object: return freight

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
