//! Implements [`narrowing_typeis_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `narrowing_typeis_2`: `TypeIs` narrows to a type inconsistent with the input type.
//!
//! Per the typing spec: "It is an error to narrow to a type that is not
//! consistent with the input type." For `TypeIs`, the narrowed type must
//! be a subtype of the input type.

use basilisk_resolver::ResolvedModule;

use super::Rule;
use crate::annotation::AnnotationResolver;
use crate::diagnostic::{error_diag_help_note, Diagnostic, ErrorCode};
use crate::subtyping::SubtypingContext;
use crate::types::InferredType;

const CODE: ErrorCode = ErrorCode {
    code: "narrowing_typeis_2",
    docs_url: "https://www.basilisk-python.dev/errors/narrowing_typeis_2",
};

/// Emits `narrowing_typeis_2` when a function returns `TypeIs[X]` but `X` is not
/// consistent with the first parameter type.
///
/// Implements [TYPEINF-NARROWING-TYPEIS] — the PEP 742 consistency precondition:
/// because `TypeIs` narrows bidirectionally, the narrowed type `X` must be a
/// subtype of (consistent with) the input parameter type. Both sides resolve
/// through [TYPEINF-ANNOTATION-RESOLUTION] first, so aliases expand and the
/// nominal walk sees classes, not annotation text; a side the module cannot
/// ground (a `TypeVar`, an unseen import) abstains rather than guesses.
pub(crate) struct TypeIsInconsistentNarrowing;

/// The three-valued consistency judgment: a verdict either way requires both
/// sides to be grounded; anything the judgment cannot decide abstains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    /// The narrowed type is assignable to the input type.
    Consistent,
    /// Both sides are grounded and the narrowed type is NOT assignable.
    Inconsistent,
    /// At least one side is not decidable here — no diagnostic.
    Unknown,
}

/// The leaf name a type compares nominally by, or `None` when the type is not
/// a groundable leaf (so the judgment must abstain or recurse structurally).
fn leaf_name(resolver: &AnnotationResolver<'_>, ty: &InferredType) -> Option<String> {
    match ty {
        InferredType::Int => Some("int".to_owned()),
        InferredType::Str | InferredType::LiteralString => Some("str".to_owned()),
        InferredType::Float => Some("float".to_owned()),
        InferredType::Bool => Some("bool".to_owned()),
        InferredType::Bytes => Some("bytes".to_owned()),
        InferredType::None_ => Some("None".to_owned()),
        InferredType::Literal(value) => Some(
            match value {
                crate::types::LiteralValue::Int(_) => "int",
                crate::types::LiteralValue::Str(_) => "str",
                crate::types::LiteralValue::Float(_) => "float",
                crate::types::LiteralValue::Bool(_) => "bool",
                crate::types::LiteralValue::Bytes(_) => "bytes",
            }
            .to_owned(),
        ),
        InferredType::Named(name) => resolver.is_grounded_name(name).then(|| name.clone()),
        _ => None,
    }
}

/// Consistency of `narrowed` with `input` on RESOLVED types.
fn consistency(
    resolver: &AnnotationResolver<'_>,
    ctx: &SubtypingContext,
    narrowed: &InferredType,
    input: &InferredType,
) -> Verdict {
    if narrowed == input
        || matches!(narrowed, InferredType::Any | InferredType::Never)
        || matches!(input, InferredType::Any)
    {
        return Verdict::Consistent;
    }
    match (narrowed, input) {
        // A narrowed union is consistent when EVERY arm is.
        (InferredType::Union(arms), _) => all_arms(
            arms.iter()
                .map(|arm| consistency(resolver, ctx, arm, input)),
        ),
        (InferredType::Optional(inner), _) => all_arms(
            [
                consistency(resolver, ctx, inner, input),
                consistency(resolver, ctx, &InferredType::None_, input),
            ]
            .into_iter(),
        ),
        // An input union accepts a narrow into ANY of its arms.
        (_, InferredType::Union(arms)) => any_arm(
            arms.iter()
                .map(|arm| consistency(resolver, ctx, narrowed, arm)),
        ),
        (_, InferredType::Optional(inner)) => any_arm(
            [
                consistency(resolver, ctx, narrowed, inner),
                matches!(narrowed, InferredType::None_)
                    .then_some(Verdict::Consistent)
                    .unwrap_or(Verdict::Inconsistent),
            ]
            .into_iter(),
        ),
        // Same-shape containers are invariant: equality was checked above, so
        // grounded-but-different arguments are inconsistent.
        (InferredType::List(a), InferredType::List(b))
        | (InferredType::Set(a), InferredType::Set(b)) => {
            invariant(resolver, ctx, &[a.as_ref().clone()], &[b.as_ref().clone()])
        }
        (InferredType::Dict(ak, av), InferredType::Dict(bk, bv)) => invariant(
            resolver,
            ctx,
            &[ak.as_ref().clone(), av.as_ref().clone()],
            &[bk.as_ref().clone(), bv.as_ref().clone()],
        ),
        (InferredType::Tuple(a), InferredType::Tuple(b)) => invariant(resolver, ctx, a, b),
        _ => leaf_consistency(resolver, ctx, narrowed, input),
    }
}

/// Both sides as nominal leaves through the shared subtype walk; anything
/// either side cannot ground abstains.
fn leaf_consistency(
    resolver: &AnnotationResolver<'_>,
    ctx: &SubtypingContext,
    narrowed: &InferredType,
    input: &InferredType,
) -> Verdict {
    match (leaf_name(resolver, narrowed), leaf_name(resolver, input)) {
        (Some(sub), Some(sup)) => {
            if ctx.is_subtype(&sub, &sup) {
                Verdict::Consistent
            } else {
                Verdict::Inconsistent
            }
        }
        _ => Verdict::Unknown,
    }
}

/// Invariant positions: every pair must be mutually consistent; a grounded
/// difference in either direction is inconsistent, arity mismatch too.
fn invariant(
    resolver: &AnnotationResolver<'_>,
    ctx: &SubtypingContext,
    a: &[InferredType],
    b: &[InferredType],
) -> Verdict {
    if a.len() != b.len() {
        return Verdict::Inconsistent;
    }
    all_arms(a.iter().zip(b).map(|(x, y)| {
        match (
            consistency(resolver, ctx, x, y),
            consistency(resolver, ctx, y, x),
        ) {
            (Verdict::Consistent, Verdict::Consistent) => Verdict::Consistent,
            (Verdict::Unknown, _) | (_, Verdict::Unknown) => Verdict::Unknown,
            _ => Verdict::Inconsistent,
        }
    }))
}

/// Fold "every arm must be consistent": any inconsistency wins, any
/// undecidable arm abstains the whole judgment.
fn all_arms(verdicts: impl Iterator<Item = Verdict>) -> Verdict {
    let mut result = Verdict::Consistent;
    for verdict in verdicts {
        match verdict {
            Verdict::Inconsistent => return Verdict::Inconsistent,
            Verdict::Unknown => result = Verdict::Unknown,
            Verdict::Consistent => {}
        }
    }
    result
}

/// Fold "some arm must accept": any consistent arm wins; otherwise abstain if
/// anything was undecidable.
fn any_arm(verdicts: impl Iterator<Item = Verdict>) -> Verdict {
    let mut result = Verdict::Inconsistent;
    for verdict in verdicts {
        match verdict {
            Verdict::Consistent => return Verdict::Consistent,
            Verdict::Unknown => result = Verdict::Unknown,
            Verdict::Inconsistent => {}
        }
    }
    result
}

impl Rule for TypeIsInconsistentNarrowing {
    fn check(
        &self,
        module: &ResolvedModule,
        ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        super::check_with_own_types(self, module, ctx, diagnostics);
    }

    fn check_with_types(
        &self,
        module: &ResolvedModule,
        types: &super::shared::module_types::ModuleTypes<'_>,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(resolver) = types.annotations() else {
            return;
        };
        let subtyping = types.subtyping();

        for func in &module.functions {
            let Some(ann_span) = func.return_annotation_span else {
                continue;
            };

            // Only `TypeIs` carries the consistency precondition — resolved,
            // so an alias of `TypeIs[X]` is checked exactly like the spelled
            // form ([TYPEINF-ANNOTATION-RESOLUTION]). Resolved from the
            // annotation NODE the span points at: re-parsing the text would
            // cost a `ruff` expression parse per annotated function, and the
            // node is already indexed.
            let Some(InferredType::Guard {
                type_is: true,
                inner,
            }) = resolver.resolve_span(ann_span)
            else {
                continue;
            };

            // PEP 742: "The type narrowing behavior is applied to the first
            // positional argument" — for a method or classmethod, the one
            // after the implicit receiver. Receiver-ness is the function's
            // KIND (a method that is not a staticmethod), never a parameter's
            // name: `def is_wide(self: object) -> TypeIs[int]` at module
            // scope has no receiver, whatever its parameter is called.
            let receiver_count =
                usize::from(func.class_name.is_some() && !func.is_staticmethod);
            let Some(param) = func
                .parameters
                .get(..func.positional_count)
                .and_then(|positional| positional.get(receiver_count))
            else {
                // No positional parameter to narrow — that defect belongs to
                // `narrowing_typeguard`, not this consistency rule.
                continue;
            };
            let Some(param_ann_span) = param.annotation_span else {
                continue;
            };
            let Some(input) = resolver.resolve_span(param_ann_span) else {
                continue;
            };

            // Structural targets (Protocols, TypedDicts) need a structural
            // judgment this nominal walk cannot make — abstain.
            if resolver.is_structural_target(&inner) || resolver.is_structural_target(&input) {
                continue;
            }

            if consistency(resolver, subtyping, &inner, &input) == Verdict::Inconsistent {
                diagnostics.push(error_diag_help_note(
                    CODE.clone(),
                    format!(
                        "`TypeIs[{inner}]` narrows to a type inconsistent with parameter type `{input}`"
                    ),
                    ann_span,
                    &module.path,
                    format!(
                        "The narrowed type `{inner}` must be consistent with the input type `{input}`"
                    ),
                    "Per the typing spec, TypeIs requires the narrowed type to be \
                     consistent with the input type",
                ));
            }
        }
    }
}
