//! `assignment_compatibility`: Assignment type incompatibility (literal mismatches).
//! Owns structural `TypedDict` assignment for [TYPEINF-SUBTYPING-TYPEDDICT].
//!
//! Detects annotated module-level variables where the declared type and the
//! literal kind of the right-hand side are clearly incompatible, for example:
//!
//! ```python
//! count: int = "hello"   # str literal assigned to int annotation → E0014
//! label: str = 42        # int literal assigned to str annotation → E0014
//! flag:  bool = "yes"    # str literal assigned to bool annotation → E0014
//! ratio: float = "1.5"   # str literal assigned to float annotation → E0014
//! ```
//!
//! The check is performed by extracting the annotation text from the source
//! around the variable's name span and comparing it against the RHS kind.

mod alias_match;
mod callable_check;
mod dataclass_check;
mod default_spec;
mod enum_expand;
mod literal_parse;
mod protocol_members;
mod sig_model;
mod sig_subtype;
mod skip_names;
mod tuple_check;
mod typeddict_struct;
mod typeform_check;

use enum_expand::enum_expansion_assignable;
use skip_names::{drop_unchecked_block_diagnostics, SkipNames};

use crate::annotation::AnnotationResolver;
use crate::span_util::slice_span;
use crate::types::InferredType;
use basilisk_resolver::{ResolvedModule, RhsKind, Span, VariableInfo};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

use dataclass_check::check_dataclass_attr_assignments;
use literal_parse::infer_with_literal_value;
use tuple_check::check_tuple_reassignments;

pub(crate) const CODE: ErrorCode = ErrorCode {
    code: "assignment_compatibility",
    docs_url: "https://www.basilisk-python.dev/errors/assignment_compatibility",
};

/// Emits `assignment_compatibility` for annotated module variables whose annotation and literal
/// RHS are obviously incompatible.
// Implements [TYPEINF-VARS-ANNOTATED] — the annotation is the declared type; the
// inferred RHS type must be assignable to it (e.g. `x: float = 42` is OK,
// `x: str = 42` is an error).
pub(crate) struct AssignmentTypeMismatch;

impl Rule for AssignmentTypeMismatch {
    fn check(
        &self,
        module: &ResolvedModule,
        ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Standalone entry point (a single-rule test, or any caller outside the
        // driver): build the cascade the driver would otherwise share.
        let annotations = crate::annotation::AnnotationResolver::for_module(module);
        self.check_with_annotations(module, annotations.as_ref(), ctx, diagnostics);
    }

    fn check_with_annotations(
        &self,
        module: &ResolvedModule,
        annotations: Option<&crate::annotation::AnnotationResolver<'_>>,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(resolver) = annotations else {
            return;
        };
        let empty_params = ParamMaps::default();
        let skip = SkipNames::collect(module);
        let call_index = callable_check::build_index(module);
        check_vars(
            &module.module_vars,
            &module.source,
            &module.path,
            diagnostics,
            &empty_params,
            &skip,
            &module.functions,
            &call_index,
            resolver,
        );
        check_local_vars(module, diagnostics, &skip, &call_index, resolver);
        check_tuple_reassignments(module, diagnostics);
        check_dataclass_attr_assignments(module, diagnostics);
        typeform_check::check_typeform_calls(module, diagnostics);
        default_spec::check_default_specializations(module, diagnostics);
        drop_unchecked_block_diagnostics(module, diagnostics);
    }
}

/// Collection literals are checked in the annotation's expected-type context.
/// This permits literal widening (`LiteralString` -> `str`, `int` -> `float`)
/// and empty-container `Never` without weakening invariance between two
/// already-typed mutable containers.
fn literal_collection_assignable(
    var: &VariableInfo,
    inferred: &InferredType,
    declared: &InferredType,
    skip: &SkipNames,
) -> bool {
    if !matches!(
        var.rhs_kind,
        RhsKind::EmptyList
            | RhsKind::EmptyDict
            | RhsKind::List(_)
            | RhsKind::Dict(_)
            | RhsKind::Set(_)
            | RhsKind::Tuple(_)
    ) {
        return false;
    }
    // Bidirectional (expected-type) inference: carry the declared element types
    // INWARD and check each literal element against them — the exact check
    // `returns`/`yield` already use. Without this the RHS is inferred bottom-up
    // to e.g. `dict[LiteralString, Unknown]` (a value from a typed variable
    // becomes `Unknown`, a string key becomes `LiteralString`) and then rejected
    // under dict invariance, so `d: dict[str, str] = {"k": x}` fires while the
    // identical `return {"k": x}` is clean (GitHub #332). A genuine element
    // mismatch still yields `Some(false)` and falls through to the alias check.
    if crate::inference::literal_collection_assignable_to(&var.rhs_kind, declared) == Some(true) {
        return true;
    }
    let ctx = alias_match::AliasCtx {
        union: &skip.value_aliases,
        generic: &skip.generic_aliases,
    };
    alias_match::alias_assignable(inferred, declared, &ctx, 0)
}

/// Declared parameter annotations for the enclosing function: parsed types
/// for assignability checks, and raw annotation texts for structural
/// callable-subtyping checks.
#[derive(Default)]
struct ParamMaps {
    types: std::collections::HashMap<String, InferredType>,
    texts: std::collections::HashMap<String, String>,
}

/// Check a slice of annotated variables for type mismatches.
///
/// `params` maps parameter names to their declared annotation types.
/// When the RHS of an annotated local variable is a simple name reference
/// that matches a parameter, the parameter's type is used for assignability
/// checking instead of the generic `Unknown` fallback.
#[expect(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "assignment checking threads full module context across per-variable branches"
)]
fn check_vars(
    vars: &[VariableInfo],
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
    params: &ParamMaps,
    skip: &SkipNames,
    functions: &[basilisk_resolver::FunctionInfo],
    call_index: &callable_check::CallIndex,
    resolver: &AnnotationResolver<'_>,
) {
    vars.iter()
        .filter(|var| var.has_annotation && var.rhs_span.is_some())
        .filter_map(|var| {
            let annotation_text = extract_annotation(source, var.name_span)?;

            // Quoted forward-reference annotations (e.g. `"Literal[Color.RED]"`)
            // are not evaluated as value types here; skip to avoid false positives.
            if annotation_text.starts_with('"') || annotation_text.starts_with('\'') {
                return None;
            }

            // The declared type is the annotation resolved through the shared
            // cascade ([TYPEINF-ANNOTATION-RESOLUTION]), so an alias or a
            // same-file class is the type it denotes rather than opaque text.
            // Resolved from the annotation NODE where the resolver recorded its
            // span; `resolve_text` re-parses, which costs a `ruff` expression
            // parse per annotated variable ([CHKARCH-TESTING-BENCH-RATCHET]).
            let declared_type = var
                .annotation_span
                .and_then(|span| resolver.resolve_span(span))
                .or_else(|| resolver.resolve_text(annotation_text))?;
            let declared_nominal = nominal_name(&declared_type);

            // TypeForm assignments require type-expression validation, not
            // value-type inference.  Delegate to the dedicated module.
            if let InferredType::TypeForm(ref inner) = declared_type {
                if typeform_check::is_valid_typeform_assignment(var, source, inner, functions) {
                    return None;
                }
                let inferred_type = infer_with_literal_value(var, source, &declared_type);
                return Some((
                    var,
                    annotation_text.to_owned(),
                    inferred_type,
                    declared_type,
                ));
            }

            // Skip TypeAlias-annotated variables — E0048 handles validation.
            // Every spelling — `TypeAlias`, `typing.TypeAlias`, `t.TypeAlias`,
            // `from typing import TypeAlias as TA` — resolves to the same name
            // through the cascade, so one comparison covers them all.
            if declared_nominal.as_deref() == Some("typealias") {
                return None;
            }

            // Skip dict literal assignments to TypedDict annotations. E0014 compares
            // the top-level type (e.g. `dict[str, str|int]` vs `Movie`) which always
            // mismatches. Field-level checking is done by E0093 instead.
            if typeddict_literal_skipped(var, source, &declared_type, skip) {
                return None;
            }

            // When the declared type is a Literal, try to infer the RHS as a
            // literal value so we can compare values, not just kinds.
            let mut inferred_type = infer_with_literal_value(var, source, &declared_type);

            // When the inferred type is Unknown and the RHS text is a parameter
            // name, use the parameter's declared type instead.
            if matches!(inferred_type, InferredType::Unknown) {
                if let Some(rhs_span) = var.rhs_span {
                    if let Some(rhs_text) = slice_span(source, rhs_span) {
                        let rhs_name = rhs_text.trim();
                        if let Some(param_type) = params.types.get(rhs_name) {
                            inferred_type = param_type.clone();
                        }
                    }
                }
            }

            // PEP 728: a TypedDict declaring `extra_items=` may be assignable
            // to `dict[str, VT]`; the name-level comparison below cannot
            // evaluate that, so such assignments are skipped.
            if extra_items_dict_skipped(&declared_type, &inferred_type, skip) {
                return None;
            }

            // A reference to a legacy value alias — a recursive `Union` alias
            // (`Json`) or a generic `list[...]`-bodied alias needing `TypeVar`
            // substitution (`G[str]`) — needs value-level matching against the
            // expanded definition. It is keyed by the annotation's own
            // spelling, not by the resolved type: expanding a *recursive* alias
            // through the cascade necessarily makes its recursive arm gradual
            // ([TYPEINF-ANNOTATION-RESOLUTION] cycle guard), which would accept
            // values this matcher rejects. The matcher dies with the alias
            // tables in [NARROWPLAN-INTEGRATION] Step 7.
            {
                let name = &annotation_text.trim().to_ascii_lowercase();
                let ctx = alias_match::AliasCtx {
                    union: &skip.value_aliases,
                    generic: &skip.generic_aliases,
                };
                if let Some(matched) =
                    alias_match::alias_value_assignable(&inferred_type, name, &ctx)
                {
                    return if matched {
                        None
                    } else {
                        Some((
                            var,
                            annotation_text.to_owned(),
                            inferred_type,
                            declared_type,
                        ))
                    };
                }
            }

            // TypedDict → TypedDict: use PEP 705 structural assignability rather
            // than name equality, which would flag every structurally-valid
            // cross-name assignment (`v: A = b` where `b: B`). Genuine mismatches
            // still fire. Only reachable when the RHS resolves to a TypedDict-typed
            // name (e.g. a parameter), so module-level checks are unaffected.
            if let (Some(decl), Some(inf)) = (&declared_nominal, nominal_name(&inferred_type)) {
                if let (Some(target), Some(src)) = (
                    skip.typeddict_schemas.get(decl.as_str()),
                    skip.typeddict_schemas.get(inf.as_str()),
                ) {
                    return if typeddict_struct::typeddict_assignable(src, target) {
                        None
                    } else {
                        Some((
                            var,
                            annotation_text.to_owned(),
                            inferred_type,
                            declared_type,
                        ))
                    };
                }
            }

            if inferred_type.is_assignable_to(&declared_type)
                || literal_collection_assignable(var, &inferred_type, &declared_type, skip)
                || enum_expansion_assignable(&inferred_type, &declared_type, &skip.enum_members)
            {
                None
            } else if callable_rescue(var, source, annotation_text, params, call_index) {
                // Structurally valid callable subtyping (callback protocols,
                // `Callable[...]` forms, `TypeAlias` callables) — not a mismatch.
                None
            } else {
                Some((
                    var,
                    annotation_text.to_owned(),
                    inferred_type,
                    declared_type,
                ))
            }
        })
        .for_each(|(var, annotation, inferred, declared)| {
            diagnostics.push(make_diagnostic(
                var,
                &annotation,
                &inferred,
                &declared,
                path,
            ));
        });
}

/// Attempt to validate a flagged assignment as structurally compatible
/// callable subtyping: the RHS must be a parameter whose raw annotation text,
/// compared against the declared annotation, passes the callable subtype check.
fn callable_rescue(
    var: &VariableInfo,
    source: &str,
    annotation_text: &str,
    params: &ParamMaps,
    call_index: &callable_check::CallIndex,
) -> bool {
    let Some(rhs_text) = var.rhs_span.and_then(|span| slice_span(source, span)) else {
        return false;
    };
    let Some(rhs_annotation) = params.texts.get(rhs_text.trim()) else {
        return false;
    };
    callable_check::assignment_compatible(annotation_text, rhs_annotation, call_index)
}

/// Check local variables in function bodies for type mismatches.
///
/// Builds a map of parameter name to declared type for each function so that
/// assignments like `x: Literal[False] = a` (where `a: Literal[0]`) can be
/// checked for Literal-level incompatibility.
fn check_local_vars(
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
    skip: &SkipNames,
    call_index: &callable_check::CallIndex,
    resolver: &AnnotationResolver<'_>,
) {
    let source = &module.source;
    for func in &module.functions {
        let params = build_param_maps(&func.parameters, source, resolver);
        check_vars(
            &func.local_vars,
            source,
            &module.path,
            diagnostics,
            &params,
            skip,
            &module.functions,
            call_index,
            resolver,
        );
    }
}

/// Build maps from parameter name to its declared `InferredType` and raw
/// annotation text by reading the annotation from source spans.
fn build_param_maps(
    params: &[basilisk_resolver::ParameterInfo],
    source: &str,
    resolver: &AnnotationResolver<'_>,
) -> ParamMaps {
    let mut maps = ParamMaps::default();
    for param in params {
        if !param.has_annotation {
            continue;
        }
        let Some(ann_span) = param.annotation_span else {
            continue;
        };
        let Some(ann_text) = slice_span(source, ann_span) else {
            continue;
        };
        // The parameter's declared type comes from the same cascade the
        // assignment's annotation does ([TYPEINF-ANNOTATION-RESOLUTION]).
        let Some(inferred) = resolver
            .resolve_span(ann_span)
            .or_else(|| resolver.resolve_text(ann_text))
        else {
            continue;
        };
        let _ = maps.types.insert(param.name.clone(), inferred);
        let _ = maps
            .texts
            .insert(param.name.clone(), ann_text.trim().to_owned());
    }
    maps
}

/// A nominal type's spelling, folded to the case this rule's name tables use.
///
/// Those tables are keyed lower-case, a legacy of
/// `InferredType::from_annotation` having lower-cased every annotation it
/// parsed. The [TYPEINF-ANNOTATION-RESOLUTION] cascade preserves a class's real
/// case, so every lookup folds here rather than at each site — and the tables
/// can be re-keyed in one place once the last lower-casing consumer dies.
fn nominal_name(ty: &InferredType) -> Option<String> {
    match ty {
        InferredType::Named(name) => Some(name.to_ascii_lowercase()),
        _ => None,
    }
}

/// [`nominal_name`] with any subscript stripped — `Pair[int]` keys as `pair`.
fn nominal_key(ty: &InferredType) -> Option<String> {
    nominal_name(ty).map(|name| match name.split_once('[') {
        Some((base, _)) => base.to_owned(),
        None => name,
    })
}

/// `true` when a dict-literal assignment to a `TypedDict` annotation should
/// be skipped (field-level checking is E0093's job).
fn typeddict_literal_skipped(
    var: &VariableInfo,
    source: &str,
    declared_type: &InferredType,
    skip: &SkipNames,
) -> bool {
    let Some(name) = nominal_key(declared_type) else {
        return false;
    };
    skip.typeddict.contains(name.as_str())
        && var
            .rhs_span
            .and_then(|sp| slice_span(source, sp))
            .is_some_and(|rhs| rhs.trim_start().starts_with('{'))
}

/// `true` when an `extra_items=` `TypedDict` is assigned to a `dict[...]`
/// annotation — assignability depends on PEP 728 value types, which the
/// name-level comparison cannot evaluate.
fn extra_items_dict_skipped(
    declared_type: &InferredType,
    inferred_type: &InferredType,
    skip: &SkipNames,
) -> bool {
    if !matches!(declared_type, InferredType::Dict(..)) {
        return false;
    }
    let Some(base) = nominal_key(inferred_type) else {
        return false;
    };
    skip.typeddict_extra_items.contains(base.as_str())
}

/// Create diagnostic for inference-based type mismatch.
fn make_diagnostic(
    var: &VariableInfo,
    annotation: &str,
    inferred: &InferredType,
    declared: &InferredType,
    path: &str,
) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Type mismatch: `{}` is annotated `{annotation}` ({}) but assigned {}",
            var.name, declared, inferred
        ),
        var.name_span,
        path,
        Some(format!(
            "Either change the annotation to match the value, or change the value to `{annotation}`"
        )),
        Some(
            "Basilisk requires the inferred type to be assignable to the declared type".to_owned(),
        ),
    )
}

/// Extract the annotation text from the source line containing `name_span`.
///
/// Looks for `: <annotation>` on the same source line as the variable name,
/// stopping at the `=` sign that introduces the RHS.  Returns `None` if no
/// such pattern is found.
pub(super) fn extract_annotation(source: &str, name_span: Span) -> Option<&str> {
    // Find the byte offset of the start of the line containing the name.
    let start = usize::try_from(name_span.start).ok()?;
    let line_start = source.get(..start)?.rfind('\n').map_or(0, |pos| pos + 1);
    let line_end = source
        .get(start..)?
        .find('\n')
        .map_or(source.len(), |pos| start + pos);

    let line = source.get(line_start..line_end)?;

    // Position of the name within the line.
    let name_offset = start.checked_sub(line_start)?;

    // Find `: ` after the name position on this line.
    let colon_pos = line.get(name_offset..)?.find(": ")? + name_offset;
    let after_colon = colon_pos + 2; // skip ': '

    // Find `=` that ends the annotation (must be after the colon).
    let annotation_end = line
        .get(after_colon..)?
        .find('=')
        .map_or(line.len(), |p| after_colon + p);

    let annotation = line.get(after_colon..annotation_end)?.trim();

    if annotation.is_empty() {
        None
    } else {
        Some(annotation)
    }
}
