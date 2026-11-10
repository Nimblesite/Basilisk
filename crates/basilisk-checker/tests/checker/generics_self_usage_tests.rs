//! Tests for [`generics_self_usage`] from [CHKARCH-DIAG-QUALITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-QUALITY
// Integration tests for generics_self_usage: Self type in invalid location.

use super::common::*;

#[test]
fn self_in_method_ok() -> Result<(), Box<dyn std::error::Error>> {
    // PEP 673 defines Self in an instance method as a type variable bound to
    // the enclosing class:
    // https://peps.python.org/pep-0673/#use-in-method-signatures
    let mutations = [
        r"from typing import Self
class Copyable:
    def clone(self) -> Self:
        return self
",
        r"from typing import Self as CurrentInstance
class Duplicable:
    def duplicate(self) -> CurrentInstance:
        return self
",
        r"import typing as type_support
class Rebuildable:
    def rebuild(self) -> type_support.Self:
        return self
",
        r"import typing
class Formatted:
    def reproduce(
        self,
    ) -> typing.Self:
        return (
            self
        )
",
    ];

    for source in mutations {
        let diagnostics = run(source)?;
        assert_rule_count(
            &diagnostics,
            "generics_self_usage",
            0,
            "PEP 673 permits Self in an instance-method return annotation",
        );
        assert_rule_count(
            &diagnostics,
            "generics_self_basic",
            0,
            "returning self satisfies the Self return contract",
        );
    }
    Ok(())
}

#[test]
fn self_outside_class() -> Result<(), Box<dyn std::error::Error>> {
    // PEP 673 says Self is valid only in a class context and always refers to
    // the encapsulating class:
    // https://peps.python.org/pep-0673/#valid-locations-for-self
    let mutations = [
        r"from typing import Self
def standalone() -> Self: ...
",
        r"from typing import Self as CurrentInstance
def detached() -> CurrentInstance: ...
",
        r"import typing as type_support
def unbound() -> type_support.Self: ...
",
        r"import typing
def formatted(
) -> typing.Self:
    ...
",
    ];

    for source in mutations {
        let diagnostics = run(source)?;
        assert_rule_count(
            &diagnostics,
            "generics_self_usage",
            1,
            "PEP 673 rejects Self outside an encapsulating class",
        );
    }
    Ok(())
}
