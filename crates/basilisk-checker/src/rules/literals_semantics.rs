//! Implements [`literals_semantics`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `literals_semantics`: Augmented assignment widens `Literal` type.
//!
//! Implements the supported annotated slice of [TYPEINF-VARS-AUGMENTED]:
//! augmented assignment keeps the declared target type and validates whether
//! the operation widens out of it.
//!
//! When a function parameter is annotated with `Literal[...]`, augmented
//! assignment (`+=`, `-=`, etc.) effectively reassigns the variable to a
//! widened type (e.g. `int` instead of `Literal[3, 4, 5]`), violating the
//! declared `Literal` constraint.
//!
//! ```python
//! def func(a: Literal[3, 4, 5]):
//!     a += 3  # E0100 — augmented assign widens Literal type
//! ```

use basilisk_resolver::{ResolvedModule, Span};

use super::Rule;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

const CODE: ErrorCode = ErrorCode {
    code: "literals_semantics",
    docs_url: "https://www.basilisk-python.dev/errors/literals_semantics",
};

/// Emits `literals_semantics` for augmented assignment on `Literal`-typed parameters.
pub(crate) struct LiteralAugmentedAssign;

impl Rule for LiteralAugmentedAssign {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for violation in &module.literal_augmented_assign_violations {
            diagnostics.push(make_diagnostic(
                &violation.var_name,
                violation.span,
                &module.path,
            ));
        }
    }
}

fn make_diagnostic(var_name: &str, span: Span, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!("Augmented assignment to `{var_name}` widens its `Literal` type"),
        span,
        path,
        Some(format!(
            "Use a separate variable instead: `result = {var_name} + ...`"
        )),
        Some(
            "`a += x` is equivalent to `a = a + x`, which changes the type of `a` \
             from `Literal[...]` to the wider base type"
                .to_owned(),
        ),
    )
}
