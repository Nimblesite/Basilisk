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

/// Synthesize the type of one expression's SOURCE text through the
/// bidirectional engine ([TYPEINF-TARGET-BIDIRECTIONAL]) — **the** shared
/// inference behind checker diagnostics
/// ([`crate::incremental_defs::expression_types`]), hover, completions, and
/// inlay hints ([NARROWPLAN-CHECKLIST] Stage 2: "reuse the same inference
/// results"). Anything unparseable or unsupported answers the conservative
/// `Unknown` — never a guess.
#[must_use]
pub fn infer_expression_source(source: &str) -> InferredType {
    let Ok(parsed) = ruff_python_parser::parse_expression(source) else {
        return InferredType::Unknown;
    };
    let module = parsed.into_syntax();
    let mut engine = crate::bidir::BidirEngine::new(std::collections::HashMap::new());
    let ty = engine.synth(&module.body);
    let solution = engine.finish();
    ty.to_inferred(&solution.vars)
}

/// Whether a type contains no `Unknown` anywhere — display surfaces show a
/// type only when it is fully known (a partial `list[Unknown]` hint would be
/// worse than silence, per the gradual posture [TYPEINF-TARGET-GRADUAL]).
#[must_use]
pub fn is_fully_known(ty: &InferredType) -> bool {
    match ty {
        InferredType::Unknown => false,
        InferredType::List(inner)
        | InferredType::Set(inner)
        | InferredType::Optional(inner)
        | InferredType::TypeForm(inner) => is_fully_known(inner),
        InferredType::Dict(key, value) => is_fully_known(key) && is_fully_known(value),
        InferredType::Tuple(elems) | InferredType::Union(elems) => elems.iter().all(is_fully_known),
        InferredType::Callable(info) => {
            info.param_types.iter().all(is_fully_known) && is_fully_known(&info.return_type)
        }
        InferredType::Generator(yielded, sent, returned) => {
            is_fully_known(yielded) && is_fully_known(sent) && is_fully_known(returned)
        }
        _ => true,
    }
}

/// Widen an inferred type to its DISPLAY form: literals become their base
/// type (`Literal[1]` → `int`), matching how annotations are conventionally
/// written in hover/inlay surfaces. Precision-preserving variants
/// (`LiteralString`, unions, containers) widen structurally.
#[must_use]
pub fn display_widened(ty: &InferredType) -> InferredType {
    match ty {
        InferredType::Literal(literal) => match literal {
            crate::types::LiteralValue::Int(_) => InferredType::Int,
            crate::types::LiteralValue::Str(_) => InferredType::Str,
            crate::types::LiteralValue::Float(_) => InferredType::Float,
            crate::types::LiteralValue::Bool(_) => InferredType::Bool,
            crate::types::LiteralValue::Bytes(_) => InferredType::Bytes,
        },
        InferredType::LiteralString => InferredType::Str,
        InferredType::List(elem) => InferredType::List(Box::new(display_widened(elem))),
        InferredType::Set(elem) => InferredType::Set(Box::new(display_widened(elem))),
        InferredType::Dict(key, value) => InferredType::Dict(
            Box::new(display_widened(key)),
            Box::new(display_widened(value)),
        ),
        InferredType::Tuple(elems) => {
            InferredType::Tuple(elems.iter().map(display_widened).collect())
        }
        InferredType::Optional(inner) => InferredType::Optional(Box::new(display_widened(inner))),
        InferredType::Union(members) => members
            .iter()
            .map(display_widened)
            .fold(InferredType::Never, InferredType::union),
        other => other.clone(),
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

    /// [NARROWPLAN-CHECKLIST] Stage 2: the shared expression-source entry
    /// point runs the SAME bidirectional engine as checker diagnostics —
    /// literals, containers, arithmetic, and method calls all synthesize.
    #[test]
    fn expression_source_synthesizes_through_the_shared_engine() {
        use super::infer_expression_source;
        use crate::types::{InferredType, LiteralValue};
        assert_eq!(
            infer_expression_source("42"),
            InferredType::Literal(LiteralValue::Int(42))
        );
        // Elements keep literal precision; display widening collapses them.
        assert_eq!(
            super::display_widened(&infer_expression_source("[1, 2]")),
            InferredType::List(Box::new(InferredType::Int))
        );
        assert_eq!(infer_expression_source("1 + 2.5"), InferredType::Float);
        assert_eq!(infer_expression_source("'a'.upper()"), InferredType::Str);
        // Unparseable or unresolvable input answers `Unknown`, never a guess.
        assert_eq!(infer_expression_source("def ("), InferredType::Unknown);
        assert_eq!(infer_expression_source("mystery"), InferredType::Unknown);
    }

    /// Display widening turns literal precision into the annotation-style
    /// base type, recursing through containers and unions.
    #[test]
    fn display_widening_reaches_annotation_form() {
        use super::display_widened;
        use crate::types::{InferredType, LiteralValue};
        assert_eq!(
            display_widened(&InferredType::Literal(LiteralValue::Int(1))),
            InferredType::Int
        );
        assert_eq!(
            display_widened(&InferredType::LiteralString),
            InferredType::Str
        );
        assert_eq!(
            display_widened(&InferredType::List(Box::new(InferredType::Literal(
                LiteralValue::Str("x".into())
            )))),
            InferredType::List(Box::new(InferredType::Str))
        );
        let union = InferredType::Union(vec![
            InferredType::Literal(LiteralValue::Int(1)),
            InferredType::Literal(LiteralValue::Int(2)),
            InferredType::Str,
        ]);
        assert_eq!(
            display_widened(&union),
            InferredType::Union(vec![InferredType::Int, InferredType::Str]),
            "widened literal duplicates must collapse in the union"
        );
    }

    /// `is_fully_known` rejects any type with a nested `Unknown` — display
    /// surfaces stay silent instead of rendering partial types
    /// ([TYPEINF-TARGET-GRADUAL]).
    #[test]
    fn fully_known_rejects_nested_unknowns() {
        use super::is_fully_known;
        use crate::types::{CallableInfo, InferredType};
        assert!(is_fully_known(&InferredType::Int));
        assert!(is_fully_known(&InferredType::List(Box::new(
            InferredType::Str
        ))));
        assert!(!is_fully_known(&InferredType::Unknown));
        assert!(!is_fully_known(&InferredType::List(Box::new(
            InferredType::Unknown
        ))));
        assert!(!is_fully_known(&InferredType::Dict(
            Box::new(InferredType::Str),
            Box::new(InferredType::Unknown)
        )));
        assert!(!is_fully_known(&InferredType::Union(vec![
            InferredType::Int,
            InferredType::Unknown
        ])));
        assert!(!is_fully_known(&InferredType::Callable(CallableInfo {
            param_types: vec![],
            return_type: Box::new(InferredType::Unknown),
        })));
        assert!(!is_fully_known(&InferredType::Generator(
            Box::new(InferredType::Int),
            Box::new(InferredType::None_),
            Box::new(InferredType::Unknown)
        )));
    }
}
