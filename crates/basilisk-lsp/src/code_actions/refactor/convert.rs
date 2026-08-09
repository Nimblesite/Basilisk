//! Implements [LSPARCH-FEATURES-CODEACTIONS]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-CODEACTIONS
//!
//! Type-syntax conversion refactors: `Union[X, Y]` ⇄ `X | Y` (PEP 604) and
//! `Optional[X]` ⇄ `X | None`.
//!
//! INERT. The previous implementation recognised `Union`/`Optional` by
//! scanning annotation source text (bracket matching, comma splitting) and
//! was deleted under [ASTREBUILD-LAW]. The features are registered in the
//! dispatch and offer nothing until they are rebuilt on the parsed AST with
//! binding resolution — `typing.Union`, an aliased import, and `Optional`
//! must be recognised by what they resolve to, and the replacement text
//! synthesised from AST nodes ([ASTREBUILD-PHASE-RESOLVER]). Their unit
//! tests remain and FAIL — that is the accurate map of the missing rebuild.

use tower_lsp::lsp_types::{CodeAction, Range, Url};

/// `Union[X, Y]` ⇄ `X | Y` conversion actions at the cursor. Inert pending
/// the AST rebuild; never offers an action.
pub(in crate::code_actions) fn convert_union_syntax(
    _uri: &Url,
    _source: &str,
    _range: &Range,
) -> Vec<CodeAction> {
    Vec::new()
}

/// `Optional[X]` → `X | None` conversion actions at the cursor. Inert
/// pending the AST rebuild; never offers an action.
pub(in crate::code_actions) fn convert_optional_syntax(
    _uri: &Url,
    _source: &str,
    _range: &Range,
) -> Vec<CodeAction> {
    Vec::new()
}
