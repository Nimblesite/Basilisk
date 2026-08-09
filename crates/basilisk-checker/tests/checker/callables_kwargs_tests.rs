//! PEP 692 tests for precise `**kwargs` typing with `Unpack[TypedDict]`.
//! <https://peps.python.org/pep-0692/#keyword-collisions>

use super::common::*;

#[test]
fn pep_692_unpack_kwargs_without_standard_parameter_collision_is_valid(
) -> Result<(), Box<dyn std::error::Error>> {
    let accepted = [
        (
            "canonical imports",
            r#"
from typing import TypedDict, Unpack
class LaunchOptions(TypedDict):
    destination: str
    retries: int
def launch(**options: Unpack[LaunchOptions]) -> None: ...
"#,
        ),
        (
            "aliased imports and renamed vocabulary",
            r#"
from typing import TypedDict as Shape, Unpack as Spread
class KilnSettings(Shape):
    temperature: int
    fuel: str
def ignite(**settings: Spread[KilnSettings]) -> None: ...
"#,
        ),
        (
            "qualified imports",
            r#"
import typing as contracts
class VoyagePlan(contracts.TypedDict):
    harbour: str
    tide: int
def depart(**plan: contracts.Unpack[VoyagePlan]) -> None: ...
"#,
        ),
        (
            "positional-only collision required to be accepted",
            r#"
from typing import TypedDict as Shape, Unpack as Spread
class Parcel(Shape):
    label: str
def dispatch(label: str, /, **parcel: Spread[Parcel]) -> None: ...
"#,
        ),
        (
            "reformatted annotation",
            r#"
from typing import TypedDict as Shape, Unpack as Spread
class ObservatoryConfig(Shape):
    aperture: int
    mode: str
def observe(
    **configuration:
        Spread[
            ObservatoryConfig
        ],
) -> None: ...
"#,
        ),
    ];

    for (mutation, source) in accepted {
        let diagnostics = run(source)?;
        assert!(
            diagnostics.is_empty(),
            "{mutation}: valid PEP 692 kwargs must be accepted: {diagnostics:#?}"
        );
        assert_eq!(
            codes(&diagnostics),
            Vec::<&str>::new(),
            "{mutation}: spelling must not affect acceptance"
        );
        assert_rule_count(
            &diagnostics,
            "callables_kwargs",
            0,
            "PEP 692 accepted kwargs declaration",
        );
    }

    Ok(())
}

#[test]
fn pep_692_unpack_kwargs_key_cannot_collide_with_standard_parameter(
) -> Result<(), Box<dyn std::error::Error>> {
    let rejected = [
        (
            "canonical imports",
            r#"
from typing import TypedDict, Unpack
class LaunchOptions(TypedDict):
    destination: str
    retries: int
def launch(destination: str, **options: Unpack[LaunchOptions]) -> None: ...
"#,
        ),
        (
            "aliased imports and renamed vocabulary",
            r#"
from typing import TypedDict as Shape, Unpack as Spread
class KilnSettings(Shape):
    temperature: int
    fuel: str
def ignite(temperature: int, **settings: Spread[KilnSettings]) -> None: ...
"#,
        ),
        (
            "qualified imports",
            r#"
import typing as contracts
class VoyagePlan(contracts.TypedDict):
    harbour: str
    tide: int
def depart(harbour: str, **plan: contracts.Unpack[VoyagePlan]) -> None: ...
"#,
        ),
        (
            "keyword-only collision",
            r#"
from typing import TypedDict as Shape, Unpack as Spread
class Parcel(Shape):
    label: str
def dispatch(*, label: str, **parcel: Spread[Parcel]) -> None: ...
"#,
        ),
        (
            "reformatted collision",
            r#"
from typing import TypedDict as Shape, Unpack as Spread
class ObservatoryConfig(Shape):
    aperture: int
    mode: str
def observe(
    aperture: int,
    **configuration:
        Spread[
            ObservatoryConfig
        ],
) -> None: ...
"#,
        ),
    ];

    for (mutation, source) in rejected {
        let diagnostics = run(source)?;
        assert_eq!(
            diagnostics.len(),
            1,
            "{mutation}: one collision must produce one isolated diagnostic: {diagnostics:#?}"
        );
        assert_eq!(
            codes(&diagnostics),
            vec!["callables_kwargs"],
            "{mutation}: the PEP 692 rule itself must reject the collision"
        );
        assert_rule_count(
            &diagnostics,
            "callables_kwargs",
            1,
            "PEP 692 standard-parameter keyword collision",
        );
    }

    Ok(())
}
