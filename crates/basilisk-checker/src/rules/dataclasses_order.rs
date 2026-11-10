//! Implements [`dataclasses_order`] from [CHKARCH-DIAG-COERCION]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-COERCION
//! `dataclasses_order`: Invalid ordering comparison of dataclass instances.
//!
//! INERT. Every verdict this rule reached came out of `module.source` as text:
//! it split the file into lines, skipped any line whose characters began with
//! `class `, `def `, `@` or `#`, searched the remainder for the literal
//! substrings `" < "`, `" <= "`, `" > "`, `" >= "`, and walked the bytes either
//! side of the hit to guess an identifier. The variable-to-class map came the
//! same way — slicing each assignment's right-hand side out of the source and
//! splitting it on `(` and `[`.
//!
//! That is scanning Python source for language vocabulary, which the project's
//! first standing rule forbids outright: recognition is a question about the
//! AST, never about the characters at the use site. A comparison written across
//! two lines, inside a docstring, or with different spacing changed the verdict;
//! a semantics-preserving reformat did too.
//!
//! The scanner, its identifier heuristics, and the text-derived variable map
//! have been deleted. The rule detects nothing until ordering comparisons are
//! recovered structurally from the AST.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// Registered but inert: ordering comparisons between dataclass instances await
/// a lawful recognition mechanism.
pub(crate) struct CrossTypeDataclassOrderComparison;

impl Rule for CrossTypeDataclassOrderComparison {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
