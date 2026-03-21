//! BSK-E0073: `NamedTuple`-to-tuple type incompatibility.
//!
//! When a `NamedTuple` instance is assigned to a variable annotated with a
//! fixed-length `tuple[...]` type, Basilisk verifies:
//!
//! 1. The element count matches the number of fields in the `NamedTuple`.
//! 2. Each element type in the tuple annotation is compatible with the
//!    corresponding `NamedTuple` field type (with covariance).
//!
//! ```python
//! class Point(NamedTuple):
//!     x: int
//!     y: int
//!     units: str = "meters"
//!
//! p = Point(x=1, y=2, units="inches")
//! v1: tuple[int, int, str] = p  # OK
//! v2: tuple[int, int] = p       # E -- too few elements (2 vs 3 fields)
//! v3: tuple[int, str, str] = p  # E -- incompatible element type
//! ```

use std::collections::HashMap;

use basilisk_resolver::{ResolvedModule, RhsKind, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};
use crate::span_util::slice_span;

use super::Rule;

use crate::rules::shared::is_type_compatible;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0073",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0073",
};

/// Emits BSK-E0073 when a `NamedTuple` instance is assigned to an incompatible
/// fixed-length `tuple[...]` annotation.
pub(crate) struct NamedTupleTupleCompat;

impl Rule for NamedTupleTupleCompat {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        // 1. Collect NamedTuple class definitions: class name -> field types.
        let namedtuple_classes: HashMap<&str, Vec<&str>> = collect_namedtuple_classes(module);
        if namedtuple_classes.is_empty() {
            return;
        }

        // 2. Build map: variable name -> NamedTuple class name.
        let var_to_nt: HashMap<&str, &str> = build_var_to_nt_map(module, &namedtuple_classes);
        if var_to_nt.is_empty() {
            return;
        }

        // 3. Check annotated variables with tuple[...] annotations.
        for var in &module.module_vars {
            check_variable(
                var,
                source,
                path,
                &var_to_nt,
                &namedtuple_classes,
                diagnostics,
            );
        }
    }
}

/// Collect `NamedTuple` class definitions: class name -> field type texts.
fn collect_namedtuple_classes(module: &ResolvedModule) -> HashMap<&str, Vec<&str>> {
    module
        .classes
        .iter()
        .filter(|cls| cls.bases.iter().any(|b| b == "NamedTuple"))
        .map(|cls| {
            let field_types: Vec<&str> = cls
                .attributes
                .iter()
                .filter(|attr| attr.has_annotation)
                .filter_map(|attr| {
                    let ann_span = attr.annotation_span?;
                    slice_span(&module.source, ann_span)
                })
                .collect();
            (cls.name.as_str(), field_types)
        })
        .collect()
}

/// Build map: variable name -> `NamedTuple` class name from constructor calls.
fn build_var_to_nt_map<'a>(
    module: &'a ResolvedModule,
    namedtuple_classes: &HashMap<&str, Vec<&str>>,
) -> HashMap<&'a str, &'a str> {
    module
        .module_vars
        .iter()
        .filter(|v| v.rhs_kind == RhsKind::CallExpr)
        .filter_map(|v| {
            let rhs_span = v.rhs_span?;
            let rhs_text = slice_span(&module.source, rhs_span)?;
            let class_name = rhs_text.split('(').next()?.trim();
            if namedtuple_classes.contains_key(class_name) {
                Some((v.name.as_str(), class_name))
            } else {
                None
            }
        })
        .collect()
}

/// Check a single annotated variable for `NamedTuple`-to-tuple incompatibility.
fn check_variable(
    var: &basilisk_resolver::VariableInfo,
    source: &str,
    path: &str,
    var_to_nt: &HashMap<&str, &str>,
    namedtuple_classes: &HashMap<&str, Vec<&str>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !var.has_annotation {
        return;
    }
    let Some(ann_span) = var.annotation_span else {
        return;
    };
    let Some(ann_text) = slice_span(source, ann_span) else {
        return;
    };
    let Some(tuple_element_types) = parse_fixed_tuple_annotation(ann_text) else {
        return;
    };
    let Some(rhs_span) = var.rhs_span else {
        return;
    };
    let Some(rhs_text) = slice_span(source, rhs_span) else {
        return;
    };
    let rhs_name = rhs_text.trim();
    let Some(nt_class_name) = var_to_nt.get(rhs_name) else {
        return;
    };
    let Some(nt_field_types) = namedtuple_classes.get(nt_class_name) else {
        return;
    };

    if tuple_element_types.len() != nt_field_types.len() {
        diagnostics.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Incompatible tuple assignment: `{rhs_name}` is a `{nt_class_name}` \
                 with {} field(s), but `{}` expects {} element(s)",
                nt_field_types.len(),
                ann_text,
                tuple_element_types.len(),
            ),
            span: var.name_span,
            path: path.to_owned(),
            help: Some(format!(
                "A `{nt_class_name}` is a subtype of `tuple[{}]`",
                nt_field_types.join(", "),
            )),
            note: Some(
                "A `NamedTuple` is a subtype of a tuple with matching element count and types"
                    .to_owned(),
            ),
        });
        return;
    }

    check_element_types(
        &tuple_element_types,
        nt_field_types,
        var,
        ann_text,
        rhs_name,
        nt_class_name,
        ann_span,
        path,
        diagnostics,
    );
}

/// Parse a fixed-length tuple annotation like `tuple[int, str, float]`.
///
/// Returns `None` if:
/// - The annotation is not a `tuple[...]` form.
/// - The annotation uses `...` (unbounded tuple like `tuple[int, ...]`).
/// - The annotation is `tuple[Any, ...]`.
fn parse_fixed_tuple_annotation(annotation: &str) -> Option<Vec<&str>> {
    let inner = annotation
        .strip_prefix("tuple[")
        .or_else(|| annotation.strip_prefix("Tuple["))
        .and_then(|s| s.strip_suffix(']'))?;

    // Skip unbounded tuples like `tuple[int, ...]` or `tuple[Any, ...]`.
    if inner.contains("...") {
        return None;
    }

    let elements = split_type_args(inner);
    if elements.is_empty() {
        return None;
    }

    Some(elements)
}

/// Split type arguments respecting bracket nesting.
///
/// E.g. `"int, str, list[int]"` -> `["int", "str", "list[int]"]`.
fn split_type_args(inner: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    for (idx, ch) in inner.char_indices() {
        match ch {
            '[' => depth = depth.saturating_add(1),
            ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                if let Some(part) = inner.get(start..idx) {
                    let part = part.trim();
                    if !part.is_empty() {
                        parts.push(part);
                    }
                }
                start = idx + 1;
            }
            _ => {}
        }
    }
    if let Some(remainder) = inner.get(start..) {
        let remainder = remainder.trim();
        if !remainder.is_empty() {
            parts.push(remainder);
        }
    }
    parts
}

/// Check each element type in the tuple annotation against the `NamedTuple` field type.
#[expect(
    clippy::too_many_arguments,
    reason = "diagnostic context requires many parameters"
)]
fn check_element_types(
    tuple_types: &[&str],
    nt_field_types: &[&str],
    var: &basilisk_resolver::VariableInfo,
    ann_text: &str,
    rhs_name: &str,
    nt_class_name: &str,
    _ann_span: Span,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (idx, (tuple_ty, nt_ty)) in tuple_types.iter().zip(nt_field_types.iter()).enumerate() {
        if !is_type_compatible(nt_ty, tuple_ty) {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Incompatible tuple element type at index {idx}: `{nt_class_name}` \
                     field type is `{nt_ty}`, but `{ann_text}` expects `{tuple_ty}`",
                ),
                span: var.name_span,
                path: path.to_owned(),
                help: Some(format!(
                    "Change element {idx} from `{tuple_ty}` to `{nt_ty}` \
                     or a compatible supertype of `{nt_ty}`",
                )),
                note: Some(format!(
                    "`{rhs_name}` is a `{nt_class_name}` instance; named tuples are \
                     covariant in their field types",
                )),
            });
        }
    }
}
