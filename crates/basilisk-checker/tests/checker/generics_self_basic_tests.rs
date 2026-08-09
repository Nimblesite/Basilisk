//! Tests for [`generics_self_basic`] from [CHKARCH-DIAG-OPTIONAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OPTIONAL
// Integration tests for generics_self_basic: Self type violation.

use super::common::*;

#[test]
fn self_type_violation_exercise() -> Result<(), Box<dyn std::error::Error>> {
    // PEP 673 rejects returning a concrete base instance from `-> Self`:
    // callers on a subclass must receive the subclass, not its parent.
    // https://peps.python.org/pep-0673/#valid-locations-for-self
    let mutations = [
        r"from typing import Self
class Base:
    def copy(self) -> Self:
        return Base()
class Child(Base): pass
result = Child().copy()
",
        r"from typing import Self as CurrentInstance
class Parent:
    def duplicate(self) -> CurrentInstance:
        return Parent()
class Descendant(Parent): pass
result = Descendant().duplicate()
",
        r"import typing as type_support
class Root:
    def clone(self) -> type_support.Self:
        return Root()
class Leaf(Root): pass
result = Leaf().clone()
",
        r"import typing
class FormattedBase:
    def rebuild(
        self,
    ) -> typing.Self:
        return FormattedBase(
        )
class FormattedChild(
    FormattedBase,
):
    pass
result = FormattedChild().rebuild()
",
    ];

    for source in mutations {
        let diagnostics = run(source)?;
        assert_rule_count(
            &diagnostics,
            "generics_self_basic",
            1,
            "PEP 673 rejects a concrete base return from a method annotated with Self",
        );
    }
    Ok(())
}
