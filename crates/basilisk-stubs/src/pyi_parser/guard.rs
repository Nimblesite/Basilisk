//! Static selection of simple version and platform guards in `.pyi` files.

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
    if_stmt: &'a StmtIf,
    target: Option<&StubTarget>,
) -> Vec<Option<&'a [Stmt]>> {
    let mut branches = Vec::new();
    let first = evaluate_guard(&if_stmt.test, target);
    if first.can_be_true {
        branches.push(Some(if_stmt.body.as_slice()));
    }
    let mut can_reach_next = first.can_be_false;

    for clause in &if_stmt.elif_else_clauses {
        if !can_reach_next {
            break;
        }
        if let Some(test) = &clause.test {
            let truth = evaluate_guard(test, target);
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

fn evaluate_guard(expr: &Expr, target: Option<&StubTarget>) -> PossibleTruth {
    match expr {
        Expr::BooleanLiteral(literal) => PossibleTruth::from_bool(literal.value),
        Expr::Name(name) if name.id.as_str() == "TYPE_CHECKING" => PossibleTruth::TRUE,
        Expr::Attribute(attribute)
            if attribute.attr.as_str() == "TYPE_CHECKING"
                && matches!(attribute.value.as_ref(), Expr::Name(name)
                    if matches!(name.id.as_str(), "typing" | "typing_extensions")) =>
        {
            PossibleTruth::TRUE
        }
        Expr::UnaryOp(unary) if matches!(unary.op, UnaryOp::Not) => {
            let inner = evaluate_guard(&unary.operand, target);
            PossibleTruth {
                can_be_true: inner.can_be_false,
                can_be_false: inner.can_be_true,
            }
        }
        Expr::BoolOp(boolean) => evaluate_boolean_guard(boolean.op, &boolean.values, target),
        Expr::Compare(_) => evaluate_comparison_guard(expr, target),
        _ => PossibleTruth::EITHER,
    }
}

fn evaluate_boolean_guard(
    operator: BoolOp,
    values: &[Expr],
    target: Option<&StubTarget>,
) -> PossibleTruth {
    let truths: Vec<PossibleTruth> = values
        .iter()
        .map(|value| evaluate_guard(value, target))
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

fn evaluate_comparison_guard(expr: &Expr, target: Option<&StubTarget>) -> PossibleTruth {
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
    if is_sys_attribute(&compare.left, "version_info") {
        return version_guard(operator, right, target);
    }
    if is_sys_attribute(right, "version_info") {
        return version_guard(flip_comparison(operator), &compare.left, target);
    }
    if is_sys_attribute(&compare.left, "platform") {
        return platform_guard(operator, right, target);
    }
    if is_sys_attribute(right, "platform") {
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

fn is_sys_attribute(expr: &Expr, attribute_name: &str) -> bool {
    matches!(expr, Expr::Attribute(attribute)
        if attribute.attr.as_str() == attribute_name
            && matches!(attribute.value.as_ref(), Expr::Name(name) if name.id.as_str() == "sys"))
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
        found: std::collections::BTreeSet::new(),
    };
    for stmt in body {
        collector.visit_stmt(stmt);
    }
    collector.found
}

struct PlatformLiterals {
    found: std::collections::BTreeSet<String>,
}

impl ruff_python_ast::visitor::Visitor<'_> for PlatformLiterals {
    fn visit_expr(&mut self, expr: &Expr) {
        if let Expr::Compare(compare) = expr {
            let sides = std::iter::once(compare.left.as_ref()).chain(compare.comparators.iter());
            let names_platform = sides.clone().any(|side| is_sys_attribute(side, "platform"));
            if names_platform {
                self.found
                    .extend(sides.filter_map(string_literal));
            }
        }
        ruff_python_ast::visitor::walk_expr(self, expr);
    }
}
