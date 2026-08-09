//! Tests for [`overloads_consistency_2`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
// Integration tests for overloads_consistency_2: inconsistent decorators across an overload group.

use super::common::*;

const RULE: &str = "overloads_consistency_2";

/// Python's typing documentation requires `@staticmethod` to be applied
/// consistently across every overload and its implementation. That obligation
/// attaches to the resolved builtin decorator, not to its source spelling:
/// <https://docs.python.org/3/library/typing.html#typing.overload>.
#[test]
fn aliased_staticmethod_is_still_the_builtin_decorator() -> Result<(), Box<dyn std::error::Error>> {
    let sources = [
        "from typing import overload as signatures\nfrom builtins import staticmethod as method_form\nclass Kiln:\n    @signatures\n    @method_form\n    def fire(level: int) -> int: ...\n    @signatures\n    @method_form\n    def fire(level: str) -> str: ...\n    def fire(level): return level\n",
        "from typing import overload as signatures\nfrom builtins import staticmethod as method_form\n\nclass Kiln:\n\n    @signatures\n    @method_form\n    def fire( level: int ) -> int: ...\n\n    @signatures\n    @method_form\n    def fire( level: str ) -> str: ...\n\n    def fire( level ):\n        return level\n",
    ];

    for source in sources {
        let diagnostics = run(source)?;
        assert_rule_count(
            &diagnostics,
            RULE,
            1,
            "an aliased builtin staticmethod must retain its resolved decorator identity",
        );
        assert!(
            messages_for(&diagnostics, RULE)
                .iter()
                .all(|message| message.contains("staticmethod")),
            "the rule-specific diagnostic must identify the inconsistent decorator: {diagnostics:#?}",
        );
    }
    Ok(())
}

/// A user attribute whose final token happens to be `staticmethod` is not the
/// builtin decorator. PEP 484 semantics therefore do not permit a diagnostic
/// based only on that coincidental spelling.
#[test]
fn unrelated_staticmethod_attribute_does_not_acquire_builtin_semantics(
) -> Result<(), Box<dyn std::error::Error>> {
    let sources = [
        "from typing import overload as signatures\nclass Decorations:\n    @staticmethod\n    def staticmethod(function): return function\ndecorations = Decorations()\nclass Kiln:\n    @signatures\n    @decorations.staticmethod\n    def fire(level: int) -> int: ...\n    @signatures\n    @decorations.staticmethod\n    def fire(level: str) -> str: ...\n    def fire(level): return level\n",
        "from typing import overload as signatures\n\nclass Decorations:\n    @staticmethod\n    def staticmethod( function ):\n        return function\n\ndecorations = Decorations()\n\nclass Kiln:\n    @signatures\n    @decorations.staticmethod\n    def fire( level: int ) -> int: ...\n\n    @signatures\n    @decorations.staticmethod\n    def fire( level: str ) -> str: ...\n\n    def fire( level ):\n        return level\n",
    ];

    for source in sources {
        let diagnostics = run(source)?;
        assert_rule_count(
            &diagnostics,
            RULE,
            0,
            "an unrelated decorator attribute must not be classified by its trailing spelling",
        );
        assert!(
            messages_for(&diagnostics, RULE).is_empty(),
            "no overload decorator-consistency message may be produced for a user decorator: {diagnostics:#?}",
        );
    }
    Ok(())
}

#[test]
fn static_inconsistent_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import overload\nclass C:\n    @overload\n    @staticmethod\n    def f(x: int) -> int: ...\n    @overload\n    @staticmethod\n    def f(x: str) -> str: ...\n    def f(x): return x\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"overloads_consistency_2"),
        "impl missing @staticmethod that the overloads have must fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn consistent_static_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import overload\nclass C:\n    @overload\n    @staticmethod\n    def f(x: int) -> int: ...\n    @overload\n    @staticmethod\n    def f(x: str) -> str: ...\n    @staticmethod\n    def f(x): return x\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"overloads_consistency_2"),
        "uniform @staticmethod across overloads + impl must not fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn final_on_overload_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import overload, final\nclass C:\n    @overload\n    @final\n    def f(self, x: int) -> int: ...\n    @overload\n    def f(self, x: str) -> str: ...\n    def f(self, x): return x\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"overloads_consistency_2"),
        "@final on an overload signature must fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn final_on_impl_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import overload, final\nclass C:\n    @overload\n    def f(self, x: int) -> int: ...\n    @overload\n    def f(self, x: str) -> str: ...\n    @final\n    def f(self, x): return x\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"overloads_consistency_2"),
        "@final on the implementation only is correct; must not fire"
    );
    Ok(())
}

#[test]
fn override_on_overload_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import overload, override\nclass B:\n    def f(self, x): ...\nclass C(B):\n    @overload\n    @override\n    def f(self, x: int) -> int: ...\n    @overload\n    def f(self, x: str) -> str: ...\n    def f(self, x): return x\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"overloads_consistency_2"),
        "@override on an overload signature must fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
