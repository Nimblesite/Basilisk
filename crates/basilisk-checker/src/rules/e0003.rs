//! Implements [BSK-E0003] from [CHKARCH-DIAG-MISSING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-missing
//! BSK-E0003: Missing variable type annotation.
//!
//! Fires when a module-level variable has no type annotation.  In strict mode
//! every module-level binding must carry an explicit annotation so that
//! Basilisk can verify downstream usage and generate accurate stubs.

use basilisk_resolver::{ResolvedModule, RhsKind, VariableInfo};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0003",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0003",
};

/// Emits BSK-E0003 for every unannotated module-level variable.
pub(crate) struct MissingVariableType;

impl Rule for MissingVariableType {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
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
    let (message, help) = match &var.rhs_kind {
        RhsKind::EmptyList => (
            format!(
                "Missing type annotation for `{}` — cannot infer element type from empty list `[]`",
                var.name
            ),
            format!("{}: list[<type>] = []", var.name),
        ),
        RhsKind::EmptyDict => (
            format!(
                "Missing type annotation for `{}` — cannot infer key/value types from empty dict `{{}}`",
                var.name
            ),
            format!("{}: dict[<key>, <value>] = {{}}", var.name),
        ),
        RhsKind::NoneValue => (
            format!(
                "Missing type annotation for `{}` — cannot infer type from `None`",
                var.name
            ),
            format!("{}: <type> | None = None", var.name),
        ),
        _ => (
            format!(
                "Missing type annotation for module-level variable `{}`",
                var.name
            ),
            format!("{}: <type> = ...", var.name),
        ),
    };

    error_diagnostic_owned(
        CODE.clone(),
        message,
        var.name_span,
        path,
        Some(format!("Add a type annotation: `{help}`")),
        Some(
            "In Basilisk, all module-level variables require explicit type annotations".to_owned(),
        ),
    )
}
