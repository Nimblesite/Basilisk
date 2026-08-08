//! Implements [`generics_typevartuple_unpack`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `generics_typevartuple_unpack`: `TypeVarTuple` unpack minimum type argument violation.
//!
//! When a function parameter has a type annotation containing a `TypeVarTuple`
//! unpack pattern like `Array[Batch, *tuple[Any, ...], Channels]`, the type has
//! fixed prefix and suffix type arguments around a variadic middle.  Any value
//! passed to that parameter must have at least `prefix_count + suffix_count`
//! type arguments.
//!
//! ```python
//! Ts = TypeVarTuple("Ts")
//!
//! class Array(Generic[*Ts]): ...
//!
//! def process(x: Array[Batch, *tuple[Any, ...], Channels]) -> None: ...
//!
//! def func(z: Array[Batch]):
//!     process(z)  # E -- Array[Batch] has 1 type arg, need at least 2
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

#[expect(
    dead_code,
    reason = "the source-text verdict was deleted under [ASTREBUILD-LAW]; the registered rule remains exposed for an AST-backed implementation"
)]
const CODE: ErrorCode = ErrorCode {
    code: "generics_typevartuple_unpack",
    docs_url: "https://www.basilisk-python.dev/errors/generics_typevartuple_unpack",
};

/// Emits `generics_typevartuple_unpack` when a function-body call passes a value whose generic type
/// does not have enough type arguments to satisfy a `TypeVarTuple` unpack pattern.
pub(crate) struct TypeVarTupleUnpackViolation;

impl Rule for TypeVarTupleUnpackViolation {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
        // ILLEGAL TO RESTORE: this rule previously parsed annotation strings
        // with `contains`, `find`, `trim`, and comma splitting. Rebuild it only
        // from resolved generic arguments, starred AST nodes, and type identity.
    }
}
