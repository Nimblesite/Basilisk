//! BSK-E0003: Missing variable type (unresolvable inference).
//!
//! Fires when a module-level variable has no type annotation AND the RHS is an
//! empty collection or `None` — cases where the element/value type cannot be
//! inferred from the literal alone.

use basilisk_resolver::{ResolvedModule, RhsKind, VariableInfo};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0003",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0003",
};

/// Emits BSK-E0003 for unannotated module-level variables whose RHS cannot
/// have its type inferred (empty collections or `None`).
pub(crate) struct MissingVariableType;

impl Rule for MissingVariableType {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        module
            .module_vars
            .iter()
            .filter(|var| !var.has_annotation && is_unresolvable(&var.rhs_kind))
            .for_each(|var| diagnostics.push(make_diagnostic(var, &module.path)));
    }
}

/// Returns `true` for RHS kinds whose element/value type cannot be inferred
/// from the literal alone.
fn is_unresolvable(rhs: &RhsKind) -> bool {
    matches!(
        rhs,
        RhsKind::EmptyList | RhsKind::EmptyDict | RhsKind::NoneValue
    )
}

fn make_diagnostic(var: &VariableInfo, path: &str) -> Diagnostic {
    let (rhs_desc, example) = match &var.rhs_kind {
        RhsKind::EmptyList => (
            "empty list `[]`",
            format!("{}: list[<type>] = []", var.name),
        ),
        RhsKind::EmptyDict => (
            "empty dict `{}`",
            format!("{}: dict[<key>, <value>] = {{}}", var.name),
        ),
        RhsKind::NoneValue => ("None", format!("{}: <type> | None = None", var.name)),
        _ => unreachable!("make_diagnostic only called for is_unresolvable rhs kinds"),
    };

    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Cannot infer type of `{}` from {} — add an explicit annotation",
            var.name, rhs_desc
        ),
        span: var.name_span,
        path: path.to_owned(),
        help: Some(format!("Add a type annotation: `{example}`")),
        note: Some(
            "Basilisk cannot infer element types from empty collections or `None`".to_owned(),
        ),
    }
}
