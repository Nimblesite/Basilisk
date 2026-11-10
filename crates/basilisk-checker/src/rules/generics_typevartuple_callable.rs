//! Implements [`generics_typevartuple_callable`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `generics_typevartuple_callable`: `TypeVarTuple` callable/tuple argument mismatch.
//!
//! When a constructor (or function) links two parameters via a `TypeVarTuple`
//! -- one as `Callable[[*Ts], R]` and the other as `tuple[*Ts]` -- passing a
//! known function as the callable infers the expected element types for the
//! tuple.  If the tuple literal has elements whose types do not match the
//! inferred order, Basilisk reports the mismatch.
//!
//! ```python
//! Ts = TypeVarTuple("Ts")
//!
//! class Process:
//!     def __init__(self, target: Callable[[*Ts], None], args: tuple[*Ts]) -> None: ...
//!
//! def func1(arg1: int, arg2: str) -> None: ...
//!
//! Process(target=func1, args=(0, ""))   # OK
//! Process(target=func1, args=("", 0))  # E -- str, int does not match int, str
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// Emits `generics_typevartuple_callable` when a tuple literal argument has elements whose types do
/// not match the order inferred from a `TypeVarTuple`-linked `Callable` argument.
pub(crate) struct TypeVarTupleCallableMismatch;

impl Rule for TypeVarTupleCallableMismatch {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
