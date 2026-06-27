//! Implements [dict_key_hashable] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-typesafety
//! dict_key_hashable: Unhashable type used as a dict key.
//!
//! Lists, sets, and plain dicts are not hashable and cannot be used as
//! dictionary keys at runtime.  Basilisk detects these statically.
//!
//! ```python
//! def bad_key() -> None:
//!     mapping = {[1, 2]: "value"}   # list as key → E0022
//! ```

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "dict_key_hashable",
    docs_url: "https://www.basilisk-python.dev/errors/dict_key_hashable",
};

/// Emits dict_key_hashable for unhashable types used as dictionary keys.
pub(crate) struct UnhashableDictKey;

impl Rule for UnhashableDictKey {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        module
            .functions
            .iter()
            .flat_map(|func| func.unhashable_keys.iter())
            .for_each(|key| {
                diagnostics.push(make_diagnostic(key.span, key.key_type, &module.path));
            });
    }
}

fn make_diagnostic(span: Span, key_type: &str, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!("Unhashable type `{key_type}` used as a dictionary key"),
        span,
        path,
        Some(format!(
            "Convert the `{key_type}` to a hashable type (e.g. `tuple`) before using it as a key"
        )),
        Some(
            "Dictionary keys must be hashable; `list`, `set`, and `dict` are not hashable"
                .to_owned(),
        ),
    )
}
