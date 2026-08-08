//! `@override` obligations ([PEP 698](https://peps.python.org/pep-0698/)).
//! [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! The spec states the rule as a single conjunction: a method decorated with
//! `@override` is an error *unless* it overrides "a method or attribute in some
//! ancestor class" **and** "the type of the overriding method is assignable to
//! the type of the overridden method". Both halves are load-bearing, and each
//! half has an edge a name-matching implementation misses:
//!
//! * *some ancestor* — not merely the direct base. A method introduced by a
//!   grandparent and overridden two levels down satisfies the rule.
//! * *or attribute* — a plain annotated class attribute is a legitimate
//!   override target, not only a `def`.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── @override needs something to override ────────────────────────────────

#[test]
fn override_requires_an_overridden_member() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the spec makes `@override` an error unless the method overrides a member in \
                      some ancestor class; a name introduced fresh in the subclass overrides \
                      nothing",
        rejected: r#"
from typing import override as confirms_ancestry


class Abutment:
    def bear(self, load: int) -> None:
        return None


class Voussoir(Abutment):
    @confirms_ancestry
    def splay(self, load: int) -> None:
        return None
"#,
        accepted: r#"
from typing import override as confirms_ancestry


class Abutment:
    def bear(self, load: int) -> None:
        return None


class Voussoir(Abutment):
    @confirms_ancestry
    def bear(self, load: int) -> None:
        return None
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import override as supersedes


class Abutment:
    def bear(self, load: int) -> None:
        return None


class Voussoir(Abutment):
    @supersedes
    def splay(self, load: int) -> None:
        return None
"#,
            ),
            import_form(
                r#"
import typing_extensions


class Abutment:
    def bear(self, load: int) -> None:
        return None


class Voussoir(Abutment):
    @typing_extensions.override
    def splay(self, load: int) -> None:
        return None
"#,
            ),
            renamed(
                r#"
import typing


class Impost:
    def carry(self, weight: int) -> None:
        return None


class Springer(Impost):
    @typing.override
    def flare(self, weight: int) -> None:
        return None
"#,
            ),
            reformatted(
                "
import typing

class Abutment:

        def bear( self , load : int ) -> None :
            return None

class Voussoir( Abutment ):

        @typing.override
        def splay( self , load : int ) -> None :   # <- no such member above
            return None
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import override as supersedes


class Abutment:
    def bear(self, load: int) -> None:
        return None


class Voussoir(Abutment):
    @supersedes
    def bear(self, load: int) -> None:
        return None
"#,
            ),
            renamed(
                r#"
import typing


class Impost:
    def carry(self, weight: int) -> None:
        return None


class Springer(Impost):
    @typing.override
    def carry(self, weight: int) -> None:
        return None
"#,
            ),
        ],
    }
    .assert_by(
        "override requires an overridden member",
        "classes_override_3",
    )
}

// ── "some ancestor", not just the direct base ────────────────────────────

#[test]
fn override_is_satisfied_by_any_ancestor_not_only_the_direct_base(
) -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason:
            "the spec says *some ancestor class*, so a member introduced by a grandparent \
                      and skipped by the intermediate class is still a valid override target; \
                      only a name absent from the whole MRO is an error",
        rejected: r#"
from typing import override as confirms_ancestry


class Abutment:
    def bear(self, load: int) -> None:
        return None


class Pier(Abutment):
    pass


class Voussoir(Pier):
    @confirms_ancestry
    def splay(self, load: int) -> None:
        return None
"#,
        accepted: r#"
from typing import override as confirms_ancestry


class Abutment:
    def bear(self, load: int) -> None:
        return None


class Pier(Abutment):
    pass


class Voussoir(Pier):
    @confirms_ancestry
    def bear(self, load: int) -> None:
        return None
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import override as supersedes


class Abutment:
    def bear(self, load: int) -> None:
        return None


class Pier(Abutment):
    pass


class Voussoir(Pier):
    @supersedes
    def splay(self, load: int) -> None:
        return None
"#,
            ),
            import_form(
                r#"
import typing_extensions


class Abutment:
    def bear(self, load: int) -> None:
        return None


class Pier(Abutment):
    pass


class Voussoir(Pier):
    @typing_extensions.override
    def splay(self, load: int) -> None:
        return None
"#,
            ),
            renamed(
                r#"
import typing


class Impost:
    def carry(self, weight: int) -> None:
        return None


class Keystone(Impost):
    pass


class Springer(Keystone):
    @typing.override
    def flare(self, weight: int) -> None:
        return None
"#,
            ),
            reformatted(
                "
import typing

class Abutment:
        def bear( self , load : int ) -> None : return None

class Pier( Abutment ): pass

class Voussoir( Pier ):
        @typing.override
        def splay( self , load : int ) -> None :   # <- absent from the whole MRO
            return None
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import override as supersedes


class Abutment:
    def bear(self, load: int) -> None:
        return None


class Pier(Abutment):
    pass


class Voussoir(Pier):
    @supersedes
    def bear(self, load: int) -> None:
        return None
"#,
            ),
            renamed(
                r#"
import typing


class Impost:
    def carry(self, weight: int) -> None:
        return None


class Keystone(Impost):
    pass


class Springer(Keystone):
    @typing.override
    def carry(self, weight: int) -> None:
        return None
"#,
            ),
        ],
    }
    .assert_by(
        "override is satisfied by any ancestor, not only the direct base",
        "classes_override_3",
    )
}

// ── the overriding type must be assignable to the overridden ─────────────

#[test]
fn an_overriding_method_must_stay_assignable_to_what_it_overrides(
) -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the second half of the rule requires the overriding method's type to be \
                      assignable to the overridden method's type; narrowing a parameter breaks \
                      that, since a caller holding the base type may pass the wider value",
        rejected: r#"
from typing import override as confirms_ancestry


class Abutment:
    def bear(self, load: object) -> None:
        return None


class Voussoir(Abutment):
    @confirms_ancestry
    def bear(self, load: int) -> None:
        return None
"#,
        accepted: r#"
from typing import override as confirms_ancestry


class Abutment:
    def bear(self, load: int) -> None:
        return None


class Voussoir(Abutment):
    @confirms_ancestry
    def bear(self, load: object) -> None:
        return None
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import override as supersedes
from builtins import object as Base


class Abutment:
    def bear(self, load: Base) -> None:
        return None


class Voussoir(Abutment):
    @supersedes
    def bear(self, load: int) -> None:
        return None
"#,
            ),
            import_form(
                r#"
import typing
import builtins


class Abutment:
    def bear(self, load: builtins.object) -> None:
        return None


class Voussoir(Abutment):
    @typing.override
    def bear(self, load: builtins.int) -> None:
        return None
"#,
            ),
            renamed(
                r#"
import typing


class Impost:
    def carry(self, weight: object) -> None:
        return None


class Springer(Impost):
    @typing.override
    def carry(self, weight: int) -> None:
        return None
"#,
            ),
            reformatted(
                "
import typing

class Abutment:
        def bear( self , load : object ) -> None :
            return None

class Voussoir( Abutment ):
        @typing.override
        def bear(
            self ,
            load : int ,   # <- narrower than the member it overrides
        ) -> None :
            return None
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import override as supersedes
from builtins import object as Base


class Abutment:
    def bear(self, load: int) -> None:
        return None


class Voussoir(Abutment):
    @supersedes
    def bear(self, load: Base) -> None:
        return None
"#,
            ),
            renamed(
                r#"
import typing


class Impost:
    def carry(self, weight: int) -> None:
        return None


class Springer(Impost):
    @typing.override
    def carry(self, weight: object) -> None:
        return None
"#,
            ),
        ],
    }
    .assert_by(
        "an overriding method must stay assignable to what it overrides",
        "classes_override",
    )
}

// ── "a method or attribute" ──────────────────────────────────────────────

#[test]
fn override_may_target_an_attribute_not_only_a_method() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "the spec permits overriding \"a method or attribute\", so a base-class \
                      attribute is a legitimate target; an implementation that searches ancestors \
                      for `def`s alone rejects lawful code",
        rejected: r#"
from typing import Callable as zero_input_measurement
from typing import override as confirms_ancestry


class Abutment:
    span: zero_input_measurement[[], int]


class Voussoir(Abutment):
    @confirms_ancestry
    def rise(self) -> int:
        return 0
"#,
        accepted: r#"
from typing import Callable as zero_input_measurement
from typing import override as confirms_ancestry


class Abutment:
    span: zero_input_measurement[[], int]


class Voussoir(Abutment):
    @confirms_ancestry
    def span(self) -> int:
        return 0
"#,
        rejected_variants: &[
            aliased(
                r#"
from typing import Callable as zero_input_measurement
from typing import override as supersedes


class Abutment:
    span: zero_input_measurement[[], int]


class Voussoir(Abutment):
    @supersedes
    def rise(self) -> int:
        return 0
"#,
            ),
            import_form(
                r#"
import typing as type_contracts
import typing_extensions as compatibility_contracts


class Abutment:
    span: type_contracts.Callable[[], int]


class Voussoir(Abutment):
    @compatibility_contracts.override
    def rise(self) -> int:
        return 0
"#,
            ),
            renamed(
                r#"
from typing import Callable as zero_input_measurement
from typing import override as confirms_ancestry


class Impost:
    reach: zero_input_measurement[[], int]


class Springer(Impost):
    @confirms_ancestry
    def lift(self) -> int:
        return 0
"#,
            ),
            reformatted(
                "
from typing import Callable as zero_input_measurement
from typing import override as confirms_ancestry

class Abutment:

        span : zero_input_measurement[ [ ] , int ]

class Voussoir( Abutment ):
        @confirms_ancestry
        def rise( self ) -> int :   # <- neither method nor attribute above
            return 0
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from typing import Callable as zero_input_measurement
from typing import override as supersedes


class Abutment:
    span: zero_input_measurement[[], int]


class Voussoir(Abutment):
    @supersedes
    def span(self) -> int:
        return 0
"#,
            ),
            renamed(
                r#"
from typing import Callable as zero_input_measurement
from typing import override as confirms_ancestry


class Impost:
    reach: zero_input_measurement[[], int]


class Springer(Impost):
    @confirms_ancestry
    def reach(self) -> int:
        return 0
"#,
            ),
        ],
    }
    .assert_by(
        "override may target an attribute, not only a method",
        "classes_override_3",
    )
}
