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
//!
//! # Coverage
//!
//! CLASSES ONLY. The PEP's rule covers functions too — `def foo[T](x: K)`
//! with an outer `K = TypeVar("K")` is the same error — and this rule never
//! looks at a function. The check below iterates `module.classes` and filters
//! on `has_pep695_type_params`; there is no function counterpart, and none is
//! stubbed out, so the omission is silent.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "generics_syntax_compatibility",
    docs_url: "https://www.basilisk-python.dev/errors/generics_syntax_compatibility",
};

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

// ##########################################################################
// # `is_word_boundary_match` IS GONE. DO NOT RECREATE IT.                  #
// #                                                                       #
// # It was a HAND-WRITTEN REGEX over Python source bytes: it slid a window #
// # across `haystack.as_bytes()`, matched `needle` byte-for-byte, then     #
// # checked the neighbouring bytes for `is_ascii_alphanumeric() || b'_'`   #
// # to fake an identifier boundary. CLAUDE.md forbids this outright —      #
// # "Any regex over Python source" and "Never parse with strings or        #
// # regex". It also could not be right: it matched inside string literals, #
// # comments, and attribute paths, and its ASCII-only boundary test splits #
// # non-ASCII identifiers, which Python permits.                           #
// #                                                                       #
// # The question it was faking — "does this base expression reference that #
// # TypeVar?" — is answered by the AST: the resolver records               #
// # `ClassInfo::base_name_value_sites`, every name inside a base           #
// # expression resolved through the binding table to the value it denotes. #
// #                                                                       #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs             #
// ##########################################################################

/// Emits `generics_syntax_compatibility` when PEP 695 syntax is mixed with traditional `TypeVars`.
pub(crate) struct Pep695TraditionalTypeVarMix;

// ##########################################################################
// # REBUILT ON RESOLVED VALUE IDENTITY.                                    #
// #                                                                       #
// # `ClassInfo::base_expression_names` is a `Vec<String>` of RENDERED      #
// # simple names harvested from base-class expressions. The deleted body   #
// # matched those strings against a set of TypeVar names collected the     #
// # same way, so:                                                          #
// #                                                                       #
// #   T = TypeVar("T")                                                     #
// #   Alias = T                                                            #
// #   class Foo(Generic[Alias]): ...      # TypeVar NOT recognised         #
// #                                                                       #
// #   class T: ...                        # unrelated class                #
// #   class Foo(Base[T]): ...             # treated as a TypeVar use       #
// #                                                                       #
// # Whether a base-expression name denotes a TypeVar is a question about   #
// # the binding it resolves to, not about the characters written.          #
// #                                                                       #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs             #
// ##########################################################################
impl Rule for Pep695TraditionalTypeVarMix {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // A `TypeVar(...)` call is identified by the span of the call
        // expression itself, which is exactly what an assignment binds.
        let typevar_sites: std::collections::HashMap<basilisk_resolver::Span, &str> = module
            .typevar_calls
            .iter()
            .map(|tv| (tv.span, tv.name.as_str()))
            .collect();
        if typevar_sites.is_empty() {
            return;
        }

        for class in &module.classes {
            if !class.has_pep695_type_params {
                continue;
            }
            for (reference, value_site) in &class.base_name_value_sites {
                let Some(typevar_name) = typevar_sites.get(value_site) else {
                    continue;
                };
                diagnostics.push(make_diagnostic(
                    format!(
                        "Class `{}` uses PEP 695 type parameter syntax but its bases \
                         reference the traditional TypeVar `{typevar_name}`",
                        class.name
                    ),
                    *reference,
                    &module.path,
                ));
            }
        }
    }
}
