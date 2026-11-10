//! Static selection of simple version and platform guards in `.pyi` files.
//!
//! Implements the "simple version and platform checks" of the
//! [typing spec's directives section](https://typing.python.org/en/latest/spec/directives.html):
//! `sys.version_info` / `sys.platform` comparisons and `TYPE_CHECKING`, which
//! is considered `True` during type checking. Recognition resolves the guard
//! expression through the module's own bindings ([RESOLV-CANONICAL-BINDING]) —
//! `import sys as s` guards evaluate identically, and a module that rebinds
//! `sys` or `TYPE_CHECKING` decides nothing.

use basilisk_canonical::{BindingTable, TypingForm};
use ruff_python_ast::{BoolOp, CmpOp, Expr, Number, Stmt, StmtIf, UnaryOp};

use crate::types::{StubTarget, StubTargetPlatform};

#[derive(Clone, Copy)]
struct PossibleTruth {
    can_be_true: bool,
    can_be_false: bool,
}

impl PossibleTruth {
    const TRUE: Self = Self {
        can_be_true: true,
        can_be_false: false,
    };
    const FALSE: Self = Self {
        can_be_true: false,
        can_be_false: true,
    };
    const EITHER: Self = Self {
        can_be_true: true,
        can_be_false: true,
    };

    fn from_bool(value: bool) -> Self {
        if value {
            Self::TRUE
        } else {
            Self::FALSE
        }
    }
}

pub(super) fn feasible_branches<'a>(
    bindings: &BindingTable,
    if_stmt: &'a StmtIf,
    target: Option<&StubTarget>,
) -> Vec<Option<&'a [Stmt]>> {
    let mut branches = Vec::new();
    let first = evaluate_guard(bindings, &if_stmt.test, target);
    if first.can_be_true {
        branches.push(Some(if_stmt.body.as_slice()));
    }
    let mut can_reach_next = first.can_be_false;

    for clause in &if_stmt.elif_else_clauses {
        if !can_reach_next {
            break;
        }
        if let Some(test) = &clause.test {
            let truth = evaluate_guard(bindings, test, target);
            if truth.can_be_true {
                branches.push(Some(clause.body.as_slice()));
            }
            can_reach_next &= truth.can_be_false;
        } else {
            branches.push(Some(clause.body.as_slice()));
            can_reach_next = false;
        }
    }
    if can_reach_next {
        branches.push(None);
    }
    branches
}

fn evaluate_guard(
    bindings: &BindingTable,
    expr: &Expr,
    target: Option<&StubTarget>,
) -> PossibleTruth {
    match expr {
        Expr::BooleanLiteral(literal) => PossibleTruth::from_bool(literal.value),
        Expr::UnaryOp(unary) if matches!(unary.op, UnaryOp::Not) => {
            let inner = evaluate_guard(bindings, &unary.operand, target);
            PossibleTruth {
                can_be_true: inner.can_be_false,
                can_be_false: inner.can_be_true,
            }
        }
        Expr::BoolOp(boolean) => {
            evaluate_boolean_guard(bindings, boolean.op, &boolean.values, target)
        }
        Expr::Compare(_) => evaluate_comparison_guard(bindings, expr, target),
        // "Considered True during type checking" — spec directives. Only an
        // expression that RESOLVES to the flag qualifies; a bare unimported
        // name or a rebound one stays undecidable.
        Expr::Name(_) | Expr::Attribute(_)
            if bindings.is_form(expr, TypingForm::TypeCheckingFlag) =>
        {
            PossibleTruth::TRUE
        }
        _ => PossibleTruth::EITHER,
    }
}

fn evaluate_boolean_guard(
    bindings: &BindingTable,
    operator: BoolOp,
    values: &[Expr],
    target: Option<&StubTarget>,
) -> PossibleTruth {
    let truths: Vec<PossibleTruth> = values
        .iter()
        .map(|value| evaluate_guard(bindings, value, target))
        .collect();
    match operator {
        BoolOp::And => PossibleTruth {
            can_be_true: truths.iter().all(|truth| truth.can_be_true),
            can_be_false: truths.iter().any(|truth| truth.can_be_false),
        },
        BoolOp::Or => PossibleTruth {
            can_be_true: truths.iter().any(|truth| truth.can_be_true),
            can_be_false: truths.iter().all(|truth| truth.can_be_false),
        },
    }
}

fn evaluate_comparison_guard(
    bindings: &BindingTable,
    expr: &Expr,
    target: Option<&StubTarget>,
) -> PossibleTruth {
    let Expr::Compare(compare) = expr else {
        return PossibleTruth::EITHER;
    };
    if compare.ops.len() != 1 || compare.comparators.len() != 1 {
        return PossibleTruth::EITHER;
    }
    let Some(operator) = compare.ops.first().copied() else {
        return PossibleTruth::EITHER;
    };
    let Some(right) = compare.comparators.first() else {
        return PossibleTruth::EITHER;
    };
    if resolves_to_sys(bindings, &compare.left, "version_info") {
        return version_guard(operator, right, target);
    }
    if resolves_to_sys(bindings, right, "version_info") {
        return version_guard(flip_comparison(operator), &compare.left, target);
    }
    if resolves_to_sys(bindings, &compare.left, "platform") {
        return platform_guard(operator, right, target);
    }
    if resolves_to_sys(bindings, right, "platform") {
        return platform_guard(flip_comparison(operator), &compare.left, target);
    }
    PossibleTruth::EITHER
}

fn version_guard(operator: CmpOp, guard_expr: &Expr, target: Option<&StubTarget>) -> PossibleTruth {
    let (Some(target), Some(guard)) = (target, version_tuple(guard_expr)) else {
        return PossibleTruth::EITHER;
    };
    compare_ordered(operator, &target.python_version, &guard)
        .map_or(PossibleTruth::EITHER, PossibleTruth::from_bool)
}

fn platform_guard(
    operator: CmpOp,
    guard_expr: &Expr,
    target: Option<&StubTarget>,
) -> PossibleTruth {
    let Some(guard) = string_literal(guard_expr) else {
        return PossibleTruth::EITHER;
    };
    let Some(StubTarget {
        platform: StubTargetPlatform::Concrete(platform),
        ..
    }) = target
    else {
        return PossibleTruth::EITHER;
    };
    compare_ordered(operator, platform.as_str(), guard.as_str())
        .map_or(PossibleTruth::EITHER, PossibleTruth::from_bool)
}

fn compare_ordered<T: Ord + ?Sized>(operator: CmpOp, left: &T, right: &T) -> Option<bool> {
    Some(match operator {
        CmpOp::Lt => left < right,
        CmpOp::LtE => left <= right,
        CmpOp::Gt => left > right,
        CmpOp::GtE => left >= right,
        CmpOp::Eq => left == right,
        CmpOp::NotEq => left != right,
        _ => return None,
    })
}

fn flip_comparison(operator: CmpOp) -> CmpOp {
    match operator {
        CmpOp::Lt => CmpOp::Gt,
        CmpOp::LtE => CmpOp::GtE,
        CmpOp::Gt => CmpOp::Lt,
        CmpOp::GtE => CmpOp::LtE,
        other => other,
    }
}

/// Whether an expression resolves to `sys.<name>` through the module's own
/// bindings — so `import sys as s; s.version_info` and
/// `from sys import version_info` qualify, and a module that rebinds `sys`
/// does not. The `Name | Attribute` guard keeps subscripted and called forms
/// (`sys.version_info[0]`) honestly undecidable rather than unwrapped.
fn resolves_to_sys(bindings: &BindingTable, expr: &Expr, name: &str) -> bool {
    matches!(expr, Expr::Name(_) | Expr::Attribute(_)) && bindings.resolves_to(expr, "sys", name)
}

fn version_tuple(expr: &Expr) -> Option<(u32, u32)> {
    let Expr::Tuple(tuple) = expr else {
        return None;
    };
    Some((
        integer_literal(tuple.elts.first()?)?,
        integer_literal(tuple.elts.get(1)?)?,
    ))
}

fn integer_literal(expr: &Expr) -> Option<u32> {
    let Expr::NumberLiteral(number) = expr else {
        return None;
    };
    match &number.value {
        Number::Int(value) => value.as_u64().and_then(|value| u32::try_from(value).ok()),
        _ => None,
    }
}

fn string_literal(expr: &Expr) -> Option<String> {
    match expr {
        Expr::StringLiteral(literal) => Some(literal.value.to_string()),
        _ => None,
    }
}

/// Every string literal a `sys.platform` comparison anywhere in `body` tests
/// against.
///
/// [`platform_guard`] is the only thing that makes an extracted stub depend on
/// the target platform, and it only ever compares the target against these
/// literals. Two platform values that compare identically against all of them
/// therefore select identical declarations — which is what lets the
/// precomputed builtins index enumerate a finite, provably complete set of
/// platform variants ([STUBRES-TYPESHED-BUILTINS-INDEX]).
pub(crate) fn platform_guard_literals(body: &[Stmt]) -> std::collections::BTreeSet<String> {
    use ruff_python_ast::visitor::Visitor as _;

    let mut collector = PlatformLiterals {
        bindings: BindingTable::from_module(body),
        found: std::collections::BTreeSet::new(),
    };
    for stmt in body {
        collector.visit_stmt(stmt);
    }
    collector.found
}

struct PlatformLiterals {
    /// The module's own bindings — comparisons are recognised exactly as
    /// [`evaluate_comparison_guard`] recognises them, so the collected set is
    /// complete for the guards that can actually fire.
    bindings: BindingTable,
    found: std::collections::BTreeSet<String>,
}

impl ruff_python_ast::visitor::Visitor<'_> for PlatformLiterals {
    fn visit_expr(&mut self, expr: &Expr) {
        if let Expr::Compare(compare) = expr {
            let sides = std::iter::once(compare.left.as_ref()).chain(compare.comparators.iter());
            let names_platform = sides
                .clone()
                .any(|side| resolves_to_sys(&self.bindings, side, "platform"));
            if names_platform {
                self.found.extend(sides.filter_map(string_literal));
            }
        }
        ruff_python_ast::visitor::walk_expr(self, expr);
    }
}
