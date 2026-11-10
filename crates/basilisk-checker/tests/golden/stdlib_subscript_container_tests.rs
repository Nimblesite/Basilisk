//! Subscript obligations on parameterised builtin containers.
//! [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! `list[int].__setitem__` is `(SupportsIndex, int) -> None` and
//! `dict[str, int].__setitem__` is `(str, int) -> None`. Both the key and the
//! value are therefore checked on every subscript assignment. The element types
//! are reached bare, through a `builtins` alias, and through attribute access,
//! so a rule keyed to the literal text `list[int]` fails the A6/A7 legs.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── list value type ──────────────────────────────────────────────────────

#[test]
fn list_item_assignment_rejects_wrong_value_type() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`list[int].__setitem__` takes an `int` value; `str` is not assignable",
        rejected: r#"
tallies: list[int] = []
tallies[0] = "three"
"#,
        accepted: r#"
tallies: list[int] = []
tallies[0] = 3
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import int as Whole, list as Roster

tallies: Roster[Whole] = []
tallies[0] = "three"
"#,
            ),
            import_form(
                r#"
import builtins

tallies: builtins.list[builtins.int] = []
tallies[0] = "three"
"#,
            ),
            renamed(
                r#"
counts: list[int] = []
counts[0] = "three"
"#,
            ),
            reformatted(
                "
tallies : list[ int ] = [

]

tallies[0] = 'three'   # <- not a tally
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import int as Whole, list as Roster

tallies: Roster[Whole] = []
tallies[0] = 3
"#,
            ),
            import_form(
                r#"
import builtins

tallies: builtins.list[builtins.int] = []
tallies[0] = 3
"#,
            ),
            renamed(
                r#"
counts: list[int] = []
counts[0] = 3
"#,
            ),
        ],
    }
    .assert("list item assignment rejects wrong value type")
}

// ── list index type ──────────────────────────────────────────────────────

#[test]
fn list_index_must_support_index() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a list index must be `SupportsIndex`; `str` has no `__index__`",
        rejected: r#"
tallies: list[int] = []
tallies["zero"] = 3
"#,
        accepted: r#"
tallies: list[int] = []
tallies[0] = 3
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import int as Whole, list as Roster

tallies: Roster[Whole] = []
tallies["zero"] = 3
"#,
            ),
            import_form(
                r#"
import builtins

tallies: builtins.list[builtins.int] = []
tallies["zero"] = 3
"#,
            ),
            renamed(
                r#"
counts: list[int] = []
counts["zero"] = 3
"#,
            ),
            reformatted(
                "
tallies : list[int] = []

tallies[
    'zero'   # <- a name, not a position
] = 3
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import int as Whole, list as Roster

tallies: Roster[Whole] = []
tallies[0] = 3
"#,
            ),
            reformatted(
                "
tallies : list[int] = []

tallies[
    0
] = 3
",
            ),
        ],
    }
    .assert("list index must support index")
}

// ── dict key type ────────────────────────────────────────────────────────

#[test]
fn dict_subscript_assignment_rejects_wrong_key_type() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`dict[str, int].__setitem__` takes a `str` key; `int` is not assignable",
        rejected: r#"
settings: dict[str, int] = {}
settings[0] = 3
"#,
        accepted: r#"
settings: dict[str, int] = {}
settings["retries"] = 3
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import dict as Mapping, int as Whole, str as Text

settings: Mapping[Text, Whole] = {}
settings[0] = 3
"#,
            ),
            import_form(
                r#"
import builtins

settings: builtins.dict[builtins.str, builtins.int] = {}
settings[0] = 3
"#,
            ),
            renamed(
                r#"
knobs: dict[str, int] = {}
knobs[0] = 3
"#,
            ),
            reformatted(
                "
settings : dict[ str , int ] = {

}

settings[ 0 ] = 3   # <- an int where a name belongs
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import dict as Mapping, int as Whole, str as Text

settings: Mapping[Text, Whole] = {}
settings["retries"] = 3
"#,
            ),
            renamed(
                r#"
knobs: dict[str, int] = {}
knobs["retries"] = 3
"#,
            ),
        ],
    }
    .assert("dict subscript assignment rejects wrong key type")
}

// ── dict value type ──────────────────────────────────────────────────────

#[test]
fn dict_subscript_assignment_rejects_wrong_value_type() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`dict[str, int].__setitem__` takes an `int` value; `str` is not assignable",
        rejected: r#"
settings: dict[str, int] = {}
settings["retries"] = "three"
"#,
        accepted: r#"
settings: dict[str, int] = {}
settings["retries"] = 3
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import dict as Mapping, int as Whole, str as Text

settings: Mapping[Text, Whole] = {}
settings["retries"] = "three"
"#,
            ),
            import_form(
                r#"
import builtins

settings: builtins.dict[builtins.str, builtins.int] = {}
settings["retries"] = "three"
"#,
            ),
            renamed(
                r#"
knobs: dict[str, int] = {}
knobs["attempts"] = "three"
"#,
            ),
            reformatted(
                "
settings : dict[str, int] = {}

settings[
    'retries'
] = 'three'   # <- spelled out, not a number
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import dict as Mapping, int as Whole, str as Text

settings: Mapping[Text, Whole] = {}
settings["retries"] = 3
"#,
            ),
            reformatted(
                "
settings : dict[str, int] = {}

settings[
    'retries'
] = 3
",
            ),
        ],
    }
    .assert("dict subscript assignment rejects wrong value type")
}
