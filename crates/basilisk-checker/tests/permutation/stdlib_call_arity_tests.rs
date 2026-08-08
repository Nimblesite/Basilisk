//! Callability and arity obligations. [PERMTEST-FAMILY-B] / [PERMTEST-VOCABULARY].
//!
//! The typing spec's callable rules make two demands of every call expression:
//! the callee must have a `__call__`, and the argument list must match one of
//! the callee's signatures. Both are decided from the resolved type of the
//! callee — never from its spelling — so each case is exercised bare, aliased
//! (`from builtins import len as extent`), and through attribute access
//! (`builtins.len`).

use super::harness::{aliased, import_form, reformatted, renamed, SpecObligation};

// ── The callee must be callable ──────────────────────────────────────────

#[test]
fn calling_a_non_callable_int() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`int` has no `__call__`, so an `int` value cannot be called",
        rejected: r#"
bellows = 123
bellows()
"#,
        accepted: r#"
class Bellows:
    def __call__(self) -> int:
        return 123


bellows = Bellows()
bellows()
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import int as Whole

bellows: Whole = 123
bellows()
"#,
            ),
            import_form(
                r#"
import builtins

bellows: builtins.int = 123
bellows()
"#,
            ),
            renamed(
                r#"
flywheel = 123
flywheel()
"#,
            ),
            reformatted(
                "
bellows = 123

bellows(
)  # an int is not callable
",
            ),
        ],
        accepted_variants: &[
            renamed(
                r#"
class Flywheel:
    def __call__(self) -> int:
        return 123


flywheel = Flywheel()
flywheel()
"#,
            ),
            reformatted(
                "
class Bellows:

        def __call__(self) -> int:

                return 123

bellows = Bellows()

bellows(
)
",
            ),
        ],
    }
    .assert("calling a non-callable int")
}

// ── Too few arguments ────────────────────────────────────────────────────

#[test]
fn len_requires_one_argument() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`len` takes exactly one positional argument; calling it with none matches no \
                      signature",
        rejected: r#"
len()
"#,
        accepted: r#"
len([])
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import len as extent

extent()
"#,
            ),
            import_form(
                r#"
import builtins

builtins.len()
"#,
            ),
            reformatted(
                "
len(
)  # nothing to measure
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import len as extent

extent([])
"#,
            ),
            import_form(
                r#"
import builtins

builtins.len([])
"#,
            ),
            reformatted(
                "
len(
    [],
)
",
            ),
        ],
    }
    .assert("len requires one argument")
}

// ── Too many arguments ───────────────────────────────────────────────────

#[test]
fn len_rejects_a_second_argument() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`len` takes exactly one positional argument; a second matches no signature",
        rejected: r#"
len([], 1)
"#,
        accepted: r#"
len([])
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import len as extent

extent([], 1)
"#,
            ),
            import_form(
                r#"
import builtins

builtins.len([], 1)
"#,
            ),
            reformatted(
                "
len(
    [],
    1,   # one argument too many
)
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import len as extent

extent([])
"#,
            ),
            reformatted(
                "
len( [] )
",
            ),
        ],
    }
    .assert("len rejects a second argument")
}

// ── A constructor with no viable zero-argument overload ──────────────────

#[test]
fn type_requires_arguments() -> Result<(), Box<dyn std::error::Error>> {
    SpecObligation {
        spec_reason: "`type` has one- and three-argument overloads; neither accepts zero arguments",
        rejected: r#"
type()
"#,
        accepted: r#"
type(3)
"#,
        rejected_variants: &[
            aliased(
                r#"
from builtins import type as ClassOf

ClassOf()
"#,
            ),
            import_form(
                r#"
import builtins

builtins.type()
"#,
            ),
            reformatted(
                "
type(
)  # no operand to inspect
",
            ),
        ],
        accepted_variants: &[
            aliased(
                r#"
from builtins import type as ClassOf

ClassOf(3)
"#,
            ),
            import_form(
                r#"
import builtins

builtins.type(3)
"#,
            ),
            reformatted(
                "
type(
    3,
)
",
            ),
        ],
    }
    .assert("type requires arguments")
}
