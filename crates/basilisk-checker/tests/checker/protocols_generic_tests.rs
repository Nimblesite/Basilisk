//! PEP 544 generic-protocol declaration and structural-assignment tests.
//!
//! Specifications:
//! - <https://typing.python.org/en/latest/spec/protocol.html#generic-protocols>
//! - <https://peps.python.org/pep-0544/#generic-protocols>
//! - <https://peps.python.org/pep-0544/#self-types-in-protocols>
//!
//! Each obligation uses aliased and qualified typing symbols plus formatting
//! mutations. No unrelated diagnostic is accepted as evidence.

use super::common::*;

const DECLARATION_RULE: &str = "protocols_generic";
const ASSIGNMENT_RULE: &str = "assignment_compatibility";

fn assert_protocol_declaration(
    source: &str,
    expected: usize,
    obligation: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(source)?;
    assert_rule_count(&diagnostics, DECLARATION_RULE, expected, obligation);
    assert_eq!(
        messages_for(&diagnostics, DECLARATION_RULE).len(),
        expected,
        "{obligation}: every declaration violation must have a rule-specific message: {diagnostics:#?}",
    );
    Ok(())
}

fn assert_protocol_assignment(
    source: &str,
    expected: usize,
    obligation: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(source)?;
    assert_rule_count(&diagnostics, ASSIGNMENT_RULE, expected, obligation);
    assert_eq!(
        messages_for(&diagnostics, ASSIGNMENT_RULE).len(),
        expected,
        "{obligation}: every structural assignment violation must have a rule-specific message: {diagnostics:#?}",
    );
    assert_rule_count(
        &diagnostics,
        DECLARATION_RULE,
        0,
        "a valid generic Protocol declaration must not be blamed for an assignment verdict",
    );
    Ok(())
}

#[test]
fn protocol_shorthand_cannot_be_combined_with_generic_base(
) -> Result<(), Box<dyn std::error::Error>> {
    let sources = [
        r#"
from typing import Generic as Family, Protocol as Contract, TypeVar as VariableForge

Yield = VariableForge("Yield", covariant=True)

class Producer(Contract[Yield], Family[Yield]):
    def produce(self) -> Yield: ...
"#,
        r#"
import typing as type_forms

Yield = type_forms.TypeVar(
    "Yield",
    covariant=True,
)

class Producer(
    type_forms.Protocol[Yield],
    type_forms.Generic[Yield],
):
    def produce(
        self,
    ) -> Yield: ...
"#,
    ];
    for source in sources {
        assert_protocol_declaration(
            source,
            1,
            "PEP 544 forbids combining Protocol[T] shorthand with Generic[T]",
        )?;
    }
    Ok(())
}

#[test]
fn protocol_shorthand_alone_is_a_valid_generic_protocol() -> Result<(), Box<dyn std::error::Error>>
{
    let sources = [
        r#"
from typing import Protocol as Contract, TypeVar as VariableForge

Yield = VariableForge("Yield", covariant=True)

class Producer(Contract[Yield]):
    def produce(self) -> Yield: ...
"#,
        r#"
import typing as type_forms

Yield = type_forms.TypeVar(
    "Yield",
    covariant=True,
)

class Producer(
    type_forms.Protocol[
        Yield
    ],
):
    def produce(self) -> Yield: ...
"#,
    ];
    for source in sources {
        assert_protocol_declaration(
            source,
            0,
            "PEP 544 defines Protocol[T] as a valid generic-protocol shorthand",
        )?;
    }
    Ok(())
}

#[test]
fn specialized_protocol_rejects_incompatible_method_signature(
) -> Result<(), Box<dyn std::error::Error>> {
    let sources = [
        r#"
from typing import Protocol as Contract, TypeVar as VariableForge

Input = VariableForge("Input")

class Processor(Contract[Input]):
    def process(self, item: Input) -> Input: ...

class WholeProcessor:
    def process(self, item: int) -> int:
        return item

processor: Processor[str] = WholeProcessor()
"#,
        r#"
import typing as type_forms
from builtins import int as Whole
from builtins import str as Text

Input = type_forms.TypeVar( "Input" )

class Processor(type_forms.Protocol[Input]):
    def process(
        self,
        item: Input,
    ) -> Input: ...

class WholeProcessor:
    def process(self, item: Whole) -> Whole:
        return item

processor: Processor[
    Text
] = WholeProcessor()
"#,
    ];
    for source in sources {
        assert_protocol_assignment(
            source,
            1,
            "PEP 544 rejects a structural implementation whose specialized method signature is incompatible",
        )?;
    }
    Ok(())
}

#[test]
fn specialized_protocol_accepts_compatible_method_signature(
) -> Result<(), Box<dyn std::error::Error>> {
    let sources = [
        r#"
from typing import Protocol as Contract, TypeVar as VariableForge

Input = VariableForge("Input")

class Processor(Contract[Input]):
    def process(self, item: Input) -> Input: ...

class WholeProcessor:
    def process(self, item: int) -> int:
        return item

processor: Processor[int] = WholeProcessor()
"#,
        r#"
import typing as type_forms
from builtins import int as Whole

Input = type_forms.TypeVar( "Input" )

class Processor(
    type_forms.Protocol[Input],
):
    def process(self, item: Input) -> Input: ...

class WholeProcessor:
    def process(
        self,
        item: Whole,
    ) -> Whole:
        return item

processor: Processor[ Whole ] = WholeProcessor()
"#,
    ];
    for source in sources {
        assert_protocol_assignment(
            source,
            0,
            "PEP 544 accepts a structural implementation with the specialized protocol signature",
        )?;
    }
    Ok(())
}

#[test]
fn protocol_with_two_type_arguments_is_checked_after_specialization(
) -> Result<(), Box<dyn std::error::Error>> {
    let sources = [
        r#"
from typing import Protocol as Contract, TypeVar as VariableForge

Input = VariableForge("Input")
Output = VariableForge("Output")

class Converter(Contract[Input, Output]):
    def convert(self, item: Input) -> Output: ...

class RenderWhole:
    def convert(self, item: int) -> str:
        return str(item)

converter: Converter[int, str] = RenderWhole()
"#,
        r#"
import typing as type_forms
from builtins import int as Whole
from builtins import str as Text

Input = type_forms.TypeVar("Input")
Output = type_forms.TypeVar( "Output" )

class Converter(
    type_forms.Protocol[Input, Output],
):
    def convert(self, item: Input) -> Output: ...

class RenderWhole:
    def convert(self, item: Whole) -> Text:
        return Text(item)

converter: Converter[
    Whole,
    Text,
] = RenderWhole()
"#,
    ];
    for source in sources {
        assert_protocol_assignment(
            source,
            0,
            "PEP 544 applies each generic protocol argument to its corresponding structural member type",
        )?;
    }
    Ok(())
}

#[test]
fn covariant_protocol_accepts_matching_producer() -> Result<(), Box<dyn std::error::Error>> {
    let sources = [
        r#"
from typing import Protocol as Contract, TypeVar as VariableForge

Yield = VariableForge("Yield", covariant=True)

class Reader(Contract[Yield]):
    def read(self) -> Yield: ...

class WholeReader:
    def read(self) -> int:
        return 42

reader: Reader[int] = WholeReader()
"#,
        r#"
import typing as type_forms
from builtins import int as Whole

Yield = type_forms.TypeVar(
    "Yield",
    covariant=True,
)

class Reader(type_forms.Protocol[Yield]):
    def read(
        self,
    ) -> Yield: ...

class WholeReader:
    def read(self) -> Whole:
        return Whole(42)

reader: Reader[ Whole ] = WholeReader()
"#,
    ];
    for source in sources {
        assert_protocol_assignment(
            source,
            0,
            "PEP 544 permits a producer protocol to use a covariant result TypeVar",
        )?;
    }
    Ok(())
}

#[test]
fn protocol_self_type_accepts_implementers_own_return_type(
) -> Result<(), Box<dyn std::error::Error>> {
    let sources = [
        r#"
from typing import Protocol as Contract, Self as CurrentKind

class Copyable(Contract):
    def copy(self) -> CurrentKind: ...

class Artifact:
    def copy(self) -> "Artifact":
        return Artifact()

copyable: Copyable = Artifact()
"#,
        r#"
import typing as type_forms

class Copyable(
    type_forms.Protocol,
):
    def copy(self) -> type_forms.Self: ...

class Artifact:
    def copy(
        self,
    ) -> "Artifact":
        return Artifact()

copyable: Copyable = Artifact()
"#,
    ];
    for source in sources {
        assert_protocol_assignment(
            source,
            0,
            "PEP 544 self-type protocols accept an implementation returning its own concrete type",
        )?;
    }
    Ok(())
}

#[test]
fn self_typed_protocol_rejects_missing_required_method() -> Result<(), Box<dyn std::error::Error>> {
    let sources = [
        r#"
from typing import Protocol as Contract, Self as CurrentKind

class Builder(Contract):
    def build(self) -> CurrentKind: ...
    def reset(self) -> None: ...

class IncompleteBuilder:
    def build(self) -> "IncompleteBuilder":
        return IncompleteBuilder()

builder: Builder = IncompleteBuilder()
"#,
        r#"
import typing as type_forms

class Builder(type_forms.Protocol):
    def build(self) -> type_forms.Self: ...
    def reset(
        self,
    ) -> None: ...

class IncompleteBuilder:
    def build(self) -> "IncompleteBuilder":
        return IncompleteBuilder()

builder: Builder = IncompleteBuilder()
"#,
    ];
    for source in sources {
        assert_protocol_assignment(
            source,
            1,
            "PEP 544 rejects a structural implementation that omits a required protocol method",
        )?;
    }
    Ok(())
}
