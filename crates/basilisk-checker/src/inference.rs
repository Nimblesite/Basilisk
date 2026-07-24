//! Implements [TYPEINF-OVERVIEW], [TYPEINF-INFERRED], [TYPEINF-ALGO],
//! [TYPEINF-VARS], [TYPEINF-VARS-SIMPLE], and the shared predicates behind
//! [TYPEINF-REQUIRED] / [TYPEINF-EXCEEDS]. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md.
//! Type inference engine for Basilisk.

use crate::types::InferredType;
use basilisk_resolver::{RhsKind, VariableInfo};

/// Infers the type of a right-hand-side expression.
#[must_use]
pub fn infer_rhs(rhs: &RhsKind) -> InferredType {
    match rhs {
        RhsKind::IntLiteral => InferredType::Int,
        RhsKind::FloatLiteral => InferredType::Float,
        // PEP 675: a literal expression is provably a LiteralString. Plain
        // `str` remains reserved for dynamic string values, preserving that
        // distinction through container inference.
        RhsKind::StrLiteral => InferredType::LiteralString,
        RhsKind::BoolLiteral => InferredType::Bool,
        RhsKind::BytesLiteral => InferredType::Bytes,
        RhsKind::NoneValue => InferredType::None_,
        RhsKind::EmptyList => InferredType::List(Box::new(InferredType::Never)),
        RhsKind::EmptyDict => {
            InferredType::Dict(Box::new(InferredType::Never), Box::new(InferredType::Never))
        }
        RhsKind::List(elements) => crate::collection_inference::infer_list_type(elements),
        RhsKind::Set(elements) => crate::collection_inference::infer_set_type(elements),
        RhsKind::Dict(pairs) => crate::collection_inference::infer_dict_type(pairs),
        RhsKind::Tuple(elements) => crate::collection_inference::infer_tuple_type(elements),
        // `KnownCall` feeds inferred-type *display* (hover, inlay hints — #253);
        // checker semantics deliberately keep call results Unknown, like `CallExpr`.
        RhsKind::CallExpr | RhsKind::KnownCall(_) | RhsKind::TypeCall | RhsKind::Other => {
            InferredType::Unknown
        }
        RhsKind::Lambda => {
            // Lambda expressions have type Callable[..., Unknown] since we don't know
            // parameter types or return type without analyzing the lambda body
            InferredType::Callable(crate::types::CallableInfo {
                param_types: Vec::new(), // Empty means we don't know parameter types
                return_type: Box::new(InferredType::Unknown),
            })
        }
    }
}

/// Checks a freshly-constructed collection *literal* against a declared
/// container type using **covariant, contextual** typing.
///
/// Implements [TYPEINF-SPECIAL-LITERAL-CONTEXT]. A stored value keeps the
/// invariant subtyping of [TYPEINF-SUBTYPING-GENERIC]: `c: list[Never]` is not
/// assignable to `list[int]`, and `specialtypes_never.py` requires that error.
/// But a literal expression has no aliasing, so in a `return`/`yield` context it
/// is typed *against* the expected type — `return []` constructs a `list[bytes]`
/// directly, and `yield {"": 0}` a `dict[str, int]` — rather than first becoming
/// a `list[Never]` / `dict[LiteralString, int]` value and then failing
/// invariance. Each literal element need only be assignable *to* the declared
/// element type.
///
/// Returns `None` when `rhs` is not a collection literal this can judge against
/// `declared`, so callers fall back to the invariant
/// [`InferredType::is_assignable_to`]. Genuine element mismatches (`return [1]`
/// against `list[str]`) still yield `Some(false)`, preserving required errors.
#[must_use]
pub fn literal_collection_assignable_to(rhs: &RhsKind, declared: &InferredType) -> Option<bool> {
    match declared {
        // A literal fits a union/optional iff it fits at least one member.
        InferredType::Union(members) => {
            // A literal fits a union iff it fits at least one member. If any
            // member is UNJUDGEABLE here (`None` — e.g. an `Any`/`object` arm),
            // we cannot definitively reject: defer to the caller's invariant
            // fallback (which accepts via that arm). Only return `Some(false)`
            // when EVERY member was judged and none accepted — otherwise a
            // valid `return [1]` for `list[str] | object` becomes a false
            // positive (the `object` arm parses to `Any`).
            let mut saw_unjudgeable = false;
            for member in members {
                match literal_collection_assignable_to(rhs, member) {
                    Some(true) => return Some(true),
                    Some(false) => {}
                    None => saw_unjudgeable = true,
                }
            }
            if saw_unjudgeable {
                None
            } else {
                Some(false)
            }
        }
        InferredType::Optional(inner) => literal_collection_assignable_to(rhs, inner),
        InferredType::List(elem) => match rhs {
            RhsKind::EmptyList => Some(true),
            RhsKind::List(elements) => Some(
                elements
                    .iter()
                    .all(|e| literal_element_assignable_to(e, elem)),
            ),
            _ => None,
        },
        InferredType::Set(elem) => match rhs {
            RhsKind::Set(elements) => Some(
                elements
                    .iter()
                    .all(|e| literal_element_assignable_to(e, elem)),
            ),
            _ => None,
        },
        InferredType::Dict(key_ty, val_ty) => match rhs {
            RhsKind::EmptyDict => Some(true),
            RhsKind::Dict(pairs) => Some(pairs.iter().all(|(key, value)| {
                literal_element_assignable_to(key, key_ty)
                    && literal_element_assignable_to(value, val_ty)
            })),
            _ => None,
        },
        InferredType::Tuple(declared_elems) => match rhs {
            RhsKind::Tuple(elements) => tuple_literal_assignable_to(elements, declared_elems),
            _ => None,
        },
        _ => None,
    }
}

/// Contextual typing for a tuple display against a declared tuple type: each
/// position carries the declared element type inward, so an empty `[]`/`{}`
/// nested in the tuple constructs the declared container instead of a
/// `list[Never]` / `dict[Never, Never]` that then fails invariance (#337).
///
/// Implements [TYPEINF-SPECIAL-LITERAL-CONTEXT] for the tuple shapes of
/// [TYPEINF-COLLECTIONS-TUPLES]. Returns `None` for shapes this cannot judge —
/// a PEP 646 unpacked segment or an arity mismatch — leaving those to the
/// invariant fallback unchanged.
fn tuple_literal_assignable_to(
    elements: &[RhsKind],
    declared_elems: &[InferredType],
) -> Option<bool> {
    if declared_elems
        .iter()
        .any(crate::types_star_tuples::is_unpacked_tuple_elem)
    {
        return None;
    }
    // `tuple[X, ...]` (PEP 484 homogeneous): every position is typed against `X`.
    if let Some(elem) = crate::types_star_tuples::homogeneous_tuple_elem(declared_elems) {
        return Some(
            elements
                .iter()
                .all(|element| literal_element_assignable_to(element, elem)),
        );
    }
    if elements.len() != declared_elems.len() {
        return None;
    }
    Some(
        elements
            .iter()
            .zip(declared_elems)
            .all(|(element, declared)| literal_element_assignable_to(element, declared)),
    )
}

/// Assignability of a single literal element: a nested collection literal stays
/// covariant/contextual; anything else uses its ordinary inferred type.
fn literal_element_assignable_to(rhs: &RhsKind, declared: &InferredType) -> bool {
    literal_collection_assignable_to(rhs, declared)
        .unwrap_or_else(|| infer_rhs(rhs).is_assignable_to(declared))
}

/// Returns `true` when the CURRENT engine fully determines a usable declared
/// type from this RHS alone — i.e. [`infer_rhs`] produces a type with no
/// `Unknown`/`Never` component and no widening guess.
///
/// Implements [TYPEINF-EXCEEDS-REQUIRED]: a missing-annotation rule
/// (BSK-0001/BSK-0002) must never fire where this returns `true`, and must
/// keep firing where it returns `false`. The predicate is deliberately exactly
/// as strong as today's inference and no stronger:
///
/// - scalar literals (`int`/`float`/`str`/`bool`/`bytes`) determine their type;
/// - non-empty containers of determining elements determine theirs;
/// - `None` does NOT determine a declared type (`T | None` needs `T`);
/// - empty containers do NOT (element types unknown);
/// - calls, lambdas, names, and arbitrary expressions do NOT
///   ([TYPEINF-EXCEEDS-NOUNKNOWN] keeps them `Unknown`).
#[must_use]
pub fn rhs_fully_determines_type(rhs: &RhsKind) -> bool {
    match rhs {
        RhsKind::IntLiteral
        | RhsKind::FloatLiteral
        | RhsKind::StrLiteral
        | RhsKind::BoolLiteral
        | RhsKind::BytesLiteral => true,
        RhsKind::List(elements) | RhsKind::Set(elements) | RhsKind::Tuple(elements) => {
            !elements.is_empty() && elements.iter().all(rhs_fully_determines_type)
        }
        RhsKind::Dict(pairs) => {
            !pairs.is_empty()
                && pairs
                    .iter()
                    .all(|(k, v)| rhs_fully_determines_type(k) && rhs_fully_determines_type(v))
        }
        RhsKind::NoneValue
        | RhsKind::EmptyList
        | RhsKind::EmptyDict
        | RhsKind::CallExpr
        | RhsKind::KnownCall(_)
        | RhsKind::TypeCall
        | RhsKind::Lambda
        | RhsKind::Other => false,
    }
}

/// Checks if a variable assignment is valid given its annotation and inferred RHS type.
///
/// # Errors
///
/// Returns an error if the RHS type cannot be inferred (i.e., it is `Unknown`).
pub fn check_annotated_variable(var_info: &VariableInfo) -> Result<(), String> {
    if var_info.has_annotation {
        let rhs_type = infer_rhs(&var_info.rhs_kind);

        // For now, we'll return an error if the RHS type is Unknown
        // In a real implementation, we would check assignability against the annotation
        if matches!(rhs_type, InferredType::Unknown) {
            return Err("RHS type cannot be inferred".to_string());
        }
    }

    Ok(())
}

/// Infers the type for a variable based on its RHS kind and annotation.
#[must_use]
pub fn infer_variable_type(var_info: &VariableInfo) -> InferredType {
    // If there's an annotation, we need to check assignability
    // For now, we just return the inferred type
    // In a full implementation, we would validate against the annotation
    infer_rhs(&var_info.rhs_kind)
}

/// Tracks variable assignments across control flow branches for union type inference.
#[derive(Debug, Clone)]
pub struct FlowUnionTracker {
    /// Maps variable names to their inferred types across different code paths
    variable_types: std::collections::HashMap<String, Vec<InferredType>>,
    /// Current branch depth for nested control flow
    branch_depth: usize,
}

impl FlowUnionTracker {
    /// Creates a new flow union tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            variable_types: std::collections::HashMap::new(),
            branch_depth: 0,
        }
    }

    /// Enters a new control flow branch (if statement, loop, etc.)
    pub fn enter_branch(&mut self) {
        self.branch_depth += 1;
    }

    /// Exits a control flow branch, merging types from all paths
    pub fn exit_branch(&mut self) {
        if self.branch_depth > 0 {
            self.branch_depth -= 1;
        }
        // Types are kept as-is across branches; a more sophisticated
        // implementation would track per-branch origins and merge here.
    }

    /// Records a variable assignment in the current branch
    pub fn record_assignment(&mut self, var_name: &str, var_type: InferredType) {
        let types = self.variable_types.entry(var_name.to_string()).or_default();

        types.push(var_type);
    }

    /// Gets the inferred union type for a variable across all code paths
    #[must_use]
    pub fn get_union_type(&self, var_name: &str) -> Option<InferredType> {
        self.variable_types.get(var_name).map(|types| {
            if types.is_empty() {
                InferredType::Unknown
            } else if types.len() == 1 {
                types.first().cloned().unwrap_or(InferredType::Unknown)
            } else {
                // Create a union of all types, deduplicating identical types
                let mut deduplicated_types = Vec::new();
                for t in types {
                    if !deduplicated_types.contains(t) {
                        deduplicated_types.push(t.clone());
                    }
                }

                if deduplicated_types.len() == 1 {
                    deduplicated_types
                        .first()
                        .cloned()
                        .unwrap_or(InferredType::Unknown)
                } else {
                    let mut union_type = deduplicated_types
                        .first()
                        .cloned()
                        .unwrap_or(InferredType::Unknown);
                    for t in deduplicated_types.get(1..).unwrap_or_default() {
                        union_type = InferredType::union(union_type, t.clone());
                    }
                    union_type
                }
            }
        })
    }

    /// Resets the tracker for a new function or scope
    pub fn reset(&mut self) {
        self.variable_types.clear();
        self.branch_depth = 0;
    }
}

impl Default for FlowUnionTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Infers types for variables assigned in different control flow paths
#[must_use]
pub fn infer_flow_union_types(
    assignments: &[(String, InferredType)],
) -> std::collections::HashMap<String, InferredType> {
    let mut tracker = FlowUnionTracker::new();

    for (var_name, var_type) in assignments {
        tracker.record_assignment(var_name, var_type.clone());
    }

    let mut result = std::collections::HashMap::new();
    for var_name in tracker.variable_types.keys() {
        if let Some(union_type) = tracker.get_union_type(var_name) {
            let _ = result.insert(var_name.clone(), union_type);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::rhs_fully_determines_type;
    use basilisk_resolver::RhsKind;

    /// [TYPEINF-EXCEEDS-REQUIRED]: scalar literals fully determine a declared
    /// type; the annotation rules must stay silent on them.
    #[test]
    fn scalar_literals_determine_the_type() {
        for kind in [
            RhsKind::IntLiteral,
            RhsKind::FloatLiteral,
            RhsKind::StrLiteral,
            RhsKind::BoolLiteral,
            RhsKind::BytesLiteral,
        ] {
            assert!(
                rhs_fully_determines_type(&kind),
                "{kind:?} must determine its type"
            );
        }
    }

    /// [TYPEINF-EXCEEDS-REQUIRED]: `None`, empty containers, calls, lambdas,
    /// and arbitrary expressions do NOT determine a usable declared type — the
    /// annotation rules must keep firing there.
    #[test]
    fn non_determining_kinds_keep_the_rules_firing() {
        for kind in [
            RhsKind::NoneValue,
            RhsKind::EmptyList,
            RhsKind::EmptyDict,
            RhsKind::CallExpr,
            RhsKind::KnownCall(Box::new(RhsKind::IntLiteral)),
            RhsKind::TypeCall,
            RhsKind::Lambda,
            RhsKind::Other,
        ] {
            assert!(
                !rhs_fully_determines_type(&kind),
                "{kind:?} must NOT determine a type"
            );
        }
    }

    /// Containers determine their type iff non-empty and every element (or
    /// key/value pair) determines its own.
    #[test]
    fn containers_recurse_and_reject_unknown_elements() {
        assert!(rhs_fully_determines_type(&RhsKind::List(vec![
            RhsKind::IntLiteral,
            RhsKind::IntLiteral,
        ])));
        assert!(rhs_fully_determines_type(&RhsKind::Tuple(vec![
            RhsKind::StrLiteral,
            RhsKind::BoolLiteral,
        ])));
        assert!(rhs_fully_determines_type(&RhsKind::Dict(vec![(
            RhsKind::StrLiteral,
            RhsKind::IntLiteral,
        )])));
        // An uninferable element poisons the whole container.
        assert!(!rhs_fully_determines_type(&RhsKind::List(vec![
            RhsKind::IntLiteral,
            RhsKind::CallExpr,
        ])));
        assert!(!rhs_fully_determines_type(&RhsKind::Dict(vec![(
            RhsKind::StrLiteral,
            RhsKind::Other,
        )])));
        // Empty collections carry no element information.
        assert!(!rhs_fully_determines_type(&RhsKind::List(vec![])));
        assert!(!rhs_fully_determines_type(&RhsKind::Dict(vec![])));
    }
}
