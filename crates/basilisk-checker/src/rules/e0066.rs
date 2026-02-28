//! BSK-E0066: Enum member value incompatible with `_value_` type annotation.
//!
//! When an enum class declares `_value_: T` (annotation-only, no value), all
//! member values assigned in the class body must be compatible with `T`.
//! Additionally, if `self._value_ = param` appears in `__init__`, the
//! parameter's type annotation must be compatible with the declared `_value_: T`.
//!
//! ```python
//! from enum import Enum
//!
//! class Color(Enum):
//!     _value_: int
//!     RED = 1          # OK — int matches int
//!     GREEN = "green"  # E — str is not compatible with int
//!
//! class Planet(Enum):
//!     _value_: str
//!
//!     def __init__(self, value: int, mass: float, radius: float):
//!         self._value_ = value  # E — int is not compatible with str
//! ```

use basilisk_resolver::{EnumValueTypeViolationKind, ResolvedModule};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0066",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0066",
};

/// Emits BSK-E0066 when an enum member value is incompatible with `_value_: T`.
pub(crate) struct EnumValueTypeMismatch;

impl Rule for EnumValueTypeMismatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for violation in &module.enum_value_type_violations {
            let (message, help) = match violation.kind {
                EnumValueTypeViolationKind::MemberValueTypeMismatch => (
                    format!(
                        "Enum member value has type `{}` but `_value_` is declared as `{}` in `{}`",
                        violation.actual_type, violation.declared_type, violation.class_name
                    ),
                    format!(
                        "Change the member value to be compatible with `{}`, \
                         or update the `_value_: {}` annotation",
                        violation.declared_type, violation.declared_type
                    ),
                ),
                EnumValueTypeViolationKind::InitValueParamTypeMismatch => (
                    format!(
                        "`self._value_` is assigned from a parameter of type `{}` \
                         but `_value_` is declared as `{}` in `{}`",
                        violation.actual_type, violation.declared_type, violation.class_name
                    ),
                    format!(
                        "Change the parameter type to `{}` to match the `_value_: {}` annotation, \
                         or update the `_value_` annotation",
                        violation.declared_type, violation.declared_type
                    ),
                ),
            };
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message,
                span: violation.span,
                path: module.path.clone(),
                help: Some(help),
                note: Some(
                    "PEP 435 / typing spec: When `_value_: T` is declared in an enum class, \
                     all member values must be compatible with `T`"
                        .to_owned(),
                ),
            });
        }
    }
}
