//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Static evaluation of `if`-guards at a configured target version.
//!
//! Type checkers statically evaluate guards such as
//! `if sys.version_info >= (3, 12):` or `if typing.TYPE_CHECKING:` to decide
//! which class members exist for the target Python version (the typing spec's
//! "version and platform checks"). A field guarded by an always-false branch is
//! absent; a field guarded by an always-true (or unevaluable) branch is present.

use ruff_python_ast::{BoolOp, CmpOp, Expr, Number, UnaryOp};

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
    /// `sys.version_info <op> (major, minor)`.
    Version {
        /// The comparison operator, normalised so `sys.version_info` is the left operand.
        op: CmpOp,
        /// The `(major, minor)` version the target is compared against.
        guard: (u32, u32),
    },
    /// `TYPE_CHECKING` / `typing.TYPE_CHECKING` — always true under a checker.
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

/// Parse an `if` test into a [`StaticCondition`]. Never fails — anything it does
/// not understand becomes [`StaticCondition::Unknown`].
#[must_use]
pub fn parse_static_condition(test: &Expr) -> StaticCondition {
    match test {
        Expr::BooleanLiteral(lit) => StaticCondition::Bool(lit.value),
        Expr::Name(name) if name.id.as_str() == "TYPE_CHECKING" => StaticCondition::TypeChecking,
        Expr::Attribute(attr) if attr.attr.as_str() == "TYPE_CHECKING" => {
            StaticCondition::TypeChecking
        }
        Expr::UnaryOp(unary) if matches!(unary.op, UnaryOp::Not) => {
            StaticCondition::Not(Box::new(parse_static_condition(&unary.operand)))
        }
        Expr::BoolOp(bool_op) => {
            let parts = bool_op.values.iter().map(parse_static_condition).collect();
            match bool_op.op {
                BoolOp::And => StaticCondition::All(parts),
                BoolOp::Or => StaticCondition::Any(parts),
            }
        }
        Expr::Compare(_) => parse_version_compare(test).unwrap_or(StaticCondition::Unknown),
        _ => StaticCondition::Unknown,
    }
}

/// Evaluate a [`StaticCondition`] at the given `target_version`.
#[must_use]
pub fn evaluate(cond: &StaticCondition, target_version: (u32, u32)) -> BranchTruth {
    match cond {
        StaticCondition::Version { op, guard } => {
            match version_holds(*op, target_version, *guard) {
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

/// Whether `target <op> guard` holds; `None` for operators that do not apply to
/// version tuples (`is`, `in`, …).
fn version_holds(op: CmpOp, target: (u32, u32), guard: (u32, u32)) -> Option<bool> {
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

/// Parse `sys.version_info <op> (major, minor)` in either operand order.
fn parse_version_compare(test: &Expr) -> Option<StaticCondition> {
    let Expr::Compare(cmp) = test else {
        return None;
    };
    if cmp.ops.len() != 1 || cmp.comparators.len() != 1 {
        return None;
    }
    let op = *cmp.ops.first()?;
    let left = cmp.left.as_ref();
    let right = cmp.comparators.first()?;

    if is_version_info_attr(left) {
        return Some(StaticCondition::Version {
            op,
            guard: parse_version_tuple(right)?,
        });
    }
    if is_version_info_attr(right) {
        return Some(StaticCondition::Version {
            op: flip_op(op),
            guard: parse_version_tuple(left)?,
        });
    }
    None
}

/// `true` for the `sys.version_info` attribute expression.
fn is_version_info_attr(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Attribute(attr)
            if matches!(attr.value.as_ref(), Expr::Name(name) if name.id.as_str() == "sys")
                && attr.attr.as_str() == "version_info"
    )
}

/// Flip a comparison operator so the version tuple can always be the right operand.
fn flip_op(op: CmpOp) -> CmpOp {
    match op {
        CmpOp::Lt => CmpOp::Gt,
        CmpOp::LtE => CmpOp::GtE,
        CmpOp::Gt => CmpOp::Lt,
        CmpOp::GtE => CmpOp::LtE,
        other => other,
    }
}

/// Parse `(major, minor)` or `(major, minor, micro)` into `(major, minor)`.
fn parse_version_tuple(expr: &Expr) -> Option<(u32, u32)> {
    let Expr::Tuple(tup) = expr else {
        return None;
    };
    if tup.elts.len() < 2 {
        return None;
    }
    Some((
        int_literal(tup.elts.first()?)?,
        int_literal(tup.elts.get(1)?)?,
    ))
}

/// Extract a `u32` from an integer literal expression.
fn int_literal(expr: &Expr) -> Option<u32> {
    let Expr::NumberLiteral(num) = expr else {
        return None;
    };
    match &num.value {
        Number::Int(value) => value.as_u64().and_then(|v| u32::try_from(v).ok()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use ruff_python_ast::{CmpOp, Stmt};

    use super::{evaluate, parse_static_condition, BranchTruth, StaticCondition};

    fn parse_if_test(source: &str) -> Result<StaticCondition, String> {
        let parsed = basilisk_parser::parse_source(
            source.to_string(),
            "<static-condition-test>".to_string(),
        )
        .map_err(|err| err.to_string())?;
        let Some(Stmt::If(if_stmt)) = parsed.ast.body.first() else {
            return Err("test fixture should start with an if statement".to_string());
        };
        Ok(parse_static_condition(&if_stmt.test))
    }

    #[test]
    fn parses_type_checking_and_boolean_composition() -> Result<(), String> {
        let cond = parse_if_test(
            r"
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
if (3, 11, 7) <= sys.version_info:
    x = 1
",
        )?;

        assert_eq!(
            cond,
            StaticCondition::Version {
                op: CmpOp::GtE,
                guard: (3, 11),
            },
        );
        assert_eq!(evaluate(&cond, (3, 12)), BranchTruth::AlwaysTrue);
        assert_eq!(evaluate(&cond, (3, 10)), BranchTruth::AlwaysFalse);
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
            let cond = StaticCondition::Version { op, guard: (3, 12) };
            assert_eq!(evaluate(&cond, target), expected);
        }
    }

    #[test]
    fn unsupported_version_operator_is_unknown() {
        let cond = StaticCondition::Version {
            op: CmpOp::In,
            guard: (3, 12),
        };
        assert_eq!(evaluate(&cond, (3, 12)), BranchTruth::Unknown);
    }

    #[test]
    fn unsupported_version_shapes_parse_as_unknown() -> Result<(), String> {
        for source in [
            "if sys.version_info >= (3,):\n    x = 1\n",
            "if sys.version_info >= version:\n    x = 1\n",
            "if (3, 12) < platform.version_info:\n    x = 1\n",
            "if sys.version_info < (3.12, 0):\n    x = 1\n",
            "if sys.version_info < (999999999999999999999999, 0):\n    x = 1\n",
        ] {
            assert_eq!(parse_if_test(source)?, StaticCondition::Unknown);
        }
        Ok(())
    }
}
