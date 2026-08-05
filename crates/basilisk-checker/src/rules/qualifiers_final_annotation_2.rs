//! Implements [`qualifiers_final_annotation_2`] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
//! `qualifiers_final_annotation_2`: `Final` type qualifier annotation violations.
//!
//! Detects violations of PEP 591's rules for the `Final` qualifier, beyond the
//! positional errors handled by E0044. Specifically:
//!
//! 1. **Class attribute `Final` without init** — `ID2: Final` / `ID3: Final[int]`
//!    in a class body without an initializer and not assigned in `__init__`.
//!
//! 2. **Instance `Final` outside `__init__`** — `self.id3: Final = 1` in a method
//!    other than `__init__`.
//!
//! 3. **Re-assignment to already-initialized Final** — `self.ID5 = 0` when
//!    `ID5: Final[int] = 0` is already given a value in the class body.
//!
//! 4. **Modification of Final class attribute** — `self.ID7 = 0` / `self.ID7 += 1`
//!    when `ID7` is declared `Final` in the class body.
//!
//! 5. **Imported Final re-assignment** — `RATE = 300` where `RATE` is imported
//!    from a module that declares it `Final`.
//!
//! 6. **Subclass override of Final** — `BORDER_WIDTH = 2.5` in a subclass when the
//!    parent declares `BORDER_WIDTH: Final = 2.5`.
//!
//! 7. **Function-local Final modification** — `x += 1` when `x: Final = 3`, or
//!    walrus/for/with/tuple-unpack on a `Final` variable.

use basilisk_resolver::{FinalViolationKind, ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "qualifiers_final_annotation_2",
    docs_url: "https://www.basilisk-python.dev/errors/qualifiers_final_annotation_2",
};

fn make_diagnostic(message: String, span: Span, path: &str, help: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        message,
        span,
        path,
        Some(help.to_owned()),
        Some("PEP 591: `Final` names may only be assigned once at declaration time".to_owned()),
    )
}

/// Emits `qualifiers_final_annotation_2` for `Final` annotation violations collected during resolution.
pub(crate) struct FinalAnnotationViolation;

impl Rule for FinalAnnotationViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let path = &module.path;
        check_final_violations(module, path, diagnostics);
        check_module_bare_assignments(module, path, diagnostics);
    }
}

fn check_final_violations(module: &ResolvedModule, path: &str, diagnostics: &mut Vec<Diagnostic>) {
    for violation in &module.final_violations {
        let span = violation.span;
        let name = &violation.name;
        match &violation.kind {
            FinalViolationKind::ClassFinalWithoutInit => {
                diagnostics.push(make_diagnostic(
                    format!(
                        "`{name}` is annotated `Final` in the class body but has no \
                         initializer and is not unconditionally assigned in `__init__`"
                    ),
                    span,
                    path,
                    "Either add an initializer (`ID: Final = value`) or assign \
                     unconditionally in `__init__`",
                ));
            }
            FinalViolationKind::InstanceFinalOutsideInit => {
                diagnostics.push(make_diagnostic(
                    format!("`self.{name}: Final` annotation is only allowed inside `__init__`"),
                    span,
                    path,
                    "Move `Final` instance attribute declarations to `__init__`",
                ));
            }
            FinalViolationKind::InstanceReassignAlreadyInitialized => {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Cannot assign to `self.{name}` — it is already initialized as \
                         `Final` in the class body"
                    ),
                    span,
                    path,
                    "Remove the assignment or remove the `Final` annotation",
                ));
            }
            FinalViolationKind::InstanceModifyFinal => {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Cannot assign to `self.{name}` — it is declared `Final` in \
                         the class body"
                    ),
                    span,
                    path,
                    "Remove the assignment or remove the `Final` annotation",
                ));
            }
            FinalViolationKind::SubclassOverrideFinal => {
                diagnostics.push(make_diagnostic(
                    format!("Cannot override `{name}` — it is declared `Final` in a base class"),
                    span,
                    path,
                    "Remove the override or rename the attribute",
                ));
            }
            FinalViolationKind::FunctionLocalFinalModification => {
                diagnostics.push(make_diagnostic(
                    format!("Cannot modify `{name}` — it is declared `Final`"),
                    span,
                    path,
                    "Remove the modification or remove the `Final` annotation",
                ));
            }
            FinalViolationKind::GlobalFinalModification
            | FinalViolationKind::ModuleLevelReassignment
            | FinalViolationKind::ClassAttributeReassignment => {}
        }
    }
}

fn check_module_bare_assignments(
    module: &ResolvedModule,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for bare in &module.module_bare_assignments {
        if module.imported_final_names.contains(&bare.name) {
            diagnostics.push(make_diagnostic(
                format!(
                    "Cannot re-assign `{}` — it is declared `Final` in an imported module",
                    bare.name
                ),
                bare.name_span,
                path,
                "Remove the re-assignment or remove the import",
            ));
        }
    }
}
