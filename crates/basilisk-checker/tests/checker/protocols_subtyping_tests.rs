//! Tests for [`protocols_subtyping`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for protocols_subtyping: Protocol tuple element type mismatch.

use super::common::*;

#[test]
fn valid_tuple_protocol_assignment() -> Result<(), Box<dyn std::error::Error>> {
    // PEP 544 requires an explicit subclass to implement protocol attributes
    // with compatible types. This is the accepted side of the PEP's `RGB`
    // example, mutated so no particular spelling can carry the verdict:
    // https://peps.python.org/pep-0544/#explicitly-declaring-implementation
    let mutations = [
        r"from typing import Protocol
class RGB(Protocol):
    rgb: tuple[int, int, int]
class Point(RGB):
    def __init__(self, red: int, green: int, blue: int) -> None:
        self.rgb = red, green, blue
",
        r"from typing import Protocol as StructuralContract
class Channels(StructuralContract):
    values: tuple[int, int, int]
class Sample(Channels):
    def __init__(self, first: int, second: int, third: int) -> None:
        self.values = first, second, third
",
        r"import typing as type_support
class Coordinates(type_support.Protocol):
    position: tuple[int, int, int]
class Location(Coordinates):
    def __init__(self, east: int, north: int, altitude: int) -> None:
        self.position = east, north, altitude
",
        r"import typing
from builtins import int as WholeNumber
from builtins import tuple as FixedProduct
class FormattedContract(
    typing.Protocol,
):
    data: FixedProduct[
        WholeNumber,
        WholeNumber,
        WholeNumber,
    ]
class FormattedImplementation(FormattedContract):
    def __init__(
        self,
        first: WholeNumber,
        second: WholeNumber,
        third: WholeNumber,
    ) -> None:
        self.data = (
            first,
            second,
            third,
        )
",
    ];

    for source in mutations {
        let diagnostics = run(source)?;
        assert_rule_count(
            &diagnostics,
            "protocols_subtyping",
            0,
            "PEP 544 accepts compatible explicit protocol attribute implementations",
        );
        assert_rule_count(
            &diagnostics,
            "assignment_compatibility",
            0,
            "compatible tuple elements are also assignment-compatible",
        );
    }
    Ok(())
}

#[test]
fn mismatched_tuple_protocol_assignment() -> Result<(), Box<dyn std::error::Error>> {
    // The rejected side of the same PEP 544 obligation changes the final tuple
    // element to `str`. Alias, qualification, identifier, and whitespace
    // mutations must all produce the same protocol-subtyping verdict.
    let mutations = [
        r"from typing import Protocol
class RGB(Protocol):
    rgb: tuple[int, int, int]
class Point(RGB):
    def __init__(self, red: int, green: int, blue: str) -> None:
        self.rgb = red, green, blue
",
        r"from typing import Protocol as StructuralContract
class Channels(StructuralContract):
    values: tuple[int, int, int]
class BadSample(Channels):
    def __init__(self, first: int, second: int, third: str) -> None:
        self.values = first, second, third
",
        r"import typing as type_support
class Coordinates(type_support.Protocol):
    position: tuple[int, int, int]
class BadLocation(Coordinates):
    def __init__(self, east: int, north: int, altitude: str) -> None:
        self.position = east, north, altitude
",
        r"import typing
from builtins import int as WholeNumber
from builtins import str as TextValue
from builtins import tuple as FixedProduct
class FormattedContract(
    typing.Protocol,
):
    data: FixedProduct[
        WholeNumber,
        WholeNumber,
        WholeNumber,
    ]
class BadFormattedImplementation(FormattedContract):
    def __init__(
        self,
        first: WholeNumber,
        second: WholeNumber,
        third: TextValue,
    ) -> None:
        self.data = (
            first,
            second,
            third,
        )
",
    ];

    for source in mutations {
        let diagnostics = run(source)?;
        assert_rule_count(
            &diagnostics,
            "protocols_subtyping",
            1,
            "PEP 544 rejects an explicit protocol attribute implementation with an incompatible tuple element",
        );
    }
    Ok(())
}
