//! BSK-E0059: Access to `__match_args__` on a dataclass with `match_args=False`.
//!
//! When `@dataclass(match_args=False)` is specified, Python does **not** generate
//! the `__match_args__` class variable.  Accessing `ClassName.__match_args__` on
//! such a class is an `AttributeError` at runtime and a static type error.
//!
//! ```python
//! from dataclasses import dataclass
//!
//! @dataclass(match_args=False)
//! class DC4:
//!     x: int
//!
//! DC4.__match_args__  # E: attribute not generated
//! ```

use std::collections::HashSet;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, error_diagnostic_owned};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0059",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0059",
};

/// Emits BSK-E0059 when `__match_args__` is accessed on a dataclass with `match_args=False`.
pub(crate) struct MatchArgsFalseAccess;

impl Rule for MatchArgsFalseAccess {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Collect names of dataclasses that have match_args=False AND do not
        // already define __match_args__ in their body.
        let no_match_args: HashSet<&str> = module
            .classes
            .iter()
            .filter(|cls| cls.is_dataclass && cls.is_dataclass_match_args_false)
            .filter(|cls| {
                // If the class body explicitly defines __match_args__ the attribute
                // exists so there is no error.
                !cls.attributes.iter().any(|a| a.name == "__match_args__")
            })
            .map(|cls| cls.name.as_str())
            .collect();

        if no_match_args.is_empty() {
            return;
        }

        for access in &module.module_attr_accesses {
            if access.attr_name == "__match_args__"
                && no_match_args.contains(access.object_name.as_str())
            {
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "`{}.__match_args__` does not exist: \
                         `@dataclass(match_args=False)` suppresses `__match_args__` generation",
                        access.object_name
                    ),
                    access.span,
                    &module.path,
                    Some(
                        "Remove `match_args=False` or do not access `__match_args__`".to_owned(),
                    ),
                    Some(
                        "PEP 634: `__match_args__` is only generated when `match_args=True` \
                         (the default)"
                            .to_owned(),
                    ),
                ));
            }
        }
    }
}
