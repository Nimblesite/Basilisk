//! Implements [NARROWPLAN-INTEGRATION] Step 2 / [TYPEINF-FUNC-RETURN]. See
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-INTEGRATION
//!
//! The returned-value judgment shared by the two return-mismatch rules: the
//! engine types the RETURNED EXPRESSION (not a syntactic shape class), so
//! `return returns_str()` in a `-> int` function is finally an error — the
//! return half of [#378](https://github.com/Nimblesite/Basilisk/issues/378).
//!
//! A call whose callee the module cannot ground still types `Unknown` and
//! abstains, which is why widening from "skip every call" to "judge every
//! call" adds catches without adding false positives.

use basilisk_resolver::ReturnStmtInfo;
use ruff_python_ast::Expr;

use crate::types::InferredType;

use super::judge::TypeJudge;

/// What the judgment concluded about one `return` statement.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ReturnVerdict {
    /// The returned value fits the declared type, or there is no evidence
    /// either way — both are silence.
    Silent,
    /// The returned value is grounded and does NOT fit; the payload is the
    /// engine's type for it, for the diagnostic message.
    Mismatch(InferredType),
}

/// Judge one `return` statement's value against the function's declared return
/// type.
///
/// The `-> None` target is judged like any other: a valued `return` mismatches
/// unless the value's type IS `None`. It differs only in that an unresolvable
/// call stays silent — `return f(self)` where `f` is untyped may legitimately
/// return `None`, and the gradual guarantee forbids inventing an error for it.
pub(crate) fn judge_return(
    judge: &TypeJudge<'_, '_>,
    stmt: &ReturnStmtInfo,
    declared: &InferredType,
) -> ReturnVerdict {
    let span = stmt.value_span;
    let inferred = judge.inferred(span);
    if matches!(inferred, InferredType::Unknown) {
        return ReturnVerdict::Silent;
    }
    if judge.fits(&inferred, declared)
        || judge.display_checks(span, declared)
        || !judge.judgeable(declared)
    {
        return ReturnVerdict::Silent;
    }
    ReturnVerdict::Mismatch(inferred)
}

/// Does this valued `return` in a `-> None` function fire?
///
/// The rule predates the engine and fires on the SHAPE of the statement, so it
/// keeps firing wherever it used to; the engine only removes firings it can
/// disprove — a value the engine types `None`, or a call whose return the
/// engine cannot resolve (the pre-engine rule skipped every call for exactly
/// that reason, and the gradual guarantee keeps that skip).
pub(crate) fn none_return_fires(judge: &TypeJudge<'_, '_>, stmt: &ReturnStmtInfo) -> bool {
    let span = stmt.value_span;
    let inferred = judge.inferred(span);
    match inferred {
        InferredType::None_ | InferredType::Any => false,
        InferredType::Unknown => !unresolved_call(judge, stmt),
        _ => true,
    }
}

/// Is the returned expression a call the engine could not resolve?
fn unresolved_call(judge: &TypeJudge<'_, '_>, stmt: &ReturnStmtInfo) -> bool {
    matches!(judge.node(stmt.value_span), Some(Expr::Call(_)))
}
