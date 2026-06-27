//! Implements [BSK-E0046] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-immutability
//! BSK-E0046: Enum member annotated with an explicit type.
//!
//! In an Enum class, members should NOT carry explicit type annotations.
//! If an attribute inside an Enum class body has both a type annotation and
//! an assigned value, it is treated as an annotated member — which is an error
//! because the type checker infers a `Literal[EnumClass.member]` type for all
//! members automatically.
//!
//! A type annotation without an assigned value (e.g. `genus: str`) is a
//! **non-member attribute** and is valid.
//!
//! ```python
//! from enum import Enum
//!
//! class Pet(Enum):
//!     genus: str           # OK — non-member attribute (annotation only, no value)
//!     CAT = "felis"        # OK — member without annotation
//!     DOG: int = 2         # E — member with explicit type annotation
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::{guards::is_enum_class, Rule};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0046",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0046",
};

/// Emits BSK-E0046 when an Enum member carries an explicit type annotation.
pub(crate) struct EnumMemberAnnotated;

impl Rule for EnumMemberAnnotated {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for cls in module.classes.iter().filter(|c| is_enum_class(c)) {
            for attr in cls
                .attributes
                .iter()
                .filter(|a| a.has_annotation && a.has_value)
            {
                // Skip dunder names — _value_, _name_, _ignore_ etc. are special enum
                // class attributes, not members.
                if attr.name.starts_with('_') && attr.name.ends_with('_') {
                    continue;
                }
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Enum member `{}` in `{}` should not have an explicit type annotation",
                        attr.name, cls.name
                    ),
                    attr.name_span,
                    &module.path,
                    Some(format!(
                        "Remove the type annotation from `{}` — enum members have an inferred \
                         `Literal[{}.{}]` type",
                        attr.name, cls.name, attr.name
                    )),
                    Some(
                        "PEP 435: Enum members should not carry explicit type annotations; \
                         use an annotation-only field (no value) for non-member attributes"
                            .to_owned(),
                    ),
                ));
            }
        }
    }
}
