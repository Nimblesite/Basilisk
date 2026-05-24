//! Implements [BSK-E0032] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-ownership
//! BSK-E0032: Invalid keyword argument in `TypedDict` class definition.
//!
//! `TypedDict` class syntax only accepts `total=True/False` as a keyword argument.
//! Using `metaclass=` or any unrecognised keyword is an error per PEP 589.
//!
//! Also fires when a `TypedDict` inherits from a non-`TypedDict` class (other
//! than `Generic[...]`), which is forbidden.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0032",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0032",
};

/// Emits BSK-E0032 for invalid keyword arguments or bases in a `TypedDict` class.
pub(crate) struct InvalidTypedDictBase;

/// Keywords with defined `TypedDict` semantics — anything else is an error.
const KNOWN_TYPED_DICT_KEYWORDS: &[&str] = &["total", "extra_items", "closed"];

impl Rule for InvalidTypedDictBase {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for class in module.classes.iter().filter(|c| c.is_typed_dict) {
            for kw in &class.class_keywords {
                if !KNOWN_TYPED_DICT_KEYWORDS.contains(&kw.as_str()) {
                    diagnostics.push(error_diagnostic_owned(
                        CODE.clone(),
                        format!(
                            "TypedDict class `{}` uses unrecognised keyword `{kw}`",
                            class.name
                        ),
                        class.name_span,
                        &module.path,
                        Some(format!(
                            "Remove `{kw}=` — TypedDict only accepts `total`, `extra_items`, or `closed`"
                        )),
                        Some(
                            "PEP 589: unrecognised keyword arguments in TypedDict are invalid"
                                .to_owned(),
                        ),
                    ));
                }
            }
        }
    }
}
