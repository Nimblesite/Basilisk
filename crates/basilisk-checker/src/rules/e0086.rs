//! BSK-E0086: Multiple `TypeVarTuple` declarations in generic.
//!
//! Only a single `TypeVarTuple` may appear in a type parameter list.
//! Using multiple `TypeVarTuple` declarations is invalid.
//!
//! ```python
//! # BAD
//! Ts1 = TypeVarTuple("Ts1")
//! Ts2 = TypeVarTuple("Ts2")
//! class Array3(Generic[*Ts1, *Ts2]):  # E: multiple TypeVarTuples not allowed
//!     ...
//!
//! # GOOD
//! class Array(Generic[*Ts]):  # OK
//!     ...
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0086",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0086",
};

/// Emits BSK-E0086 when multiple `TypeVarTuples` are used in a generic.
pub(crate) struct MultipleTypeVarTuplesInGeneric;

impl Rule for MultipleTypeVarTuplesInGeneric {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for cls in &module.classes {
            let tvt_count = cls
                .generic_params
                .iter()
                .filter(|p| p.is_typevartuple)
                .count();
            if tvt_count >= 2 {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Class `{}` has {tvt_count} `TypeVarTuple`s in its generic parameters; \
                         only one is allowed",
                        cls.name
                    ),
                    span: cls.name_span,
                    path: module.path.clone(),
                    help: Some(
                        "A generic class may contain at most one `TypeVarTuple` (`*Ts`)".to_owned(),
                    ),
                    note: Some(
                        "PEP 646: only a single TypeVarTuple is permitted per generic".to_owned(),
                    ),
                });
            }
        }
    }
}
