//! Unpacking and iteration obligations. [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! A tuple target on the left of `=` demands an iterable on the right, and when
//! the right-hand side has a statically known length that length must match the
//! number of targets. `for` demands the same iterable protocol. `int` provides
//! neither `__iter__` nor `__getitem__`, so it satisfies none of them.

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── Unpacking a non-iterable ─────────────────────────────────────────────

#[test]
fn unpacking_requires_an_iterable() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`int` implements neither `__iter__` nor `__getitem__`, so it cannot be \
                      unpacked",
        rejected: r#"
crest, trough = 1
"#,
        accepted: r#"
crest, trough = (1, 2)
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import int as Whole

reading: Whole = 1
crest, trough = reading
"#,
            ),
            import_form(
                r#"
import builtins

reading: builtins.int = 1
crest, trough = reading
"#,
            ),
            renamed(
                r#"
summit, hollow = 1
"#,
            ),
            reformatted(
                "
(
    crest,
    trough,
) = 1   # <- a scalar cannot fill two names
",
            ),
        ],
        accepted_variants: &[
            renamed(
                r#"
summit, hollow = (1, 2)
"#,
            ),
            reformatted(
                "
(
    crest,
    trough,
) = (
    1,
    2,
)
",
            ),
        ],
    }
    .assert("unpacking requires an iterable")
}

// ── Too many values ──────────────────────────────────────────────────────

#[test]
fn unpacking_rejects_surplus_values() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a 3-tuple cannot be unpacked into 2 targets without a starred target",
        rejected: r#"
crest, trough = (1, 2, 3)
"#,
        accepted: r#"
crest, trough, shoal = (1, 2, 3)
"#,
        rejected_variants: &[
            renamed(
                r#"
summit, hollow = (1, 2, 3)
"#,
            ),
            reformatted(
                "
(
    crest,
    trough,
) = (
    1,
    2,
    3,   # <- one value with nowhere to go
)
",
            ),
            aliased(
                r#"
from builtins import tuple as Fixed

readings: Fixed[int, int, int] = (1, 2, 3)
crest, trough = readings
"#,
            ),
        ],
        accepted_variants: &[
            renamed(
                r#"
summit, hollow, hillock = (1, 2, 3)
"#,
            ),
            reformatted(
                "
(
    crest,
    trough,
    shoal,
) = (
    1,
    2,
    3,
)
",
            ),
        ],
    }
    .assert("unpacking rejects surplus values")
}

// ── Too few values ───────────────────────────────────────────────────────

#[test]
fn unpacking_rejects_missing_values() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "a 1-tuple cannot fill 2 unpacking targets",
        rejected: r#"
crest, trough = (1,)
"#,
        accepted: r#"
crest, trough = (1, 2)
"#,
        rejected_variants: &[
            renamed(
                r#"
summit, hollow = (1,)
"#,
            ),
            reformatted(
                "
(
    crest,
    trough,   # <- nothing supplies this one
) = (
    1,
)
",
            ),
            aliased(
                r#"
from builtins import tuple as Fixed

readings: Fixed[int] = (1,)
crest, trough = readings
"#,
            ),
        ],
        accepted_variants: &[
            renamed(
                r#"
summit, hollow = (1, 2)
"#,
            ),
            reformatted(
                "
crest , trough = ( 1 , 2 )
",
            ),
        ],
    }
    .assert("unpacking rejects missing values")
}

// ── Iterating a non-iterable ─────────────────────────────────────────────

#[test]
fn for_loop_requires_an_iterable() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`for` calls `__iter__` on its operand; `int` has none",
        rejected: r#"
sundial = 123

for shadow in sundial:
    pass
"#,
        accepted: r#"
sundial = [123]

for shadow in sundial:
    pass
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import int as Whole

sundial: Whole = 123

for shadow in sundial:
    pass
"#,
            ),
            import_form(
                r#"
import builtins

sundial: builtins.int = 123

for shadow in sundial:
    pass
"#,
            ),
            renamed(
                r#"
gnomon = 123

for cast in gnomon:
    pass
"#,
            ),
            reformatted(
                "
sundial = 123

# an int is not a sequence of anything
for shadow in ( sundial ):
        pass
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import int as Whole, list as Roster

sundial: Roster[Whole] = [123]

for shadow in sundial:
    pass
"#,
            ),
            renamed(
                r#"
gnomon = [123]

for cast in gnomon:
    pass
"#,
            ),
        ],
    }
    .assert("for loop requires an iterable")
}
