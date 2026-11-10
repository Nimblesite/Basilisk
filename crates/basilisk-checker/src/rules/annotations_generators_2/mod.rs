//! `annotations_generators_2`: Generator yield/send/return type mismatch.
//!
//! INERT. This rule never looked at a `yield` statement. It located each
//! function body by searching `module.source` for the first `:` after the
//! `def`, measured indentation in bytes, then walked lines until it met one
//! whose characters began with `def `, `class `, `async def ` or `@`, and
//! called the span between the two a body. It then ran a hand-rolled lexer over
//! those bytes — skipping quotes and `#` by hand — looking for the five
//! characters `yield`, followed by the five characters ` from`, and sliced the
//! yielded expression out as text to compare against a return annotation that
//! was itself sliced out of the source as text.
//!
//! That is scanning Python source for language vocabulary, which the project's
//! first standing rule forbids outright: recognition is a question about the
//! AST, never about the characters at the use site. Ruff has already parsed
//! this module; every `yield` and `yield from` is a typed node with an exact
//! span. Re-lexing the text by hand reproduced the parser badly — a `yield`
//! inside a nested function, a decorator between two defs, or a body written on
//! the `def` line all changed the verdict, as did reformatting alone.
//!
//! `annotation.rs`, `type_check.rs` and `yield_scan.rs` have been deleted. The
//! rule detects nothing until generator checking is rebuilt on the AST.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

pub(crate) const CODE: ErrorCode = ErrorCode {
    code: "annotations_generators_2",
    docs_url: "https://www.basilisk-python.dev/errors/annotations_generators_2",
};

/// Registered but inert: generator yield/send/return checking awaits a lawful
/// recognition mechanism.
pub(crate) struct GeneratorTypeMismatch;

impl Rule for GeneratorTypeMismatch {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
