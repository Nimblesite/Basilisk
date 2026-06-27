//! Implements [`literals_parameterizations_2`] from [CHKARCH-DIAG-COERCION]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-coercion
//! `literals_parameterizations_2`: `Literal["EnumClass.MEMBER"]` (string) used where
//! `Literal[EnumClass.MEMBER]` (enum member reference) is required.
//!
//! A quoted string like `"Color.RED"` is a `str` literal — it is NOT the same
//! as the enum member `Color.RED`.  When a variable is declared as
//! `Literal[Color.RED]` but assigned from a parameter typed as
//! `Literal["Color.RED"]`, the types are incompatible.
//!
//! ```python
//! from enum import Enum
//! from typing import Literal
//!
//! class Color(Enum):
//!     RED = 1
//!
//! def func2(a: Literal[Color.RED]) -> None:
//!     x1: Literal["Color.RED"] = a  # E — string literal != enum member
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "literals_parameterizations_2",
    docs_url: "https://www.basilisk-python.dev/errors/literals_parameterizations_2",
};

/// Emits `literals_parameterizations_2` when a `Literal["Class.Member"]` string annotation is used
/// where `Literal[Class.Member]` is required.
pub(crate) struct LiteralStringEnumMismatch;

impl Rule for LiteralStringEnumMismatch {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for mismatch in &module.literal_string_enum_mismatches {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Variable `{}` is annotated as `{}` (a string literal), \
                     but `{}` is an enum member reference, not a string",
                    mismatch.var_name, mismatch.annotation, mismatch.enum_form
                ),
                mismatch.span,
                &module.path,
                Some(format!(
                    "Change the annotation from `Literal[\"{}\"]` to `Literal[{}]` \
                     to reference the enum member directly",
                    mismatch.enum_form, mismatch.enum_form
                )),
                Some(
                    "PEP 586 / typing spec: `Literal[\"Color.RED\"]` is a string literal \
                     type; `Literal[Color.RED]` is the enum member type. \
                     These are distinct and incompatible types."
                        .to_owned(),
                ),
            ));
        }
    }
}
