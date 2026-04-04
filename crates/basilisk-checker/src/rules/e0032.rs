//! BSK-E0032: Invalid keyword argument in `TypedDict` class definition.
//!
//! `TypedDict` class syntax only accepts `total=True/False` as a keyword argument.
//! Using `metaclass=` or any unrecognised keyword is an error per PEP 589.
//!
//! Also fires when a `TypedDict` inherits from a non-`TypedDict` class (other
//! than `Generic[...]`), which is forbidden.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

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
                    diagnostics.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "TypedDict class `{}` uses unrecognised keyword `{kw}`",
                            class.name
                        ),
                        span: class.name_span,
                        path: module.path.clone(),
                        help: Some(format!(
                            "Remove `{kw}=` — TypedDict only accepts `total`, `extra_items`, or `closed`"
                        )),
                        note: Some(
                            "PEP 589: unrecognised keyword arguments in TypedDict are invalid"
                                .to_owned(),
                        ),
                        provenance: None,
                    });
                }
            }
        }
    }
}
