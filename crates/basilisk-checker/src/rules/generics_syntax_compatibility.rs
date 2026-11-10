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

use std::collections::HashSet;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

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

/// Returns `true` when `needle` appears as a whole identifier in `haystack`.
fn is_word_boundary_match(haystack: &str, needle: &str) -> bool {
    let needle_bytes = needle.as_bytes();
    let haystack_bytes = haystack.as_bytes();
    let needle_len = needle_bytes.len();

    if needle_len > haystack_bytes.len() {
        return false;
    }

    haystack_bytes
        .windows(needle_len)
        .enumerate()
        .any(|(i, window)| {
            if window != needle_bytes {
                return false;
            }
            let before_ok = i == 0
                || !haystack_bytes
                    .get(i - 1)
                    .is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'_');
            let after_ok = i + needle_len >= haystack_bytes.len()
                || !haystack_bytes
                    .get(i + needle_len)
                    .is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'_');
            before_ok && after_ok
        })
}

/// Emits `generics_syntax_compatibility` when PEP 695 syntax is mixed with traditional `TypeVars`.
pub(crate) struct Pep695TraditionalTypeVarMix;

impl Rule for Pep695TraditionalTypeVarMix {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Build a set of all module-level traditional TypeVar names.
        let traditional_typevars: HashSet<&str> =
            basilisk_resolver::collect_name_set(&module.typevar_calls);

        // Check classes: if a class uses PEP 695 syntax and references a traditional
        // TypeVar in its base expressions that is NOT one of its own PEP 695 params.
        for cls in &module.classes {
            if !cls.has_pep695_type_params {
                continue;
            }

            if traditional_typevars.is_empty() {
                continue;
            }

            let own_params: HashSet<&str> = cls
                .pep695_type_param_names
                .iter()
                .map(String::as_str)
                .collect();

            let offending: Vec<&str> = cls
                .base_expression_names
                .iter()
                .filter(|name| {
                    traditional_typevars.contains(name.as_str())
                        && !own_params.contains(name.as_str())
                })
                .map(String::as_str)
                .collect();

            for typevar_name in offending {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Class `{}` uses PEP 695 type parameter syntax but references \
                         traditional TypeVar `{}` from outer scope",
                        cls.name, typevar_name
                    ),
                    cls.name_span,
                    &module.path,
                ));
            }
        }

        // Check functions/methods: if a function uses PEP 695 syntax and references a
        // traditional TypeVar in its annotations that is NOT one of its own PEP 695 params.
        for func in &module.functions {
            if !func.has_pep695_type_params {
                continue;
            }
            let own_params: HashSet<&str> = func
                .pep695_type_param_names
                .iter()
                .map(String::as_str)
                .collect();

            // Collect annotation texts to scan.
            let annotation_texts: Vec<&str> = func
                .parameters
                .iter()
                .filter_map(|p| {
                    p.annotation_span
                        .and_then(|s| slice_span(&module.source, s))
                })
                .chain(
                    func.return_annotation_span
                        .and_then(|s| slice_span(&module.source, s)),
                )
                .collect();

            for typevar_name in &traditional_typevars {
                if own_params.contains(typevar_name) {
                    continue;
                }
                let appears = annotation_texts
                    .iter()
                    .any(|text| is_word_boundary_match(text, typevar_name));

                if appears {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "Function `{}` uses PEP 695 type parameter syntax but references \
                             traditional TypeVar `{}` from outer scope",
                            func.name, typevar_name
                        ),
                        func.name_span,
                        &module.path,
                    ));
                    // One diagnostic per function is sufficient.
                    break;
                }
            }
        }
    }
}
