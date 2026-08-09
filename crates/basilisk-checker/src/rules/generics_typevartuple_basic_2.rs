//! Implements [`generics_typevartuple_basic_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `generics_typevartuple_basic_2`: `TypeVarTuple` must be unpacked with `*` operator.
//!
//! When a `TypeVarTuple` is used in a generic class base list or as a direct
//! type annotation, it must be unpacked using the `*` operator.  Using a
//! `TypeVarTuple` without unpacking is invalid per PEP 646.
//!
//! ```python
//! from typing import Generic, TypeVarTuple
//!
//! Ts = TypeVarTuple("Ts")
//!
//! # BAD
//! class Cls(Generic[Ts]):  # E: TypeVarTuple must be unpacked with *
//!     ...
//!
//! def f(*args: Ts) -> None:  # E: TypeVarTuple must be unpacked with *
//!     ...
//!
//! # GOOD
//! class Cls2(Generic[*Ts]):  # OK
//!     ...
//!
//! def f2(*args: *Ts) -> None:  # OK
//!     ...
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "generics_typevartuple_basic_2",
    docs_url: "https://www.basilisk-python.dev/errors/generics_typevartuple_basic_2",
};

fn make_diag(msg: String, span: basilisk_resolver::Span, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        msg,
        span,
        path,
        Some("Unpack the `TypeVarTuple` with `*`, e.g. `Generic[*Ts]` or `*args: *Ts`".to_owned()),
        Some("PEP 646: TypeVarTuple must always be used with the `*` unpack operator".to_owned()),
    )
}

/// Emits `generics_typevartuple_basic_2` when a `TypeVarTuple` is used without unpacking.
pub(crate) struct TypeVarTupleUnpackRequired;

impl Rule for TypeVarTupleUnpackRequired {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Collect all TypeVarTuple names defined in this module.
        let tvt_names = super::shared::typevar_tuple_names(&module.typevar_calls);

        if tvt_names.is_empty() {
            return;
        }

        let path = &module.path;

        // Check class generic parameters: if a class uses a TypeVarTuple name in its
        // generic parameter list without the `*` unpack, it's an error.
        for cls in &module.classes {
            for param in &cls.generic_params {
                if !param.is_typevartuple && tvt_names.contains(param.name.as_str()) {
                    diagnostics.push(make_diag(
                        format!(
                            "`TypeVarTuple` `{}` must be unpacked with `*` in generic parameter list",
                            param.name
                        ),
                        param.span,
                        path,
                    ));
                }
            }
        }

        // The former implementation parsed rendered annotation text. That was
        // illegal and has been deleted. This panic is mandatory until this path
        // is rebuilt from resolved annotation AST nodes and canonical symbols.
        panic!(
            "generics_typevartuple_basic_2: annotation validation has no legal AST implementation"
        );
    }
}
