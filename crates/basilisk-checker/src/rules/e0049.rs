//! BSK-E0049: Multiple unbounded tuple components in a single tuple type.
//!
//! A `tuple[...]` type annotation may contain at most one unbounded component.
//! An unbounded component is:
//! - `*tuple[T, ...]` — a starred subscript where the inner tuple is variadic
//! - `*Ts` / `*<Name>` — a starred `TypeVarTuple` unpack
//! - `Unpack[tuple[T, ...]]` — the legacy unpack form
//!
//! For example, `tuple[*tuple[str, ...], *tuple[int, ...]]` is invalid because
//! it has two unbounded components.
//!
//! ```python
//! t: tuple[*tuple[str, ...], *tuple[int, ...]]  # E — two unbounded components
//! t: tuple[*tuple[str, ...], *Ts]               # E — two unbounded components
//! t: tuple[*tuple[str, ...], str]               # OK — only one unbounded
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0049",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0049",
};

/// Emits BSK-E0049 when a tuple type annotation has more than one unbounded component.
pub(crate) struct MultipleUnboundedTupleTypes;

impl Rule for MultipleUnboundedTupleTypes {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for &span in &module.multiple_unbounded_tuple_spans {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                "tuple type contains more than one unbounded component".to_owned(),
                span,
                &module.path,
                Some(
                    "Only one `*tuple[T, ...]` or `*Ts` component is allowed per tuple type"
                        .to_owned(),
                ),
                Some("PEP 646: a tuple type may contain at most one unbounded unpack".to_owned()),
            ));
        }
    }
}
