//! BSK-E0042: PEP 695 type parameter syntax mixed with traditional `TypeVars`.
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

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0042",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0042",
};

fn make_diagnostic(message: String, span: basilisk_resolver::Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message,
        span,
        path: path.to_owned(),
        help: Some(
            "When using PEP 695 type parameter syntax, declare all type parameters \
             in the `[...]` list rather than using outer-scope TypeVar instances."
                .to_owned(),
        ),
        note: Some(
            "PEP 695: traditional TypeVars from outer scope are not allowed in \
             classes/functions that use the new type parameter syntax."
                .to_owned(),
        ),
    }
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
                || (!haystack_bytes[i - 1].is_ascii_alphanumeric()
                    && haystack_bytes[i - 1] != b'_');
            let after_ok = i + needle_len >= haystack_bytes.len()
                || (!haystack_bytes[i + needle_len].is_ascii_alphanumeric()
                    && haystack_bytes[i + needle_len] != b'_');
            before_ok && after_ok
        })
}

/// Emits BSK-E0042 when PEP 695 syntax is mixed with traditional `TypeVars`.
pub(crate) struct Pep695TraditionalTypeVarMix;

impl Rule for Pep695TraditionalTypeVarMix {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Build a set of all module-level traditional TypeVar names.
        let traditional_typevars: HashSet<&str> = module
            .typevar_calls
            .iter()
            .map(|tv| tv.name.as_str())
            .collect();

        // Check classes: if a class uses PEP 695 syntax and references a traditional
        // TypeVar in its base expressions that is NOT one of its own PEP 695 params.
        // Also check for redundant `Generic[T]` or `Protocol[T]` with PEP 695 syntax.
        for cls in &module.classes {
            if !cls.has_pep695_type_params {
                continue;
            }

            // PEP 695: using Generic[T] or Protocol[T] with bracket syntax is invalid —
            // the class already declares its type params with [T, ...] syntax.
            // Detect this by checking if Generic/Protocol appears in base_expression_names
            // but NOT in bases (meaning it's subscripted, not bare).
            let bases_set: HashSet<&str> = cls.bases.iter().map(String::as_str).collect();
            for generic_name in &["Generic", "Protocol"] {
                let in_expressions = cls
                    .base_expression_names
                    .iter()
                    .any(|n| n.as_str() == *generic_name);
                let in_bases = bases_set.contains(*generic_name);
                if in_expressions && !in_bases {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "Class `{}` uses PEP 695 type parameter syntax but also \
                             inherits from parameterized `{}[...]`; use bare `{}` instead",
                            cls.name, generic_name, generic_name
                        ),
                        cls.name_span,
                        &module.path,
                    ));
                }
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
                        .and_then(|s| module.source.get(s.start as usize..s.end as usize))
                })
                .chain(
                    func.return_annotation_span
                        .and_then(|s| module.source.get(s.start as usize..s.end as usize)),
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
