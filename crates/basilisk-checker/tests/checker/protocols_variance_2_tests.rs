//! PEP 544 tests for declared versus inferred generic protocol variance.
//! <https://peps.python.org/pep-0544/#generic-protocols>

use super::common::*;

#[test]
fn pep_544_protocol_declared_covariance_matches_output_only_use(
) -> Result<(), Box<dyn std::error::Error>> {
    let accepted = [
        (
            "canonical imports",
            r#"
from typing import Protocol, TypeVar
T_co = TypeVar("T_co", covariant=True)
class Source(Protocol[T_co]):
    def read(self) -> T_co: ...
"#,
        ),
        (
            "aliased imports and renamed symbols",
            r#"
from typing import Protocol as Contract, TypeVar as Parameter
Harvest_co = Parameter("Harvest_co", covariant=True)
class Granary(Contract[Harvest_co]):
    def collect(self) -> Harvest_co: ...
"#,
        ),
        (
            "qualified imports",
            r#"
import typing as contracts
Payload_co = contracts.TypeVar("Payload_co", covariant=True)
class Receiver(contracts.Protocol[Payload_co]):
    def receive(self) -> Payload_co: ...
"#,
        ),
        (
            "reformatted protocol",
            r#"
from typing import Protocol as Contract, TypeVar as Parameter
Result_co = Parameter(
    "Result_co",
    covariant = True,
)
class Observatory(
    Contract[
        Result_co
    ]
):
    def measure(
        self,
    ) -> Result_co: ...
"#,
        ),
    ];

    for (mutation, source) in accepted {
        let diagnostics = run(source)?;
        assert!(
            diagnostics.is_empty(),
            "{mutation}: matching PEP 544 variance must be accepted: {diagnostics:#?}"
        );
        assert_eq!(
            codes(&diagnostics),
            Vec::<&str>::new(),
            "{mutation}: spelling must not alter inferred variance"
        );
        assert_rule_count(
            &diagnostics,
            "protocols_variance_2",
            0,
            "PEP 544 correctly declared covariant protocol",
        );
    }

    Ok(())
}

#[test]
fn pep_544_output_only_protocol_cannot_be_declared_invariant(
) -> Result<(), Box<dyn std::error::Error>> {
    let rejected = [
        (
            "canonical imports",
            r#"
from typing import Protocol, TypeVar
T = TypeVar("T")
class Source(Protocol[T]):
    def read(self) -> T: ...
"#,
        ),
        (
            "aliased imports and renamed symbols",
            r#"
from typing import Protocol as Contract, TypeVar as Parameter
Harvest = Parameter("Harvest")
class Granary(Contract[Harvest]):
    def collect(self) -> Harvest: ...
"#,
        ),
        (
            "qualified imports",
            r#"
import typing as contracts
Payload = contracts.TypeVar("Payload")
class Receiver(contracts.Protocol[Payload]):
    def receive(self) -> Payload: ...
"#,
        ),
        (
            "reformatted protocol",
            r#"
from typing import Protocol as Contract, TypeVar as Parameter
Result = Parameter(
    "Result",
)
class Observatory(
    Contract[
        Result
    ]
):
    def measure(
        self,
    ) -> Result: ...
"#,
        ),
    ];

    for (mutation, source) in rejected {
        let diagnostics = run(source)?;
        assert_eq!(
            diagnostics.len(),
            1,
            "{mutation}: one variance mismatch must produce one isolated diagnostic: {diagnostics:#?}"
        );
        assert_eq!(
            codes(&diagnostics),
            vec!["protocols_variance_2"],
            "{mutation}: the PEP 544 variance rule itself must reject the protocol"
        );
        assert_rule_count(
            &diagnostics,
            "protocols_variance_2",
            1,
            "PEP 544 inferred covariance declared invariant",
        );
    }

    Ok(())
}
