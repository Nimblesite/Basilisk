//! Every rule-side subtype verdict routes through the module-seeded
//! `subtyping::SubtypingContext` — [NARROWPLAN-SUBTYPING],
//! [NARROWPLAN-INTEGRATION] ("one subtyping implementation"),
//! [TYPEINF-SUBTYPING-NOMINAL]. See
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-SUBTYPING
//! and `crates/basilisk-checker/src/subtyping.rs`.
//!
//! Mutation-resistant pins: each positive test here passes ONLY because the
//! rule consults the module's registered class hierarchy (a same-module
//! subclass is accepted where the bare numeric tower would reject), and each
//! is paired with a negative that keeps the diagnostic alive for a genuinely
//! unrelated class. Reverting any migrated rule to a tower-only or
//! rule-local table breaks the positive; deleting the verdict breaks the
//! negative.

use super::common::*;

/// `generics_defaults_2` ([TYPEINF-SUBTYPING-NOMINAL]): a `TypeVar` default
/// that SUBCLASSES the bound satisfies it — nominal edge, not tower.
#[test]
fn typevar_default_subclass_of_bound_is_accepted() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar


class Base:
    pass


class Sub(Base):
    pass


T = TypeVar("T", bound=Base, default=Sub)
"#;
    let diags = run(source)?;
    let fired: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code.contains("generics_defaults"))
        .map(|d| d.message.clone())
        .collect();
    assert!(
        fired.is_empty(),
        "`Sub` subclasses `Base` — the module-seeded context must accept the \
         default; a tower-only verdict rejects it. Got: {fired:?}"
    );
    Ok(())
}

/// The paired negative: an unrelated default still fires — the routing must
/// not silence the diagnostic itself.
#[test]
fn typevar_default_unrelated_to_bound_still_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar


class Base:
    pass


class Elsewhere:
    pass


T = TypeVar("T", bound=Base, default=Elsewhere)
"#;
    let diags = run(source)?;
    assert!(
        diags
            .iter()
            .any(|d| d.code.code.contains("generics_defaults")),
        "`Elsewhere` does not satisfy `bound=Base` — the diagnostic must \
         survive the context routing. Got: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

/// `generics_defaults_referential` ([TYPEINF-SUBTYPING-NOMINAL]): a
/// referenced `TypeVar` whose bound subclasses the referencing bound is
/// compatible under PEP 696 — again a nominal edge.
#[test]
fn referential_default_bound_subclass_is_accepted() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar


class Animal:
    pass


class Dog(Animal):
    pass


T1 = TypeVar("T1", bound=Dog)
T2 = TypeVar("T2", bound=Animal, default=T1)
"#;
    let diags = run(source)?;
    let fired: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code.contains("generics_defaults_referential"))
        .map(|d| d.message.clone())
        .collect();
    assert!(
        fired.is_empty(),
        "`Dog` (T1's bound) subclasses `Animal` (T2's bound) — PEP 696 \
         accepts the referential default through the nominal walk. Got: {fired:?}"
    );
    Ok(())
}

/// The referential negative: reversed bounds (referenced bound is the
/// SUPERCLASS) still violate PEP 696 and must fire.
#[test]
fn referential_default_bound_superclass_still_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar


class Animal:
    pass


class Dog(Animal):
    pass


T1 = TypeVar("T1", bound=Animal)
T2 = TypeVar("T2", bound=Dog, default=T1)
"#;
    let diags = run(source)?;
    assert!(
        diags
            .iter()
            .any(|d| d.code.code.contains("generics_defaults_referential")),
        "`Animal` is not a subtype of `Dog` — the referential bound check \
         must keep firing. Got: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    Ok(())
}

/// The shared context-free helper (`rules::shared::is_type_compatible`,
/// [TYPEINF-SUBTYPING-UNION]): a union-typed SOURCE is accepted when every
/// alternative fits the target — the context splits both sides, the old
/// hand-rolled helper split only the target.
#[test]
fn union_source_every_alternative_fits_is_accepted() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def takes_float(value: float) -> float:
    return value


def pick(flag: bool, small: bool, big: int) -> None:
    mixed: int | bool = big if flag else small
    takes_float(mixed)
";
    let diags = run(source)?;
    let fired: Vec<_> = diags
        .iter()
        .filter(|d| d.message.contains("int | bool"))
        .map(|d| d.message.clone())
        .collect();
    assert!(
        fired.is_empty(),
        "every member of `int | bool` is a subtype of `float` — the \
         both-sides union split must accept it. Got: {fired:?}"
    );
    Ok(())
}
