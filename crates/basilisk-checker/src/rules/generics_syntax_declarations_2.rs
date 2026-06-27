//! Implements [`generics_syntax_declarations_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! `generics_syntax_declarations_2`: Invalid attribute access on bounded type variable.
//!
//! When a PEP 695 type parameter has a bound (e.g., `T: str`), attribute
//! accesses on parameters typed as `T` must be valid for the bound type.
//!
//! ```python
//! class C[T: str]:
//!     def method(self, x: T):
//!         x.capitalize()  # OK - str has capitalize
//!         x.is_integer()  # E - str does NOT have is_integer
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "generics_syntax_declarations_2",
    docs_url: "https://www.basilisk-python.dev/errors/generics_syntax_declarations_2",
};

/// Emits `generics_syntax_declarations_2` for invalid attribute accesses on bounded type variables.
pub(crate) struct BoundedTypeVarAttrAccess;

impl Rule for BoundedTypeVarAttrAccess {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for violation in &module.bounded_typevar_attr_violations {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Attribute `{}` is not defined on `{}`; \
                     parameter `{}` is typed as `{}` (bound to `{}`)",
                    violation.attr_name,
                    violation.bound_type,
                    violation.param_name,
                    violation.typevar_name,
                    violation.bound_type,
                ),
                violation.span,
                &module.path,
                Some(format!(
                    "`{}.{}` is not a method of `{}`. \
                     Only attributes defined on the bound type `{}` are accessible.",
                    violation.param_name,
                    violation.attr_name,
                    violation.bound_type,
                    violation.bound_type,
                )),
                Some(
                    "PEP 695: When a type parameter has a bound, only attributes \
                     defined on the bound type are accessible on the type variable."
                        .to_owned(),
                ),
            ));
        }
    }
}
