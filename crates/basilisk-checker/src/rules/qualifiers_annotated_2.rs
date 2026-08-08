//! Implements [`qualifiers_annotated_2`] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
//! `qualifiers_annotated_2`: `Annotated[...]` requires at least two arguments.
//!
//! PEP 593 requires `Annotated` to be subscripted with at least two arguments:
//! a type and one or more metadata values. `Annotated[int]` with only a single
//! argument is a type error.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

#[expect(
    dead_code,
    reason = "rule is registered but INERT: its text-matched verdict path was deleted under [ASTREBUILD-LAW] and no diagnostic is emitted until the semantic rebuild ([ASTREBUILD-PHASE-RESOLVER])"
)]
const CODE: ErrorCode = ErrorCode {
    code: "qualifiers_annotated_2",
    docs_url: "https://www.basilisk-python.dev/errors/qualifiers_annotated_2",
};

/// Emits `qualifiers_annotated_2` when `Annotated[X]` has fewer than two arguments.
pub(crate) struct AnnotatedTooFewArguments;

impl Rule for AnnotatedTooFewArguments {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
