//! Implements [`generics_typevartuple_specialization`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `generics_typevartuple_specialization`: Multiple `TypeVarTuple` unpacks in generic or tuple type.
//!
//! Only a single `TypeVarTuple` unpack (`*Ts`) may appear in a type parameter
//! list or in a `tuple[...]` type expression.
//!
//! ```python
//! # BAD — multiple TypeVarTuples in class
//! class Array3(Generic[*Ts1, *Ts2]):  # E
//!     ...
//!
//! # BAD — multiple unpacks in tuple type
//! TA5 = tuple[T1, *Ts, T2, *Ts]  # E
//! TA6 = tuple[T1, *Ts, T2, *tuple[int, ...]]  # E
//!
//! # GOOD
//! class Array(Generic[*Ts]): ...
//! TA1 = tuple[*Ts, T1, T2]  # OK — single unpack
//! ```

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "generics_typevartuple_specialization",
    docs_url: "https://www.basilisk-python.dev/errors/generics_typevartuple_specialization",
};

fn make_diag(message: String, span: Span, path: &str) -> Diagnostic {
    error_diagnostic(
        CODE.clone(),
        message,
        span,
        path,
        Some("A `tuple[...]` type may contain at most one unpacked `TypeVarTuple` (`*Ts`)"),
        Some("PEP 646: only a single TypeVarTuple is permitted per generic or tuple type"),
    )
}

/// Emits `generics_typevartuple_specialization` when multiple `TypeVarTuples` are used in a generic or
/// multiple unpacks appear in a `tuple[...]` type expression.
pub(crate) struct MultipleTypeVarTuplesInGeneric;

impl Rule for MultipleTypeVarTuplesInGeneric {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // 1. Check class generic parameters.
        for cls in &module.classes {
            let tvt_count = cls
                .generic_params
                .iter()
                .filter(|p| p.is_typevartuple)
                .count();
            if tvt_count >= 2 {
                diagnostics.push(make_diag(
                    format!(
                        "Class `{}` has {tvt_count} `TypeVarTuple`s in its generic parameters; \
                         only one is allowed",
                        cls.name
                    ),
                    cls.name_span,
                    &module.path,
                ));
            }
        }

        // 2. Check tuple type alias expressions for multiple unpacks.
        check_tuple_type_multiple_unpacks(module, diagnostics);
    }
}

/// Scan module-level type alias definitions for `tuple[..., *X, ..., *Y, ...]`
/// patterns that contain multiple unpack operators.
fn check_tuple_type_multiple_unpacks(_module: &ResolvedModule, _diagnostics: &mut Vec<Diagnostic>) {
    // The former implementation searched source slices for hard-coded `tuple[`
    // text and counted stars by splitting characters. That was illegal and has
    // been deleted. This panic is mandatory until resolved subscript and
    // starred-expression AST nodes implement the rule.
    panic!(
        "generics_typevartuple_specialization: tuple-unpack validation has no legal AST implementation"
    );
}
