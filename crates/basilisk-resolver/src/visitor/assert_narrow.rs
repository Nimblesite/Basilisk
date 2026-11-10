//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE

use ruff_python_ast::Stmt;

use crate::scope::AssertTypeCallInfo;

/// `assert_type` narrowing census.
///
/// Empty on purpose: the previous collector compared rendered annotation
/// strings, which was deleted as spelling logic. The rule downstream is INERT
/// until the type-expression layer answers this from resolved types
/// ([ASTREBUILD-PHASE-TYPEEXPR]).
pub(super) fn collect(_stmts: &[Stmt], _source: &str) -> Vec<AssertTypeCallInfo> {
    Vec::new()
}
