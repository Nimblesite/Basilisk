//! BSK-E0065: Access to an `int`-only attribute on a `float`-typed parameter.
//!
//! The Python typing spec (PEP 484 / typing spec §Special cases for float and complex)
//! states that `int` is not a subtype of `float` for static type-checking purposes.
//! Attributes such as `numerator` and `denominator` are defined on `int` but NOT on
//! `float`.  Accessing them on a parameter declared as `float` is therefore a static
//! type error.
//!
//! The check is deliberately conservative — it only fires on **top-level** statements
//! inside a function body, skipping any access inside an `if`/`for`/`while`/`match`/
//! `with`/`try` block.  This means that accesses protected by an `isinstance` guard
//! (where the parameter has been narrowed to `int`) are never flagged.
//!
//! ```python
//! def func1(f: float):
//!     f.numerator  # E — float does not have .numerator
//!
//!     if not isinstance(f, float):
//!         f.numerator  # OK — narrowed to int inside the branch
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0065",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0065",
};

/// Emits BSK-E0065 for `int`-only attribute accesses on `float`-typed parameters.
pub(crate) struct FloatParamIntAttrAccess;

impl Rule for FloatParamIntAttrAccess {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for access in &module.float_param_int_attr_accesses {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Attribute `{}` is not defined on `float`; \
                     parameter `{}` is typed as `float`, not `int`",
                    access.attr_name, access.param_name
                ),
                access.span,
                &module.path,
                Some(format!(
                    "`{}.{}` is only valid when the value is known to be `int`. \
                     Use an `isinstance(f, int)` guard or change the annotation to `int`.",
                    access.param_name, access.attr_name
                )),
                Some(
                    "PEP 484 / typing spec: `int` is NOT a subtype of `float` for \
                     static type-checking. Attributes like `numerator` and `denominator` \
                     are `int`-only and are not available on `float`."
                        .to_owned(),
                ),
            ));
        }
    }
}
