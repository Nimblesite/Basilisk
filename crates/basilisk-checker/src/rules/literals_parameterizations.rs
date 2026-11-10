//! Implements [`literals_parameterizations`] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
//! `literals_parameterizations`: Invalid `Literal` parameterization.
//!
//! PEP 586 restricts what values may appear inside `Literal[...]`.
//! Only these are legal:
//!   - Integer literals (decimal, hex, binary, octal; optionally signed)
//!   - String literals (`str` and `bytes`)
//!   - Boolean literals (`True`, `False`)
//!   - `None`
//!   - Enum member access (`Color.RED`)
//!   - Nested `Literal[...]`
//!
//! Everything else is illegal, including:
//!   - Arithmetic / unary expressions (`3 + 4`, `~5`, `not False`)
//!   - Function calls (`"foo".replace(...)`)
//!   - Containers (`(1, 2)`, `{"a": "b"}`)
//!   - Type objects, `TypeVar`s, `Any` (`Literal[int]`, `Literal[T]`)
//!   - Float literals (`3.14`)
//!   - Ellipsis (`...`)
//!   - Bare `Literal` with no arguments
//!   - Variables and function objects

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

#[expect(
    dead_code,
    reason = "rule is registered but INERT: its text-matched verdict path was deleted under [ASTREBUILD-LAW] and no diagnostic is emitted until the semantic rebuild ([ASTREBUILD-PHASE-RESOLVER])"
)]
const CODE: ErrorCode = ErrorCode {
    code: "literals_parameterizations",
    docs_url: "https://www.basilisk-python.dev/errors/literals_parameterizations",
};

/// Emits `literals_parameterizations` for invalid `Literal[...]` parameterizations.
pub(crate) struct InvalidLiteralParam;

impl Rule for InvalidLiteralParam {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
