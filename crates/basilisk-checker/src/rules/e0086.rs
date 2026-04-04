//! BSK-E0086: Multiple `TypeVarTuple` unpacks in generic or tuple type.
//!
//! Only a single `TypeVarTuple` unpack (`*Ts`) may appear in a type parameter
//! list or in a `tuple[...]` type expression.
//!
//! ```python
//! # BAD — multiple TypeVarTuples in class
//! class Array3(Generic[*Ts1, *Ts2]):  # E
//!     ...
//!
//! # BAD — multiple unpacks in tuple type
//! TA5 = tuple[T1, *Ts, T2, *Ts]  # E
//! TA6 = tuple[T1, *Ts, T2, *tuple[int, ...]]  # E
//!
//! # GOOD
//! class Array(Generic[*Ts]): ...
//! TA1 = tuple[*Ts, T1, T2]  # OK — single unpack
//! ```

use std::collections::HashSet;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0086",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0086",
};

fn make_diag(message: String, span: Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message,
        span,
        path: path.to_owned(),
        help: Some(
            "A `tuple[...]` type may contain at most one unpacked `TypeVarTuple` (`*Ts`)"
                .to_owned(),
        ),
        note: Some(
            "PEP 646: only a single TypeVarTuple is permitted per generic or tuple type".to_owned(),
        ),
        provenance: None,
    }
}

/// Count the number of starred (unpack) elements in a `tuple[...]` subscript string.
///
/// Splits by `,` at depth-0 (not inside nested brackets) and counts elements
/// whose trimmed text starts with `*`.
fn count_unpacks_in_tuple_subscript(subscript: &str) -> usize {
    let mut count = 0usize;
    let mut depth = 0u32;
    let mut element_start = 0usize;

    for (idx, ch) in subscript.char_indices() {
        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let element = subscript[element_start..idx].trim();
                if element.starts_with('*') {
                    count += 1;
                }
                element_start = idx + 1;
            }
            _ => {}
        }
    }
    // Last element after final comma (or the only element).
    let last = subscript[element_start..].trim();
    if last.starts_with('*') {
        count += 1;
    }
    count
}

/// Find the index of the matching `]` for content starting right after `[`.
fn find_matching_bracket(content: &str) -> Option<usize> {
    let mut depth = 0u32;
    for (idx, ch) in content.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                if depth == 0 {
                    return Some(idx);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

/// Emits BSK-E0086 when multiple `TypeVarTuples` are used in a generic or
/// multiple unpacks appear in a `tuple[...]` type expression.
pub(crate) struct MultipleTypeVarTuplesInGeneric;

impl Rule for MultipleTypeVarTuplesInGeneric {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // 1. Check class generic parameters.
        for cls in &module.classes {
            let tvt_count = cls
                .generic_params
                .iter()
                .filter(|p| p.is_typevartuple)
                .count();
            if tvt_count >= 2 {
                diagnostics.push(make_diag(
                    format!(
                        "Class `{}` has {tvt_count} `TypeVarTuple`s in its generic parameters; \
                         only one is allowed",
                        cls.name
                    ),
                    cls.name_span,
                    &module.path,
                ));
            }
        }

        // 2. Check tuple type alias expressions for multiple unpacks.
        check_tuple_type_multiple_unpacks(module, diagnostics);
    }
}

/// Scan module-level type alias definitions for `tuple[..., *X, ..., *Y, ...]`
/// patterns that contain multiple unpack operators.
fn check_tuple_type_multiple_unpacks(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    // Collect TypeVarTuple names so we know which names are TVTs.
    let tvt_names: HashSet<&str> = module
        .typevar_calls
        .iter()
        .filter(|tv| tv.is_typevartuple)
        .map(|tv| tv.name.as_str())
        .collect();

    if tvt_names.is_empty() {
        return;
    }

    // Check type alias definitions whose RHS is a `tuple[...]`.
    for alias in &module.type_alias_defs {
        let rhs_base = alias.rhs_base_name.as_deref().unwrap_or("");
        if rhs_base != "tuple" {
            continue;
        }

        // Get the source text for the alias span and find `tuple[...]`.
        let Some(source_text) = slice_span(&module.source, alias.span) else {
            continue;
        };

        let Some(tuple_start) = source_text.find("tuple[") else {
            continue;
        };
        let inner_start = tuple_start + "tuple[".len();
        let inner = &source_text[inner_start..];
        let Some(close) = find_matching_bracket(inner) else {
            continue;
        };
        let subscript = &inner[..close];

        let unpack_count = count_unpacks_in_tuple_subscript(subscript);
        if unpack_count >= 2 {
            diagnostics.push(make_diag(
                format!(
                    "Tuple type alias `{}` has {unpack_count} unpack operators; \
                     at most one `*` unpack is allowed in a `tuple[...]` type",
                    alias.name
                ),
                alias.span,
                &module.path,
            ));
        }
    }
}
