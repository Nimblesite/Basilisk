//! BSK-E0068: `Literal["EnumClass.MEMBER"]` (string) used where
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

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0068",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0068",
};

/// Emits BSK-E0068 when a `Literal["Class.Member"]` string annotation is used
/// where `Literal[Class.Member]` is required.
pub(crate) struct LiteralStringEnumMismatch;

impl Rule for LiteralStringEnumMismatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for mismatch in &module.literal_string_enum_mismatches {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Variable `{}` is annotated as `{}` (a string literal), \
                     but `{}` is an enum member reference, not a string",
                    mismatch.var_name, mismatch.annotation, mismatch.enum_form
                ),
                span: mismatch.span,
                path: module.path.clone(),
                help: Some(format!(
                    "Change the annotation from `Literal[\"{}\"]` to `Literal[{}]` \
                     to reference the enum member directly",
                    mismatch.enum_form, mismatch.enum_form
                )),
                note: Some(
                    "PEP 586 / typing spec: `Literal[\"Color.RED\"]` is a string literal \
                     type; `Literal[Color.RED]` is the enum member type. \
                     These are distinct and incompatible types."
                        .to_owned(),
                ),
            });
        }
    }
}
