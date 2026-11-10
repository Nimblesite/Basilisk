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

/// The run's diagnostic codes, sorted — the spelling-independent verdict.
///
/// Used where the equivalence under test is a *typing-spec* equivalence rather
/// than one of the [PERMTEST-FAMILY-A] respelling classes: `Annotated[X, ...]`
/// and `X` are different programs that the spec requires the checker to judge
/// identically.
pub(super) fn verdict(source: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut codes: Vec<String> = analyse(source)?
        .iter()
        .map(|diag| diag.code.code.to_owned())
        .collect();
    codes.sort();
    Ok(codes)
}

// ── Final: reassignment ──────────────────────────────────────────────────
// The typing spec makes a name declared `Final` un-rebindable: there may be
// exactly one assignment to it, the one at its declaration. The qualifier is a
// property of the *declaration*, so it binds however the symbol was spelled.

pub(super) const REASSIGN_REJECTED: &str = r#"
from collections.abc import MutableMapping
from typing import Final as Riveted

TOLL_TABLE: Riveted[MutableMapping[str, int]] = {"chalk": 2}
TOLL_TABLE = {"slate": 3}
"#;

pub(super) const REASSIGN_ACCEPTED: &str = r#"
from collections.abc import MutableMapping
from typing import Final as Riveted

TOLL_TABLE: Riveted[MutableMapping[str, int]] = {"chalk": 2}
SPARE_TABLE: MutableMapping[str, int] = {"slate": 3}
SPARE_TABLE = {"birch": 5}
"#;

pub(super) const REASSIGN_REJECTED_ALIASED: &str = r#"
from collections.abc import MutableMapping
from typing import Final as Bulwark

TOLL_TABLE: Bulwark[MutableMapping[str, int]] = {"chalk": 2}
TOLL_TABLE = {"slate": 3}
"#;

pub(super) const REASSIGN_REJECTED_IMPORT_FORM: &str = r#"
import typing
from collections.abc import MutableMapping

TOLL_TABLE: typing.Final[MutableMapping[str, int]] = {"chalk": 2}
TOLL_TABLE = {"slate": 3}
"#;

pub(super) const REASSIGN_REJECTED_RENAMED: &str = r#"
from collections.abc import MutableMapping
from typing import Final as Riveted

WHARFAGE: Riveted[MutableMapping[str, int]] = {"chalk": 2}
WHARFAGE = {"slate": 3}
"#;

pub(super) const REASSIGN_REJECTED_REFORMATTED: &str = "
from collections.abc import MutableMapping
from typing import Final as Riveted
# the toll table is fixed for the whole season


TOLL_TABLE: Riveted[MutableMapping[str, int]] = (
        {'chalk': 2}
)

TOLL_TABLE  =  {'slate': 3}   # the defect, one line down
";

pub(super) const REASSIGN_ACCEPTED_IMPORT_FORM: &str = r#"
import typing
from collections.abc import MutableMapping

TOLL_TABLE: typing.Final[MutableMapping[str, int]] = {"chalk": 2}
SPARE_TABLE: MutableMapping[str, int] = {"slate": 3}
SPARE_TABLE = {"birch": 5}
"#;

#[test]
fn final_name_may_not_be_rebound() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a `Final` declaration admits exactly one assignment, its own",
        rejected: REASSIGN_REJECTED,
        accepted: REASSIGN_ACCEPTED,
        rejected_variants: &[
            aliased(REASSIGN_REJECTED_ALIASED),
            import_form(REASSIGN_REJECTED_IMPORT_FORM),
            renamed(REASSIGN_REJECTED_RENAMED),
            reformatted(REASSIGN_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[import_form(REASSIGN_ACCEPTED_IMPORT_FORM)],
    }
    .assert("Final name rebinding")
}

// ── Final: attribute override ────────────────────────────────────────────
// A `Final` attribute declared in a class body may not be redeclared by a
// subclass. Declaring an unrelated attribute is untouched by the rule.

pub(super) const FINAL_ATTR_REJECTED: &str = r"
from typing import Final as Riveted

class Quernstone:
    girth: Riveted[int] = 30

class Millrace(Quernstone):
    girth: int = 40
";

pub(super) const FINAL_ATTR_ACCEPTED: &str = r"
from typing import Final as Riveted

class Quernstone:
    girth: Riveted[int] = 30

class Millrace(Quernstone):
    breadth: int = 40
";

pub(super) const FINAL_ATTR_REJECTED_IMPORT_FORM: &str = r"
import typing

class Quernstone:
    girth: typing.Final[int] = 30

class Millrace(Quernstone):
    girth: int = 40
";

pub(super) const FINAL_ATTR_REJECTED_RENAMED: &str = r"
from typing import Final as Riveted

class Trestle:
    tare: Riveted[int] = 30

class Corbel(Trestle):
    tare: int = 40
";

pub(super) const FINAL_ATTR_REJECTED_REFORMATTED: &str = r"
from typing import Final as Riveted


# the girth of a millstone never changes once dressed
class Quernstone:

        girth: Riveted[int] = 30


class Millrace(
        Quernstone
):
        girth: int = 40
";

#[test]
fn final_attribute_may_not_be_overridden() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a subclass may not redeclare an attribute the base declared `Final`",
        rejected: FINAL_ATTR_REJECTED,
        accepted: FINAL_ATTR_ACCEPTED,
        rejected_variants: &[
            import_form(FINAL_ATTR_REJECTED_IMPORT_FORM),
            renamed(FINAL_ATTR_REJECTED_RENAMED),
            reformatted(FINAL_ATTR_REJECTED_REFORMATTED),
        ],
        accepted_variants: &[],
    }
    .assert("Final attribute override")
}

// ── Final: method override ───────────────────────────────────────────────
// A method decorated `@final` may not be overridden in a subclass; adding a
// *new* method to the subclass is unrelated to the decorator.

pub(super) const FINAL_METHOD_REJECTED: &str = r"
from typing import final as keystone

class Portcullis:
    @keystone
    def winch(self) -> int:
        return 1

class Barbican(Portcullis):
    def winch(self) -> int:
        return 2
";

pub(super) const FINAL_METHOD_ACCEPTED: &str = r"
from typing import final as keystone

class Portcullis:
    @keystone
    def winch(self) -> int:
        return 1

class Barbican(Portcullis):
    def dredge(self) -> int:
        return 2
";

pub(super) const FINAL_METHOD_REJECTED_IMPORT_FORM: &str = r"
import typing

class Portcullis:
    @typing.final
    def winch(self) -> int:
        return 1

class Barbican(Portcullis):
    def winch(self) -> int:
        return 2
";

pub(super) const FINAL_METHOD_REJECTED_RENAMED: &str = r"
from typing import final as keystone

class Windlass:
    @keystone
    def haul(self) -> int:
        return 1

class Bollard(Windlass):
    def haul(self) -> int:
        return 2
";

pub(super) const FINAL_METHOD_ACCEPTED_ALIASED: &str = r"
from typing import final as sealed_here

class Portcullis:
    @sealed_here
    def winch(self) -> int:
        return 1

class Barbican(Portcullis):
    def dredge(self) -> int:
        return 2
";

#[test]
fn final_method_may_not_be_overridden() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`@final` on a method forbids any subclass redefinition of that name",
        rejected: FINAL_METHOD_REJECTED,
        accepted: FINAL_METHOD_ACCEPTED,
        rejected_variants: &[
            import_form(FINAL_METHOD_REJECTED_IMPORT_FORM),
            renamed(FINAL_METHOD_REJECTED_RENAMED),
        ],
        accepted_variants: &[aliased(FINAL_METHOD_ACCEPTED_ALIASED)],
    }
    .assert("final method override")
}

// ── Final: class subclassing ─────────────────────────────────────────────
// `@final` on a class forbids subclassing it. The two sources below differ
// only in which class carries the decorator, so a rule that reacts to the
// decorator's presence rather than to what it decorates cannot tell them apart.

pub(super) const FINAL_CLASS_REJECTED: &str = r"
import functools
from typing import final as keystone

@keystone
class Hauberk:
    @functools.cached_property
    def heft(self) -> int:
        return 12

class Gambeson(Hauberk):
    pass
";

pub(super) const FINAL_CLASS_ACCEPTED: &str = r"
import functools
from typing import final as keystone

class Hauberk:
    @functools.cached_property
    def heft(self) -> int:
        return 12

@keystone
class Gambeson(Hauberk):
    pass
";

pub(super) const FINAL_CLASS_REJECTED_IMPORT_FORM: &str = r"
import functools
import typing

@typing.final
class Hauberk:
    @functools.cached_property
    def heft(self) -> int:
        return 12

class Gambeson(Hauberk):
    pass
";

pub(super) const FINAL_CLASS_ACCEPTED_IMPORT_FORM: &str = r"
import functools
import typing

class Hauberk:
    @functools.cached_property
    def heft(self) -> int:
        return 12

@typing.final
class Gambeson(Hauberk):
    pass
";

#[test]
fn final_class_may_not_be_subclassed() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`@final` on a class forbids deriving from it; on a leaf it forbids nothing",
        rejected: FINAL_CLASS_REJECTED,
        accepted: FINAL_CLASS_ACCEPTED,
        rejected_variants: &[import_form(FINAL_CLASS_REJECTED_IMPORT_FORM)],
        accepted_variants: &[import_form(FINAL_CLASS_ACCEPTED_IMPORT_FORM)],
    }
    .assert("final class subclassing")
}

// ── Final: declaration without a value ───────────────────────────────────
// A `Final` name must be given its value where it is declared. Bare `Final`
// with no type argument and no assignment leaves the checker nothing to infer
// from and nothing to freeze, and is an error on both counts.

pub(super) const FINAL_NO_VALUE_REJECTED: &str = r"
from typing import Final as Riveted

FREEBOARD: Riveted
";

pub(super) const FINAL_NO_VALUE_ACCEPTED: &str = r"
from typing import Final as Riveted

FREEBOARD: Riveted = 2.5
";

pub(super) const FINAL_NO_VALUE_REJECTED_ALIASED: &str = r"
from typing import Final as Bulwark

FREEBOARD: Bulwark
";

pub(super) const FINAL_NO_VALUE_REJECTED_IMPORT_FORM: &str = r"
import typing

FREEBOARD: typing.Final
";

pub(super) const FINAL_NO_VALUE_REJECTED_REFORMATTED: &str = r"
from typing import Final as Riveted


# declared, but never given a value


FREEBOARD  :  Riveted
";

/// A `Final` declaration with a full type and still no value — an error for
/// the qualifier's own reason, independent of inference.
pub(super) const FINAL_NO_VALUE_TYPED: &str = r"
import weakref
from typing import Final as Riveted

class Cellarer:
    pass

CURRENT_WARDEN: Riveted[weakref.ReferenceType[Cellarer]]
";
