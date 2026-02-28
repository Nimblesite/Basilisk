//! BSK-E0026: `TypeVar` declared with exactly one constraint.
//!
//! PEP 484 requires a `TypeVar` to have either zero constraints (unconstrained)
//! or two or more constraints.  A single constraint makes no sense because it
//! would be equivalent to using the type directly.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0026",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0026",
};

/// Emits BSK-E0026 when a `TypeVar` is declared with exactly one constraint.
pub(crate) struct TypeVarSingleConstraint;

impl Rule for TypeVarSingleConstraint {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for tv in &module.typevar_calls {
            if tv.constraint_count == 1 {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "`{}` has a single constraint; TypeVar requires 0 or 2+ constraints",
                        tv.name
                    ),
                    span: tv.span,
                    path: module.path.clone(),
                    help: Some(
                        "Add a second constraint or remove the single constraint".to_owned(),
                    ),
                    note: Some(
                        "PEP 484: a TypeVar with one constraint is invalid".to_owned(),
                    ),
                });
            }
        }
    }
}
