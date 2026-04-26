//! BSK-E0029: Method defined inside a `TypedDict` class.
//!
//! `TypedDict` classes (PEP 589) are restricted to key declarations only.
//! Defining methods (other than `__init__` which is synthesised) is an error.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0029",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0029",
};

/// Emits BSK-E0029 when a method is defined inside a `TypedDict` class.
pub(crate) struct TypedDictMethodNotAllowed;

impl Rule for TypedDictMethodNotAllowed {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for class in module.classes.iter().filter(|c| c.is_typed_dict) {
            for method_name in &class.method_names {
                // __init_subclass__ and __class_getitem__ are synthesised; skip them.
                if matches!(
                    method_name.as_str(),
                    "__init_subclass__" | "__class_getitem__"
                ) {
                    continue;
                }
                let func = module.functions.iter().find(|f| {
                    f.class_name.as_deref() == Some(&class.name) && &f.name == method_name
                });
                if let Some(f) = func {
                    diagnostics.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "Method `{}` is not allowed in TypedDict class `{}`",
                            method_name, class.name
                        ),
                        span: f.name_span,
                        path: module.path.clone(),
                        help: Some(
                            "TypedDict classes may only declare typed fields, not methods"
                                .to_owned(),
                        ),
                        note: Some(
                            "PEP 589: TypedDict does not support method definitions".to_owned(),
                        ),
                    });
                }
            }
        }
    }
}
