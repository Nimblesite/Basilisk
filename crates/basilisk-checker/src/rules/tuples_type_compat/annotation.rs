//! Implements [`tuples_type_compat`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Tuple annotation modelling and compatibility helpers for `tuples_type_compat`.
//!
//! The tuple model is built from the parsed `ruff` AST
//! ([LINESCANPLAN-AST-MIGRATION], issue #408): starred unpacks are
//! `Expr::Starred` nodes and element types are expression nodes rendered
//! canonically — never `*tuple[` substring hits or comma-splitting of source
//! text.

use ruff_python_ast::{Expr, Number};

use crate::annotation::AnnotationResolver;
use crate::rules::shared::ann_str;
use crate::rules::shared::typing_form::{denotes, subscript_args};

// ---------------------------------------------------------------------------
// Parsed tuple annotation representation
// ---------------------------------------------------------------------------

/// A structured tuple type annotation.
#[derive(Debug)]
pub(super) enum TupleShape {
    /// `tuple[T1, T2, ..., Tn]` — fully fixed length.
    Fixed { count: usize },
    /// `tuple[T, ...]` — homogeneous unbounded.
    Homogeneous { element_type: String },
    /// Mixed form with a starred unpack:
    /// `tuple[P.., *tuple[M, ...], S..]` (`has_unbounded=true`) or
    /// `tuple[P.., *tuple[F..], S..]` (fixed unpack, flattened into the prefix).
    Mixed {
        fixed_prefix: usize,
        fixed_suffix: usize,
        has_unbounded: bool,
        prefix_types: Vec<String>,
        suffix_types: Vec<String>,
        middle_type: Option<String>,
    },
}

/// Is this annotation a `tuple[...]` subscript (builtin `tuple` or
/// `typing.Tuple` under any spelling)?
fn tuple_subscript<'e>(resolver: &AnnotationResolver<'_>, expr: &'e Expr) -> Option<Vec<&'e Expr>> {
    let Expr::Subscript(subscript) = expr else {
        return None;
    };
    let is_tuple = matches!(subscript.value.as_ref(), Expr::Name(name) if name.id.as_str() == "tuple")
        || denotes(resolver, &subscript.value, "Tuple");
    is_tuple.then(|| subscript_args(&subscript.slice))
}

/// Parse a tuple annotation node into a structured shape.
///
/// Returns `None` for non-tuple annotations and forms the model does not
/// cover (e.g. a `*Ts` `TypeVarTuple` unpack).
pub(super) fn parse_tuple_shape(
    resolver: &AnnotationResolver<'_>,
    expr: &Expr,
) -> Option<TupleShape> {
    let args = tuple_subscript(resolver, expr)?;

    // Empty tuple: `tuple[()]`.
    if let [Expr::Tuple(inner)] = args.as_slice() {
        if inner.elts.is_empty() {
            return Some(TupleShape::Fixed { count: 0 });
        }
    }

    // Homogeneous unbounded: `tuple[T, ...]`.
    if let [element, Expr::EllipsisLiteral(_)] = args.as_slice() {
        return Some(TupleShape::Homogeneous {
            element_type: ann_str(element),
        });
    }

    let star_idx = args.iter().position(|arg| matches!(arg, Expr::Starred(_)));
    let Some(star_idx) = star_idx else {
        return Some(TupleShape::Fixed { count: args.len() });
    };

    // The starred component must itself be a tuple form.
    let Expr::Starred(starred) = args.get(star_idx)? else {
        return None;
    };
    let unpack_args = tuple_subscript(resolver, &starred.value)?;

    let mut prefix_types: Vec<String> = args.get(..star_idx)?.iter().map(|e| ann_str(e)).collect();
    let suffix_types: Vec<String> = args
        .get(star_idx + 1..)?
        .iter()
        .map(|e| ann_str(e))
        .collect();
    let fixed_suffix = suffix_types.len();

    // `*tuple[M, ...]` — unbounded middle.
    if let [middle, Expr::EllipsisLiteral(_)] = unpack_args.as_slice() {
        return Some(TupleShape::Mixed {
            fixed_prefix: prefix_types.len(),
            fixed_suffix,
            has_unbounded: true,
            prefix_types,
            suffix_types,
            middle_type: Some(ann_str(middle)),
        });
    }

    // `*tuple[()]` — empty fixed unpack.
    let is_empty_unpack = matches!(
        unpack_args.as_slice(),
        [Expr::Tuple(inner)] if inner.elts.is_empty()
    );
    if !is_empty_unpack {
        // `*tuple[T1, T2]` — fixed unpack: its elements join the prefix.
        prefix_types.extend(unpack_args.iter().map(|e| ann_str(e)));
    }
    Some(TupleShape::Mixed {
        fixed_prefix: prefix_types.len(),
        fixed_suffix,
        has_unbounded: false,
        prefix_types,
        suffix_types,
        middle_type: None,
    })
}

/// Does this tuple annotation contain a starred unpack component?
pub(super) fn has_starred_unpack(resolver: &AnnotationResolver<'_>, expr: &Expr) -> bool {
    tuple_subscript(resolver, expr)
        .is_some_and(|args| args.iter().any(|arg| matches!(arg, Expr::Starred(_))))
}

/// Check whether a source variable's tuple shape is incompatible with the
/// target's shape.
///
/// Handles:
/// - homogeneous `tuple[T, ...]` assigned to a mixed starred form → E
/// - anything possibly longer than a fixed-length target → E
pub(super) fn check_var_against_shape(
    src: &TupleShape,
    target: &TupleShape,
) -> Option<&'static str> {
    match (target, src) {
        (TupleShape::Mixed { .. }, TupleShape::Homogeneous { .. }) => {
            Some("homogeneous unbounded tuple is not assignable to mixed starred-unpack form")
        }
        (TupleShape::Fixed { count: target_len }, src_shape) => {
            let src_may_be_longer = match src_shape {
                TupleShape::Homogeneous { .. } => true,
                TupleShape::Mixed {
                    fixed_prefix,
                    fixed_suffix,
                    has_unbounded,
                    ..
                } => *has_unbounded || (fixed_prefix + fixed_suffix > *target_len),
                TupleShape::Fixed { count: src_len } => src_len > target_len,
            };
            src_may_be_longer.then_some(
                "source tuple type may have more elements than the fixed-length target allows",
            )
        }
        _ => None,
    }
}

/// Check whether a tuple literal's elements are compatible with a tuple shape.
///
/// Returns `Some(message)` when the literal violates the annotation.
pub(super) fn check_literal_against_shape(
    elems: &[&Expr],
    shape: &TupleShape,
) -> Option<&'static str> {
    match shape {
        TupleShape::Fixed { count } => (elems.len() != *count)
            .then_some("tuple literal length does not match fixed starred-unpack annotation"),

        TupleShape::Homogeneous { element_type } => elems
            .iter()
            .any(|elem| !elem_type_compatible(elem, element_type))
            .then_some("tuple literal element type incompatible with homogeneous annotation"),

        TupleShape::Mixed {
            fixed_prefix,
            fixed_suffix,
            has_unbounded,
            prefix_types,
            suffix_types,
            middle_type,
        } => check_literal_against_mixed(
            elems,
            *fixed_prefix,
            *fixed_suffix,
            *has_unbounded,
            prefix_types,
            suffix_types,
            middle_type.as_deref(),
        ),
    }
}

/// Check a tuple literal against a mixed starred-unpack shape
/// like `tuple[int, *tuple[str, ...], int]`.
fn check_literal_against_mixed(
    elems: &[&Expr],
    fixed_prefix: usize,
    fixed_suffix: usize,
    has_unbounded: bool,
    prefix_types: &[String],
    suffix_types: &[String],
    middle_type: Option<&str>,
) -> Option<&'static str> {
    let n = elems.len();
    let min_len = fixed_prefix + fixed_suffix;

    if !has_unbounded {
        // Fixed total length: prefix + suffix (no unbounded middle).
        if n != min_len {
            return Some("tuple literal length does not match fixed starred-unpack annotation");
        }
    } else if n < min_len {
        return Some("tuple literal has too few elements for starred-unpack annotation");
    }

    // Check fixed prefix.
    for (i, prefix_type) in prefix_types.iter().enumerate() {
        if let Some(elem) = elems.get(i) {
            if !elem_type_compatible(elem, prefix_type) {
                return Some("tuple literal element type incompatible with annotation prefix");
            }
        }
    }

    // Check fixed suffix (from the right).
    for (j, suffix_type) in suffix_types.iter().enumerate() {
        let elem_idx = n - fixed_suffix + j;
        if let Some(elem) = elems.get(elem_idx) {
            if !elem_type_compatible(elem, suffix_type) {
                return Some("tuple literal element type incompatible with annotation suffix");
            }
        }
    }

    // Check middle elements against the unbounded type.
    if has_unbounded {
        if let Some(mid_type) = middle_type {
            let middle_start = fixed_prefix;
            let middle_end = n - fixed_suffix;
            for elem in elems.get(middle_start..middle_end).unwrap_or_default() {
                if !elem_type_compatible(elem, mid_type) {
                    return Some(
                        "tuple literal middle element type incompatible with starred-unpack annotation",
                    );
                }
            }
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Type compatibility helpers
// ---------------------------------------------------------------------------

/// The inferred builtin type of a tuple literal element node, when knowable.
fn infer_elem_type(elem: &Expr) -> Option<&'static str> {
    match elem {
        Expr::BooleanLiteral(_) => Some("bool"),
        Expr::NumberLiteral(lit) => match &lit.value {
            Number::Int(_) => Some("int"),
            Number::Float(_) => Some("float"),
            Number::Complex { .. } => Some("complex"),
        },
        Expr::StringLiteral(_) | Expr::FString(_) => Some("str"),
        Expr::BytesLiteral(_) => Some("bytes"),
        Expr::UnaryOp(unary) => infer_elem_type(&unary.operand),
        _ => None,
    }
}

/// Check whether a literal element node is compatible with an annotation type
/// rendering. Unknown element types are conservatively allowed.
pub(super) fn elem_type_compatible(elem: &Expr, ann_type: &str) -> bool {
    let Some(inferred) = infer_elem_type(elem) else {
        return true;
    };
    types_assignable(inferred, ann_type)
}

/// Returns `true` when `src` is assignable to `target` under the numeric
/// tower, with `Any` compatible in both directions.
pub(super) fn types_assignable(src: &str, target: &str) -> bool {
    if src == target || src == "Any" || target == "Any" {
        return true;
    }
    matches!(
        (src, target),
        ("int", "float" | "complex") | ("bool", "int" | "float" | "complex") | ("float", "complex")
    )
}
