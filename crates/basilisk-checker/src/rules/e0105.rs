//! BSK-E0105: Invalid attribute access on bounded type variable.
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

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0105",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0105",
};

/// Emits BSK-E0105 for invalid attribute accesses on bounded type variables.
pub(crate) struct BoundedTypeVarAttrAccess;

impl Rule for BoundedTypeVarAttrAccess {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for violation in &module.bounded_typevar_attr_violations {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Attribute `{}` is not defined on `{}`; \
                     parameter `{}` is typed as `{}` (bound to `{}`)",
                    violation.attr_name,
                    violation.bound_type,
                    violation.param_name,
                    violation.typevar_name,
                    violation.bound_type,
                ),
                span: violation.span,
                path: module.path.clone(),
                help: Some(format!(
                    "`{}.{}` is not a method of `{}`. \
                     Only attributes defined on the bound type `{}` are accessible.",
                    violation.param_name,
                    violation.attr_name,
                    violation.bound_type,
                    violation.bound_type,
                )),
                note: Some(
                    "PEP 695: When a type parameter has a bound, only attributes \
                     defined on the bound type are accessible on the type variable."
                        .to_owned(),
                ),
                provenance: None,
            });
        }
    }
}
