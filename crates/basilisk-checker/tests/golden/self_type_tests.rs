//! `Self` obligations (PEP 673). [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! PEP 673 enumerates its rejections explicitly, and each one here is taken from
//! that list rather than invented:
//!
//! * "`def foo(bar: Self) -> Self: ...` # Rejected (not within a class)"
//! * "`bar: Self` # Rejected (not within a class)"
//! * "we reject `Self` in staticmethods"
//! * "we reject using `Self` with type arguments, such as `Self[int]`"
//!
//! Paired against the PEP's explicit acceptances — `@classmethod … -> Self`,
//! `__new__ … -> Self`, `Self` as a parameter type, and `Self` in an attribute
//! annotation — so a rule that rejects `Self` wholesale outside a return
//! position fails the accepted legs.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── Self must appear within a class ──────────────────────────────────────

#[test]
fn self_is_rejected_outside_a_class_body() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "PEP 673 rejects `Self` in a module-level function — there is no enclosing \
                      class for it to be bound to; inside a classmethod it is accepted",
        rejected: r#"
import typing


def bind(quire: typing.Self) -> typing.Self:
    return quire
"#,
        accepted: r#"
import typing


class Codex:
    @classmethod
    def bind(cls) -> typing.Self:
        return cls()
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import Self as Same


def bind(quire: Same) -> Same:
    return quire
"#,
            ),
            import_form(
                r#"
import typing_extensions


def bind(quire: typing_extensions.Self) -> typing_extensions.Self:
    return quire
"#,
            ),
            renamed(
                r#"
import typing


def stitch(folio: typing.Self) -> typing.Self:
    return folio
"#,
            ),
            reformatted(
                "
import typing

def bind(
    quire : typing.Self ,   # <- no enclosing class
) -> typing.Self :
        return quire
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import Self as Same


class Codex:
    @classmethod
    def bind(cls) -> Same:
        return cls()
"#,
            ),
            renamed(
                r#"
import typing


class Ledger:
    @classmethod
    def stitch(cls) -> typing.Self:
        return cls()
"#,
            ),
        ],
    }
    .assert("Self is rejected outside a class body")
}

// ── Self is rejected in a staticmethod ───────────────────────────────────

#[test]
fn self_is_rejected_in_a_staticmethod() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "PEP 673 rejects `Self` in staticmethods, since there is no `self` or `cls` \
                      for it to track; the same annotation on an instance method is accepted",
        rejected: r#"
import typing


class Codex:
    @staticmethod
    def bind() -> typing.Self:
        raise RuntimeError("no receiver")
"#,
        accepted: r#"
import typing


class Codex:
    def bind(self) -> typing.Self:
        return self
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import Self as Same


class Codex:
    @staticmethod
    def bind() -> Same:
        raise RuntimeError("no receiver")
"#,
            ),
            import_form(
                r#"
import typing_extensions


class Codex:
    @staticmethod
    def bind() -> typing_extensions.Self:
        raise RuntimeError("no receiver")
"#,
            ),
            renamed(
                r#"
import typing


class Ledger:
    @staticmethod
    def stitch() -> typing.Self:
        raise RuntimeError("no receiver")
"#,
            ),
            reformatted(
                "
import typing

class Codex:

        @staticmethod
        def bind() -> typing.Self :   # <- no receiver to bind Self to
            raise RuntimeError( 'no receiver' )
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import Self as Same


class Codex:
    def bind(self) -> Same:
        return self
"#,
            ),
            renamed(
                r#"
import typing


class Ledger:
    def stitch(self) -> typing.Self:
        return self
"#,
            ),
        ],
    }
    .assert("Self is rejected in a staticmethod")
}

// ── Self at module scope, versus in an attribute annotation ──────────────

#[test]
fn self_is_rejected_at_module_scope_but_allowed_on_an_attribute()
-> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "PEP 673 lists `bar: Self` at module level as rejected, while an attribute \
                      annotation inside a class is accepted and treated as returning `Self`",
        rejected: r#"
import typing

gutter: typing.Self
"#,
        accepted: r#"
import typing


class Codex:
    successor: typing.Self | None = None
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import Self as Same

gutter: Same
"#,
            ),
            import_form(
                r#"
import typing_extensions

gutter: typing_extensions.Self
"#,
            ),
            renamed(
                r#"
import typing

deckle: typing.Self
"#,
            ),
            reformatted(
                "
import typing

gutter : typing.Self   # <- module scope, no enclosing class
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import Self as Same


class Codex:
    successor: Same | None = None
"#,
            ),
            renamed(
                r#"
import typing


class Ledger:
    predecessor: typing.Self | None = None
"#,
            ),
        ],
    }
    .assert("Self is rejected at module scope but allowed on an attribute")
}

// ── Self takes no type arguments ─────────────────────────────────────────

#[test]
fn self_may_not_be_subscripted() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "PEP 673 rejects using `Self` with type arguments such as `Self[int]`; `Self` \
                      already denotes the enclosing class and takes no parameters",
        rejected: r#"
import typing


class Codex:
    def bind(self) -> typing.Self[int]:
        return self
"#,
        accepted: r#"
import typing


class Codex:
    def bind(self) -> typing.Self:
        return self
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import Self as Same


class Codex:
    def bind(self) -> Same[int]:
        return self
"#,
            ),
            import_form(
                r#"
import typing
import builtins


class Codex:
    def bind(self) -> typing.Self[builtins.int]:
        return self
"#,
            ),
            renamed(
                r#"
import typing


class Ledger:
    def stitch(self) -> typing.Self[int]:
        return self
"#,
            ),
            reformatted(
                "
import typing

class Codex:
        def bind( self ) -> typing.Self[
            int   # <- Self admits no type arguments
        ] :
            return self
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import Self as Same


class Codex:
    def bind(self) -> Same:
        return self
"#,
            ),
            renamed(
                r#"
import typing


class Ledger:
    def stitch(self) -> typing.Self:
        return self
"#,
            ),
        ],
    }
    .assert("Self may not be subscripted")
}

// ── __new__ is an accepted Self position ─────────────────────────────────

#[test]
fn self_is_accepted_in_new_and_as_a_parameter_type()
-> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "PEP 673 explicitly accepts `def __new__(cls, …) -> Self` and `Self` as a \
                      parameter annotation; rejecting either is a false positive. A module-level \
                      function keeps the rejection even when it merely takes `Self`",
        rejected: r#"
import typing


def collate(left: typing.Self, right: typing.Self) -> None:
    return None
"#,
        accepted: r#"
import typing


class Codex:
    def __new__(cls, leaves: int) -> typing.Self:
        return super().__new__(cls)

    def collate(self, other: typing.Self) -> None:
        return None
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import Self as Same


def collate(left: Same, right: Same) -> None:
    return None
"#,
            ),
            import_form(
                r#"
import typing_extensions


def collate(
    left: typing_extensions.Self, right: typing_extensions.Self
) -> None:
    return None
"#,
            ),
            renamed(
                r#"
import typing


def gather(recto: typing.Self, verso: typing.Self) -> None:
    return None
"#,
            ),
            reformatted(
                "
import typing

def collate(
    left  : typing.Self ,
    right : typing.Self ,   # <- still outside any class
) -> None :
        return None
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import Self as Same


class Codex:
    def __new__(cls, leaves: int) -> Same:
        return super().__new__(cls)

    def collate(self, other: Same) -> None:
        return None
"#,
            ),
            renamed(
                r#"
import typing


class Ledger:
    def __new__(cls, folios: int) -> typing.Self:
        return super().__new__(cls)

    def gather(self, peer: typing.Self) -> None:
        return None
"#,
            ),
        ],
    }
    .assert("Self is accepted in __new__ and as a parameter type")
}
