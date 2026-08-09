//! Implements [`generics_syntax_compatibility`] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
//! `generics_syntax_compatibility`: PEP 695 type parameter syntax mixed with traditional `TypeVars`.
//!
//! PEP 695 introduced a new syntax for declaring type parameters (`class Foo[T]`
//! and `def foo[T]()`). When a class or function uses this new syntax, it must
//! not reference traditional `TypeVar` instances from an outer scope in its
//! base classes or parameter annotations.
//!
//! ```python
//! from typing import TypeVar
//!
//! K = TypeVar("K")
//!
//! class ClassA[V](dict[K, V]):  # E: traditional TypeVar K used with PEP 695 syntax
//!     ...
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

#[expect(
    dead_code,
    reason = "caller deleted for spelling dependence; retained for the rebuild — see \
              tests/string_keyed_class_hierarchy_pin_tests.rs"
)]
const CODE: ErrorCode = ErrorCode {
    code: "generics_syntax_compatibility",
    docs_url: "https://www.basilisk-python.dev/errors/generics_syntax_compatibility",
};

#[expect(
    dead_code,
    reason = "caller deleted for spelling dependence; retained for the rebuild — see \
              tests/string_keyed_class_hierarchy_pin_tests.rs"
)]
fn make_diagnostic(message: String, span: basilisk_resolver::Span, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        message,
        span,
        path,
        Some(
            "When using PEP 695 type parameter syntax, declare all type parameters \
             in the `[...]` list rather than using outer-scope TypeVar instances."
                .to_owned(),
        ),
        Some(
            "PEP 695: traditional TypeVars from outer scope are not allowed in \
             classes/functions that use the new type parameter syntax."
                .to_owned(),
        ),
    )
}

#[expect(
    dead_code,
    reason = "caller deleted for spelling dependence; retained for the rebuild — see \
              tests/string_keyed_class_hierarchy_pin_tests.rs"
)]
// ##########################################################################
// # DELETED BODY — `is_word_boundary_match`. DO NOT RESTORE IT.
// #
// # A HAND-WRITTEN REGEX over Python source bytes: it slid a window across
// # `haystack.as_bytes()`, matched `needle` byte-for-byte, then checked the
// # neighbouring bytes for `is_ascii_alphanumeric() || b'_'` to fake an
// # identifier boundary. CLAUDE.md forbids this outright — "Any regex over
// # Python source" and "Never parse with strings or regex".
// #
// # It also cannot be right: it matched inside string literals, comments, and
// # attribute paths, and its ASCII-only boundary test splits non-ASCII
// # identifiers, which Python permits.
// #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
// ##########################################################################
fn is_word_boundary_match(_haystack: &str, _needle: &str) -> bool {
    panic!(
        "basilisk-checker: `is_word_boundary_match` was DELETED because it was a \
         hand-written regex over Python SOURCE BYTES — a sliding window with an \
         ASCII-only identifier-boundary test. It panics because the real \
         implementation — asking the AST which identifiers a base expression \
         references — DOES NOT EXIST YET. Do not restore the scan."
    )
}

/// Emits `generics_syntax_compatibility` when PEP 695 syntax is mixed with traditional `TypeVars`.
pub(crate) struct Pep695TraditionalTypeVarMix;

impl Rule for Pep695TraditionalTypeVarMix {
    // ##########################################################################
    // # DELETED BODY. DO NOT RESTORE IT AND DO NOT RETURN A DEFAULT.
    // #
    // # `ClassInfo::base_expression_names` is a `Vec<String>` of RENDERED simple
    // # names harvested from base-class expressions. This code matched those
    // # strings against a set of TypeVar names collected the same way, so:
    // #
    // #   T = TypeVar("T")
    // #   Alias = T
    // #   class Foo(Generic[Alias]): ...      # TypeVar NOT recognised
    // #
    // #   class T: ...                        # unrelated class
    // #   class Foo(Base[T]): ...             # treated as a TypeVar use
    // #
    // # Whether a base-expression name denotes a TypeVar is a question about the
    // # binding it resolves to, not about the characters written.
    // #
    // # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
    // ##########################################################################
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
        panic!(
        "basilisk-checker: `generics_syntax_compatibility::check` was DELETED because it matched TypeVar identity by \
         RENDERED NAME against `base_expression_names`, so an aliased TypeVar was \
         invisible and any unrelated symbol spelled like one matched. It panics \
         because the real implementation — base expressions resolved through the \
         binding table — DOES NOT EXIST YET. Do not restore the name matching and \
         do not substitute a default answer."
    )
    }
}
