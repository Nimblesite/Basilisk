//! BSK-E0133: Protocol `TypeVar` variance mismatch.
//!
//! When a generic protocol class declares a `TypeVar` as invariant but the
//! inferred variance (from method parameter and return positions) is strictly
//! covariant or contravariant, a diagnostic is emitted recommending the more
//! specific variance.
//!
//! PEP 544 specifies that type checkers should warn when the inferred variance
//! of a type variable used in a protocol differs from its declared variance.
//!
//! ```python
//! from typing import Protocol, TypeVar
//!
//! T = TypeVar("T")  # invariant
//!
//! class MyProto(Protocol[T]):  # E — T should be covariant
//!     def method(self) -> T: ...
//! ```

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0133",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0133",
};

/// Extract text from source at the given span.
fn span_text(source: &str, span: Span) -> Option<&str> {
    source.get(span.start as usize..span.end as usize)
}

/// Check whether `name` appears as a standalone type reference in `text`.
///
/// Matches the type variable name at word boundaries — `T1` does not match
/// inside `T1_co` or `T12`.
fn contains_typevar(text: &str, name: &str) -> bool {
    let name_bytes = name.as_bytes();
    let text_bytes = text.as_bytes();
    let mut start = 0;
    while start + name.len() <= text.len() {
        if let Some(pos) = text[start..].find(name) {
            let abs_pos = start + pos;
            let before_ok = abs_pos == 0 || !is_ident_char(text_bytes[abs_pos - 1]);
            let after_pos = abs_pos + name_bytes.len();
            let after_ok = after_pos >= text_bytes.len() || !is_ident_char(text_bytes[after_pos]);
            if before_ok && after_ok {
                return true;
            }
            start = abs_pos + 1;
        } else {
            break;
        }
    }
    false
}

/// Returns `true` for ASCII alphanumeric or underscore (identifier chars).
const fn is_ident_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Check if the `TypeVar` appears only inside invariant container subscripts
/// in the return type text (e.g. `list[T]`, `dict[K, V]`, `set[T]`).
///
/// Returns `true` when every occurrence of the `TypeVar` in `ret_text` is
/// nested inside `[...]` brackets (i.e. not at the top level of the type).
fn typevar_only_in_subscript(ret_text: &str, tv_name: &str) -> bool {
    let text_bytes = ret_text.as_bytes();
    let mut start = 0;
    let mut found_any = false;
    while start + tv_name.len() <= ret_text.len() {
        let Some(pos) = ret_text[start..].find(tv_name) else {
            break;
        };
        let abs_pos = start + pos;
        let before_ok = abs_pos == 0 || !is_ident_char(text_bytes[abs_pos - 1]);
        let after_pos = abs_pos + tv_name.len();
        let after_ok = after_pos >= text_bytes.len() || !is_ident_char(text_bytes[after_pos]);
        if before_ok && after_ok {
            found_any = true;
            // Check bracket depth at this position.
            let depth = bracket_depth(&ret_text[..abs_pos]);
            if depth == 0 {
                // At top level — not inside a subscript.
                return false;
            }
        }
        start = abs_pos + 1;
    }
    found_any
}

/// Count the nesting depth of `[` vs `]` brackets in the given prefix.
fn bracket_depth(prefix: &str) -> usize {
    let mut depth: usize = 0;
    for byte in prefix.bytes() {
        match byte {
            b'[' => depth = depth.saturating_add(1),
            b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth
}

/// The inferred variance of a `TypeVar` within a protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InferredVariance {
    /// Used only in return types directly (covariant position).
    Covariant,
    /// Used only in parameter types (contravariant position).
    Contravariant,
    /// Used in both positions, in attribute annotations, or inside invariant containers.
    Invariant,
    /// Not used at all in any non-exempt method or attribute.
    Unused,
}

/// Emits BSK-E0133 for protocol `TypeVar` variance mismatches.
pub(crate) struct ProtocolVarianceMismatch;

impl Rule for ProtocolVarianceMismatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Build a map of TypeVar name -> (is_covariant, is_contravariant).
        let typevar_variance: HashMap<&str, (bool, bool)> = module
            .typevar_calls
            .iter()
            .map(|tv| (tv.name.as_str(), (tv.is_covariant, tv.is_contravariant)))
            .collect();

        let source = &module.source;
        let path = &module.path;

        for class in &module.classes {
            if !class.base_expression_names.iter().any(|b| b == "Protocol") {
                continue;
            }

            let methods: Vec<_> = module
                .functions
                .iter()
                .filter(|f| f.class_name.as_deref() == Some(&class.name))
                .collect();

            for param in &class.generic_params {
                let tv_name = &param.name;

                // Skip names that are not traditional TypeVars (e.g. ParamSpec, TypeVarTuple).
                let Some(&(declared_co, declared_contra)) = typevar_variance.get(tv_name.as_str())
                else {
                    continue;
                };

                // Skip TypeVars that already declare explicit variance.
                if declared_co || declared_contra {
                    continue;
                }

                let inferred = infer_variance(class, &methods, tv_name, source);

                let suggested = match inferred {
                    InferredVariance::Covariant | InferredVariance::Unused => "covariant",
                    InferredVariance::Contravariant => "contravariant",
                    InferredVariance::Invariant => continue,
                };

                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "TypeVar `{tv_name}` in protocol `{}` should be {suggested}",
                        class.name,
                    ),
                    span: class.name_span,
                    path: path.clone(),
                    help: Some(format!(
                        "Declare `{tv_name}` with `{suggested}=True` to match its usage in \
                         this protocol"
                    )),
                    note: Some(
                        "PEP 544: type checkers warn when inferred variance differs from \
                         declared variance"
                            .to_owned(),
                    ),
                });
            }
        }
    }
}

/// Infer the variance of a `TypeVar` from its usage in a protocol class.
///
/// Considers method signatures (parameters = contravariant, returns = covariant)
/// and class-body attribute annotations (invariant, since attrs can be read and written).
fn infer_variance(
    class: &ClassInfo,
    methods: &[&basilisk_resolver::FunctionInfo],
    tv_name: &str,
    source: &str,
) -> InferredVariance {
    // If the TypeVar appears in any class attribute annotation, it is invariant.
    if typevar_in_class_attributes(class, tv_name, source) {
        return InferredVariance::Invariant;
    }

    let mut in_covariant = false;
    let mut in_contravariant = false;

    for method in methods {
        // __init__ is exempt from variance calculations per the spec.
        if method.name == "__init__" {
            continue;
        }

        // Check return type (covariant position).
        if let Some(ret_span) = method.return_annotation_span {
            if let Some(ret_text) = span_text(source, ret_span) {
                if contains_typevar(ret_text, tv_name) {
                    // If the TypeVar only appears inside subscript brackets
                    // (e.g. `list[T]`), treat as invariant rather than covariant,
                    // because common containers like `list`, `dict`, `set` are
                    // invariant in their type parameters.
                    if typevar_only_in_subscript(ret_text, tv_name) {
                        return InferredVariance::Invariant;
                    }
                    in_covariant = true;
                }
            }
        }

        // Check parameter types (contravariant position).
        let params_to_check = if method.parameters.is_empty() {
            &method.parameters[..]
        } else {
            let first_name = &method.parameters[0].name;
            if first_name == "self" || first_name == "cls" {
                &method.parameters[1..]
            } else {
                &method.parameters[..]
            }
        };

        for param in params_to_check {
            if let Some(ann_span) = param.annotation_span {
                if let Some(ann_text) = span_text(source, ann_span) {
                    if contains_typevar(ann_text, tv_name) {
                        in_contravariant = true;
                    }
                }
            }
        }
    }

    match (in_covariant, in_contravariant) {
        (true, false) => InferredVariance::Covariant,
        (false, true) => InferredVariance::Contravariant,
        (true, true) => InferredVariance::Invariant,
        (false, false) => InferredVariance::Unused,
    }
}

/// Check if a `TypeVar` name appears in any class-body attribute annotation.
///
/// Protocol attributes (e.g. `x: T | None`) are invariant because they can
/// be both read and written by conforming classes.
fn typevar_in_class_attributes(class: &ClassInfo, tv_name: &str, source: &str) -> bool {
    class.attributes.iter().any(|attr| {
        attr.annotation_span
            .and_then(|sp| span_text(source, sp))
            .is_some_and(|ann_text| contains_typevar(ann_text, tv_name))
    })
}
