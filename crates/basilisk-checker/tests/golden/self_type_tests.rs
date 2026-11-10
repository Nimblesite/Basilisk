//! `Self` obligations ([PEP 673](https://peps.python.org/pep-0673/)).
//! [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! PEP 673 enumerates its rejections explicitly, and each one here is taken from
//! that list rather than invented:
//!
//! * A module-level function annotation is rejected because no class binds it.
//! * A module-level variable annotation is rejected for the same reason.
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
from typing import Self as enclosing_shape


def bind(quire: object) -> enclosing_shape:
    return quire
"#,
        accepted: r#"
from builtins import classmethod as subtype_constructor
from typing import Self as enclosing_shape


class Codex:
    @subtype_constructor
    def bind(cls) -> enclosing_shape:
        return cls()
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import Self as mirrored_shape


def bind(quire: object) -> mirrored_shape:
    return quire
"#,
            ),
            import_form(
                r#"
import typing as type_contracts


def bind(quire: object) -> type_contracts.Self:
    return quire
"#,
            ),
            renamed(
                r#"
from typing_extensions import Self as extension_shape


def stitch(folio: object) -> extension_shape:
    return folio
"#,
            ),
            reformatted(
                "
from typing import Self as enclosing_shape

def bind(
    quire : object ,
) -> enclosing_shape :   # <- no enclosing class
        return quire
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import classmethod as alternate_constructor
from typing import Self as mirrored_shape


class Codex:
    @alternate_constructor
    def bind(cls) -> mirrored_shape:
        return cls()
"#,
            ),
            renamed(
                r#"
import builtins as runtime_tools
import typing as type_contracts


class Ledger:
    @runtime_tools.classmethod
    def stitch(cls) -> type_contracts.Self:
        return cls()
"#,
            ),
        ],
    }
    .assert_by(
        "Self is rejected outside a class body",
        "generics_self_usage",
    )
}

// ── Self is rejected in a staticmethod ───────────────────────────────────

#[test]
fn self_is_rejected_in_a_staticmethod() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "PEP 673 rejects `Self` in staticmethods, since there is no `self` or `cls` \
                      for it to track; the same annotation on an instance method is accepted",
        rejected: r#"
from builtins import staticmethod as without_receiver
from typing import Self as enclosing_shape


class Codex:
    @without_receiver
    def bind() -> enclosing_shape:
        ...
"#,
        accepted: r#"
from typing import Self as enclosing_shape


class Codex:
    def bind(self) -> enclosing_shape:
        return self
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import staticmethod as detached_operation
from typing import Self as mirrored_shape


class Codex:
    @detached_operation
    def bind() -> mirrored_shape:
        ...
"#,
            ),
            import_form(
                r#"
import builtins as runtime_tools
import typing as type_contracts


class Codex:
    @runtime_tools.staticmethod
    def bind() -> type_contracts.Self:
        ...
"#,
            ),
            renamed(
                r#"
from builtins import staticmethod as detached_operation
from typing_extensions import Self as extension_shape


class Ledger:
    @detached_operation
    def stitch() -> extension_shape:
        ...
"#,
            ),
            reformatted(
                "
from builtins import staticmethod as without_receiver
from typing import Self as enclosing_shape

class Codex:

        @without_receiver
        def bind() -> enclosing_shape :   # <- no receiver to bind Self to
            ...
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import Self as mirrored_shape


class Codex:
    def bind(self) -> mirrored_shape:
        return self
"#,
            ),
            renamed(
                r#"
import typing as type_contracts


class Ledger:
    def stitch(self) -> type_contracts.Self:
        return self
"#,
            ),
        ],
    }
    .assert_by("Self is rejected in a staticmethod", "generics_self_usage")
}

// ── Self at module scope, versus in an attribute annotation ──────────────

#[test]
fn self_is_rejected_at_module_scope_but_allowed_on_an_attribute(
) -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason:
            "PEP 673 lists a module-level `Self` annotation as rejected, while an attribute \
                      annotation inside a class is accepted and treated as returning `Self`",
        rejected: r#"
from typing import Self as enclosing_shape

gutter: enclosing_shape
"#,
        accepted: r#"
from typing import Self as enclosing_shape


class Codex:
    successor: enclosing_shape | None = None
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import Self as mirrored_shape

gutter: mirrored_shape
"#,
            ),
            import_form(
                r#"
import typing as type_contracts

gutter: type_contracts.Self
"#,
            ),
            renamed(
                r#"
from typing_extensions import Self as extension_shape

deckle: extension_shape
"#,
            ),
            reformatted(
                "
from typing import Self as enclosing_shape

gutter : enclosing_shape   # <- module scope, no enclosing class
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import Self as mirrored_shape


class Codex:
    successor: mirrored_shape | None = None
"#,
            ),
            renamed(
                r#"
import typing as type_contracts


class Ledger:
    predecessor: type_contracts.Self | None = None
"#,
            ),
        ],
    }
    .assert_by(
        "Self is rejected at module scope but allowed on an attribute",
        "generics_self_usage",
    )
}

// ── Self takes no type arguments ─────────────────────────────────────────

#[test]
fn self_may_not_be_subscripted() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason:
            "PEP 673 rejects using `Self` with type arguments such as `Self[int]`; `Self` \
                      already denotes the enclosing class and takes no parameters",
        rejected: r#"
from typing import Self as enclosing_shape


class Codex:
    def bind(self) -> enclosing_shape[int]:
        return self
"#,
        accepted: r#"
from typing import Self as enclosing_shape


class Codex:
    def bind(self) -> enclosing_shape:
        return self
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import Self as mirrored_shape


class Codex:
    def bind(self) -> mirrored_shape[int]:
        return self
"#,
            ),
            import_form(
                r#"
import typing as type_contracts
import builtins


class Codex:
    def bind(self) -> type_contracts.Self[builtins.int]:
        return self
"#,
            ),
            renamed(
                r#"
from typing_extensions import Self as extension_shape


class Ledger:
    def stitch(self) -> extension_shape[int]:
        return self
"#,
            ),
            reformatted(
                "
from typing import Self as enclosing_shape

class Codex:
        def bind( self ) -> enclosing_shape[
            int   # <- Self admits no type arguments
        ] :
            return self
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import Self as mirrored_shape


class Codex:
    def bind(self) -> mirrored_shape:
        return self
"#,
            ),
            renamed(
                r#"
import typing as type_contracts


class Ledger:
    def stitch(self) -> type_contracts.Self:
        return self
"#,
            ),
        ],
    }
    .assert_by("Self may not be subscripted", "generics_self_basic")
}

// ── __new__ is an accepted Self position ─────────────────────────────────

#[test]
fn self_is_accepted_in_new_and_as_a_parameter_type() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "PEP 673 explicitly accepts `def __new__(cls, …) -> Self` and `Self` as a \
                      parameter annotation; rejecting either is a false positive. A module-level \
                      function keeps the rejection even when it merely takes `Self`",
        rejected: r#"
from typing import Self as enclosing_shape


def collate(left: enclosing_shape, right: object) -> None:
    return None
"#,
        accepted: r#"
from typing import Self as enclosing_shape


class Codex:
    def __new__(cls, leaves: int) -> enclosing_shape:
        return super().__new__(cls)

    def collate(self, other: enclosing_shape) -> None:
        return None
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import Self as mirrored_shape


def collate(left: mirrored_shape, right: object) -> None:
    return None
"#,
            ),
            import_form(
                r#"
import typing as type_contracts


def collate(
    left: type_contracts.Self, right: object
) -> None:
    return None
"#,
            ),
            renamed(
                r#"
from typing_extensions import Self as extension_shape


def gather(recto: extension_shape, verso: object) -> None:
    return None
"#,
            ),
            reformatted(
                "
from typing import Self as enclosing_shape

def collate(
    left  : enclosing_shape ,
    right : object ,   # <- still outside any class
) -> None :
        return None
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import Self as mirrored_shape


class Codex:
    def __new__(cls, leaves: int) -> mirrored_shape:
        return super().__new__(cls)

    def collate(self, other: mirrored_shape) -> None:
        return None
"#,
            ),
            renamed(
                r#"
import typing as type_contracts


class Ledger:
    def __new__(cls, folios: int) -> type_contracts.Self:
        return super().__new__(cls)

    def gather(self, peer: type_contracts.Self) -> None:
        return None
"#,
            ),
        ],
    }
    .assert_by(
        "Self is accepted in __new__ and as a parameter type",
        "generics_self_usage",
    )
}
