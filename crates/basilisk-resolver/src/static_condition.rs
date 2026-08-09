//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Static evaluation of `if`-guards at a configured target version.
//!
//! Type checkers statically evaluate guards such as
//! `if sys.version_info >= (3, 12):` or `if typing.TYPE_CHECKING:` to decide
//! which class members exist for the target Python version — the typing
//! spec's version-and-platform-checks and `TYPE_CHECKING` directives
//! (<https://typing.python.org/en/latest/spec/directives.html>). A field
//! guarded by an always-false branch is absent; a field guarded by an
//! always-true (or unevaluable) branch is present.
//!
//! Both special names are recognised by resolving the guard expression
//! through the module's [`BindingTable`], never from its spelling:
//! `import sys as s; s.version_info` and `from typing import TYPE_CHECKING
//! as TC` behave exactly like the plain spellings, and a module that binds
//! `sys` or `TYPE_CHECKING` to something else is never misread. Implements
//! [RESOLV-CANONICAL-BINDING].

use ruff_python_ast::{BoolOp, CmpOp, Expr, ExprBoolOp, ExprCompare, Number, UnaryOp};

use crate::canonical::{BindingTable, TypingForm};

/// Truth of a [`StaticCondition`] once the target version is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchTruth {
    /// The branch is taken for every supported target.
    AlwaysTrue,
    /// The branch is never taken.
    AlwaysFalse,
    /// The condition cannot be evaluated statically.
    Unknown,
}

/// A target-independent, statically-evaluable form of an `if` test. Recorded by
/// the resolver (which is version-blind) and evaluated later against a concrete
/// target via [`evaluate`].
#[derive(Debug, Clone, PartialEq)]
pub enum StaticCondition {
    /// A comparison of the target version against a `(major, minor)` guard.
    Version {
        /// The comparison operator, applied as `target <op> guard`.
        op: CmpOp,
        /// The `(major, minor)` version the target is compared against.
        guard: (u32, u32),
        /// The guard tuple's third element, when it has one
        /// (`sys.version_info >= (3, 11, 7)`).
        ///
        /// The target names every micro of a `(major, minor)` release, so
        /// when the target equals `guard` a micro-versioned comparison holds
        /// for some micros and not others and is not statically decidable.
        micro: Option<u32>,
    },
    /// A type-checking-only guard — always true under a checker.
    TypeChecking,
    /// A boolean literal (`True` / `False`).
    Bool(bool),
    /// `not <cond>`.
    Not(Box<StaticCondition>),
    /// `a and b and …`.
    All(Vec<StaticCondition>),
    /// `a or b or …`.
    Any(Vec<StaticCondition>),
    /// Anything not statically evaluable (e.g. a runtime flag).
    Unknown,
}

/// Parse an `if` test into a [`StaticCondition`], resolving names through the
/// module's `bindings`. Never fails — anything it does not understand becomes
/// [`StaticCondition::Unknown`].
#[must_use]
pub fn parse_static_condition(bindings: &BindingTable, test: &Expr) -> StaticCondition {
    match test {
        Expr::BooleanLiteral(lit) => StaticCondition::Bool(lit.value),
        Expr::UnaryOp(unary) if matches!(unary.op, UnaryOp::Not) => {
            StaticCondition::Not(Box::new(parse_static_condition(bindings, &unary.operand)))
        }
        Expr::BoolOp(bool_op) => boolean_composition(bindings, bool_op),
        Expr::Compare(compare) => version_comparison(bindings, compare),
        Expr::Name(_) | Expr::Attribute(_)
            if bindings.is_form(test, TypingForm::TypeCheckingFlag) =>
        {
            StaticCondition::TypeChecking
        }
        _ => StaticCondition::Unknown,
    }
}

/// `a and b and …` / `a or b or …`.
fn boolean_composition(bindings: &BindingTable, bool_op: &ExprBoolOp) -> StaticCondition {
    let parts = bool_op
        .values
        .iter()
        .map(|value| parse_static_condition(bindings, value))
        .collect();
    match bool_op.op {
        BoolOp::And => StaticCondition::All(parts),
        BoolOp::Or => StaticCondition::Any(parts),
    }
}

/// A single-operator comparison with `sys.version_info` on either side and a
/// literal version tuple on the other. Chained comparisons and any other
/// subject are not statically evaluable.
fn version_comparison(bindings: &BindingTable, compare: &ExprCompare) -> StaticCondition {
    let ([op], [comparator]) = (compare.ops.as_ref(), compare.comparators.as_ref()) else {
        return StaticCondition::Unknown;
    };
    if resolves_to_version_info(bindings, &compare.left) {
        return version_guard(*op, comparator);
    }
    if resolves_to_version_info(bindings, comparator) {
        return version_guard(flip_comparison(*op), &compare.left);
    }
    StaticCondition::Unknown
}

/// Whether an expression resolves to `sys.version_info` — plain, through an
/// aliased `import sys as s`, or as `from sys import version_info`.
fn resolves_to_version_info(bindings: &BindingTable, expr: &Expr) -> bool {
    matches!(expr, Expr::Name(_) | Expr::Attribute(_))
        && bindings.resolves_to(expr, "sys", "version_info")
}

/// The condition for `sys.version_info <op> guard_expr`.
fn version_guard(op: CmpOp, guard_expr: &Expr) -> StaticCondition {
    version_tuple(guard_expr).map_or(StaticCondition::Unknown, |(guard, micro)| {
        StaticCondition::Version { op, guard, micro }
    })
}

/// Mirror a comparison so the version subject reads on the left:
/// `(3, 12) <= sys.version_info` is `sys.version_info >= (3, 12)`.
const fn flip_comparison(op: CmpOp) -> CmpOp {
    match op {
        CmpOp::Lt => CmpOp::Gt,
        CmpOp::LtE => CmpOp::GtE,
        CmpOp::Gt => CmpOp::Lt,
        CmpOp::GtE => CmpOp::LtE,
        other => other,
    }
}

/// A literal `(major, minor)` or `(major, minor, micro)` integer tuple.
fn version_tuple(expr: &Expr) -> Option<((u32, u32), Option<u32>)> {
    let Expr::Tuple(tuple) = expr else {
        return None;
    };
    let (major, minor, micro) = match tuple.elts.as_slice() {
        [major, minor] => (major, minor, None),
        [major, minor, micro] => (major, minor, Some(integer_literal(micro)?)),
        _ => return None,
    };
    Some(((integer_literal(major)?, integer_literal(minor)?), micro))
}

/// An integer literal small enough to be a version component.
fn integer_literal(expr: &Expr) -> Option<u32> {
    let Expr::NumberLiteral(number) = expr else {
        return None;
    };
    match &number.value {
        Number::Int(value) => value.as_u64().and_then(|value| u32::try_from(value).ok()),
        _ => None,
    }
}

/// Evaluate a [`StaticCondition`] at the given `target_version`.
#[must_use]
pub fn evaluate(cond: &StaticCondition, target_version: (u32, u32)) -> BranchTruth {
    match cond {
        StaticCondition::Version { op, guard, micro } => {
            match version_holds(*op, target_version, *guard, micro.is_some()) {
                Some(true) => BranchTruth::AlwaysTrue,
                Some(false) => BranchTruth::AlwaysFalse,
                None => BranchTruth::Unknown,
            }
        }
        StaticCondition::TypeChecking => BranchTruth::AlwaysTrue,
        StaticCondition::Bool(value) => from_bool(*value),
        StaticCondition::Not(inner) => negate(evaluate(inner, target_version)),
        StaticCondition::All(parts) => evaluate_all(parts, target_version),
        StaticCondition::Any(parts) => evaluate_any(parts, target_version),
        StaticCondition::Unknown => BranchTruth::Unknown,
    }
}

fn from_bool(value: bool) -> BranchTruth {
    if value {
        BranchTruth::AlwaysTrue
    } else {
        BranchTruth::AlwaysFalse
    }
}

fn negate(truth: BranchTruth) -> BranchTruth {
    match truth {
        BranchTruth::AlwaysTrue => BranchTruth::AlwaysFalse,
        BranchTruth::AlwaysFalse => BranchTruth::AlwaysTrue,
        BranchTruth::Unknown => BranchTruth::Unknown,
    }
}

fn evaluate_all(parts: &[StaticCondition], target: (u32, u32)) -> BranchTruth {
    let mut all_true = true;
    for part in parts {
        match evaluate(part, target) {
            BranchTruth::AlwaysFalse => return BranchTruth::AlwaysFalse,
            BranchTruth::Unknown => all_true = false,
            BranchTruth::AlwaysTrue => {}
        }
    }
    if all_true {
        BranchTruth::AlwaysTrue
    } else {
        BranchTruth::Unknown
    }
}

fn evaluate_any(parts: &[StaticCondition], target: (u32, u32)) -> BranchTruth {
    let mut all_false = true;
    for part in parts {
        match evaluate(part, target) {
            BranchTruth::AlwaysTrue => return BranchTruth::AlwaysTrue,
            BranchTruth::Unknown => all_false = false,
            BranchTruth::AlwaysFalse => {}
        }
    }
    if all_false {
        BranchTruth::AlwaysFalse
    } else {
        BranchTruth::Unknown
    }
}

/// Whether `target <op> guard` holds; `None` when the operator does not apply
/// to version tuples (`is`, `in`, …) or the outcome depends on the micro
/// version the target does not model.
fn version_holds(
    op: CmpOp,
    target: (u32, u32),
    guard: (u32, u32),
    guard_has_micro: bool,
) -> Option<bool> {
    if guard_has_micro && target == guard {
        // The target names every micro of its release, so a guard like
        // `>= (3, 11, 7)` at target 3.11 holds for some micros and not
        // others. When the (major, minor) prefixes differ, the prefix alone
        // decides every operator regardless of the micro.
        return None;
    }
    Some(match op {
        CmpOp::Lt => target < guard,
        CmpOp::LtE => target <= guard,
        CmpOp::Gt => target > guard,
        CmpOp::GtE => target >= guard,
        CmpOp::Eq => target == guard,
        CmpOp::NotEq => target != guard,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use ruff_python_ast::{CmpOp, Stmt};

    use crate::canonical::BindingTable;

    use super::{evaluate, parse_static_condition, BranchTruth, StaticCondition};

    fn parse_if_test(source: &str) -> Result<StaticCondition, String> {
        let parsed = basilisk_parser::parse_source(
            source.to_string(),
            "<static-condition-test>".to_string(),
        )
        .map_err(|err| err.to_string())?;
        let bindings = BindingTable::from_module(&parsed.ast.body);
        let if_stmt = parsed
            .ast
            .body
            .iter()
            .find_map(|stmt| match stmt {
                Stmt::If(if_stmt) => Some(if_stmt),
                _ => None,
            })
            .ok_or_else(|| "test fixture should contain an if statement".to_string())?;
        Ok(parse_static_condition(&bindings, &if_stmt.test))
    }

    #[test]
    fn parses_type_checking_and_boolean_composition() -> Result<(), String> {
        let cond = parse_if_test(
            r"
import typing

if typing.TYPE_CHECKING and (not False or feature_flag):
    x = 1
",
        )?;

        assert_eq!(
            cond,
            StaticCondition::All(vec![
                StaticCondition::TypeChecking,
                StaticCondition::Any(vec![
                    StaticCondition::Not(Box::new(StaticCondition::Bool(false))),
                    StaticCondition::Unknown,
                ]),
            ]),
        );
        assert_eq!(evaluate(&cond, (3, 12)), BranchTruth::AlwaysTrue);
        Ok(())
    }

    #[test]
    fn resolves_type_checking_through_module_bindings() -> Result<(), String> {
        let recognised = [
            "import typing\nif typing.TYPE_CHECKING:\n    pass\n",
            "import typing_extensions\nif typing_extensions.TYPE_CHECKING:\n    pass\n",
            "import typing as t\nif t.TYPE_CHECKING:\n    pass\n",
            "from typing import TYPE_CHECKING\nif TYPE_CHECKING:\n    pass\n",
            "from typing_extensions import TYPE_CHECKING as TC\nif TC:\n    pass\n",
        ];
        for source in recognised {
            assert_eq!(
                parse_if_test(source)?,
                StaticCondition::TypeChecking,
                "fixture:\n{source}"
            );
        }

        // The name is resolved to what the module binds it to, never taken
        // from its spelling: a foreign module's TYPE_CHECKING attribute, a
        // rebound name, and an unimported qualifier are all runtime flags.
        let unresolved = [
            "import settings\nif settings.TYPE_CHECKING:\n    pass\n",
            "import settings as typing\nif typing.TYPE_CHECKING:\n    pass\n",
            "TYPE_CHECKING = True\nif TYPE_CHECKING:\n    pass\n",
            "if typing.TYPE_CHECKING:\n    pass\n",
        ];
        for source in unresolved {
            assert_eq!(
                parse_if_test(source)?,
                StaticCondition::Unknown,
                "fixture:\n{source}"
            );
        }
        Ok(())
    }

    #[test]
    fn evaluates_boolean_all_unknown_and_false_cases() {
        let with_unknown =
            StaticCondition::All(vec![StaticCondition::Bool(true), StaticCondition::Unknown]);
        let with_false = StaticCondition::All(vec![
            StaticCondition::Bool(true),
            StaticCondition::Bool(false),
        ]);

        assert_eq!(evaluate(&with_unknown, (3, 12)), BranchTruth::Unknown);
        assert_eq!(evaluate(&with_false, (3, 12)), BranchTruth::AlwaysFalse);
    }

    #[test]
    fn evaluates_boolean_any_unknown_and_false_cases() {
        let with_unknown =
            StaticCondition::Any(vec![StaticCondition::Bool(false), StaticCondition::Unknown]);
        let all_false = StaticCondition::Any(vec![
            StaticCondition::Bool(false),
            StaticCondition::Bool(false),
        ]);

        assert_eq!(evaluate(&with_unknown, (3, 12)), BranchTruth::Unknown);
        assert_eq!(evaluate(&all_false, (3, 12)), BranchTruth::AlwaysFalse);
    }

    #[test]
    fn parses_reversed_version_comparison_and_micro_tuple() -> Result<(), String> {
        let cond = parse_if_test(
            r"
import sys

if (3, 11, 7) <= sys.version_info:
    x = 1
",
        )?;

        assert_eq!(
            cond,
            StaticCondition::Version {
                op: CmpOp::GtE,
                guard: (3, 11),
                micro: Some(7),
            },
        );
        assert_eq!(evaluate(&cond, (3, 12)), BranchTruth::AlwaysTrue);
        assert_eq!(evaluate(&cond, (3, 10)), BranchTruth::AlwaysFalse);
        // Target 3.11 spans micros on both sides of 3.11.7, so neither branch
        // is statically decided.
        assert_eq!(evaluate(&cond, (3, 11)), BranchTruth::Unknown);
        Ok(())
    }

    #[test]
    fn resolves_version_info_through_module_bindings() -> Result<(), String> {
        let expected = StaticCondition::Version {
            op: CmpOp::GtE,
            guard: (3, 12),
            micro: None,
        };
        assert_eq!(
            parse_if_test("import sys as system\nif system.version_info >= (3, 12):\n    x = 1\n")?,
            expected,
        );
        assert_eq!(
            parse_if_test(
                "from sys import version_info\nif version_info >= (3, 12):\n    x = 1\n"
            )?,
            expected,
        );

        // A rebound or unimported `sys` is not the interpreter's `sys`.
        for source in [
            "import fake as sys\nif sys.version_info >= (3, 12):\n    x = 1\n",
            "if sys.version_info >= (3, 12):\n    x = 1\n",
        ] {
            assert_eq!(
                parse_if_test(source)?,
                StaticCondition::Unknown,
                "fixture:\n{source}"
            );
        }
        Ok(())
    }

    #[test]
    fn evaluates_each_supported_version_operator() {
        let target = (3, 12);

        for (op, expected) in [
            (CmpOp::Lt, BranchTruth::AlwaysFalse),
            (CmpOp::LtE, BranchTruth::AlwaysTrue),
            (CmpOp::Gt, BranchTruth::AlwaysFalse),
            (CmpOp::Eq, BranchTruth::AlwaysTrue),
            (CmpOp::NotEq, BranchTruth::AlwaysFalse),
        ] {
            let cond = StaticCondition::Version {
                op,
                guard: (3, 12),
                micro: None,
            };
            assert_eq!(evaluate(&cond, target), expected);
        }
    }

    #[test]
    fn micro_guard_is_undecidable_at_its_own_minor() {
        for op in [
            CmpOp::Lt,
            CmpOp::LtE,
            CmpOp::Gt,
            CmpOp::GtE,
            CmpOp::Eq,
            CmpOp::NotEq,
        ] {
            let cond = StaticCondition::Version {
                op,
                guard: (3, 11),
                micro: Some(7),
            };
            assert_eq!(evaluate(&cond, (3, 11)), BranchTruth::Unknown, "op: {op:?}");
        }
    }

    #[test]
    fn unsupported_version_operator_is_unknown() {
        let cond = StaticCondition::Version {
            op: CmpOp::In,
            guard: (3, 12),
            micro: None,
        };
        assert_eq!(evaluate(&cond, (3, 12)), BranchTruth::Unknown);
    }

    #[test]
    fn unsupported_version_shapes_parse_as_unknown() -> Result<(), String> {
        for source in [
            "import sys\nif sys.version_info >= (3,):\n    x = 1\n",
            "import sys\nif sys.version_info >= version:\n    x = 1\n",
            "import platform\nif (3, 12) < platform.version_info:\n    x = 1\n",
            "import sys\nif sys.version_info < (3.12, 0):\n    x = 1\n",
            "import sys\nif sys.version_info < (999999999999999999999999, 0):\n    x = 1\n",
            "import sys\nif sys.version_info < (3, 11, 7, 0):\n    x = 1\n",
            "import sys\nif (3, 10) < sys.version_info < (3, 12):\n    x = 1\n",
        ] {
            assert_eq!(
                parse_if_test(source)?,
                StaticCondition::Unknown,
                "fixture:\n{source}"
            );
        }
        Ok(())
    }
}
