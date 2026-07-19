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
mod literal_parse;
mod protocol_members;
mod sig_model;
mod sig_subtype;
mod tuple_check;
mod typeddict_struct;
mod typeform_check;

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
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let empty_params = ParamMaps::default();
        let skip = SkipNames {
            typeddict: collect_typeddict_names(module),
            typeddict_extra_items: collect_extra_items_typeddict_names(module),
            type_alias: collect_type_alias_names(module),
            type_alias_type: collect_type_alias_type_names(module),
            value_aliases: alias_match::collect_value_aliases(module),
            generic_aliases: alias_match::collect_generic_aliases(module),
            typeddict_schemas: typeddict_struct::build_typeddict_schemas(module),
        };
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
        );
        check_local_vars(module, diagnostics, &skip, &call_index);
        check_tuple_reassignments(module, diagnostics);
        check_dataclass_attr_assignments(module, diagnostics);
        typeform_check::check_typeform_calls(module, diagnostics);
        default_spec::check_default_specializations(module, diagnostics);
        drop_unchecked_block_diagnostics(module, diagnostics);
    }
}

/// Remove E0014 diagnostics inside `if not TYPE_CHECKING:` blocks — that code
/// is explicitly excluded from type checking (PEP 484).
fn drop_unchecked_block_diagnostics(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    use ruff_text_size::Ranged as _;

    let Some(parsed) = crate::rules::shared::parse_module(module) else {
        return;
    };
    let blocks: Vec<(u32, u32)> = parsed
        .ast
        .body
        .iter()
        .filter_map(|stmt| {
            let ruff_python_ast::Stmt::If(if_stmt) = stmt else {
                return None;
            };
            let ruff_python_ast::Expr::UnaryOp(unary) = if_stmt.test.as_ref() else {
                return None;
            };
            let is_not_type_checking = unary.op == ruff_python_ast::UnaryOp::Not
                && matches!(
                    unary.operand.as_ref(),
                    ruff_python_ast::Expr::Name(n) if n.id.as_str() == "TYPE_CHECKING"
                );
            is_not_type_checking.then(|| {
                let range = if_stmt.range();
                (range.start().to_u32(), range.end().to_u32())
            })
        })
        .collect();
    if blocks.is_empty() {
        return;
    }
    diagnostics.retain(|diag| {
        diag.code.code != CODE.code
            || !blocks
                .iter()
                .any(|&(start, end)| diag.span.start >= start && diag.span.end <= end)
    });
}

/// Collect names of `TypedDict` classes defined in this module.
///
/// `assignment_compatibility` cannot do structural field-level type checking on `TypedDict`
/// subclasses, so dict literal assignments to `TypedDict` annotations are
/// skipped to avoid false positives.
fn collect_typeddict_names(module: &ResolvedModule) -> std::collections::HashSet<String> {
    // Recognise transitive TypedDict subclasses (`class Album(NamedDict): ...`),
    // not just classes that name `TypedDict` directly. Otherwise E0014 stops
    // skipping their dict-literal assignments and false-positives on every valid
    // `album: Album = {...}` whose base — not the leaf — is the TypedDict.
    let mut names: std::collections::HashSet<String> =
        basilisk_resolver::transitive_typeddict_names(&module.classes)
            .into_iter()
            .map(str::to_ascii_lowercase)
            .collect();

    // Include functional-form TypedDicts: `Name = TypedDict("Name", {...})`.
    for td_call in &module.typeddict_calls {
        let _ = names.insert(td_call.lhs_name.to_ascii_lowercase());
    }

    names
}

/// Collect names of PEP 695 type aliases defined in this module (lowercased).
///
/// E0014 cannot evaluate expanded type alias types, so annotations that
/// reference a type alias are skipped to avoid false positives.
fn collect_type_alias_names(module: &ResolvedModule) -> std::collections::HashSet<String> {
    module
        .type_statements
        .iter()
        .map(|ts| ts.name.to_ascii_lowercase())
        .collect()
}

/// Names that E0014 must skip to avoid false positives.
struct SkipNames {
    /// `TypedDict` class names (lowercase).
    typeddict: std::collections::HashSet<String>,
    /// `TypedDict` classes declaring `extra_items=` (PEP 728, lowercase).
    typeddict_extra_items: std::collections::HashSet<String>,
    /// PEP 695 type alias names (lowercase).
    type_alias: std::collections::HashSet<String>,
    /// `TypeAliasType(...)` call LHS names (lowercase).
    type_alias_type: std::collections::HashSet<String>,
    /// Legacy value aliases — `Name = Union[...]` or a concrete container such
    /// as `Name = dict[K, V]` (lowercase → definition), used for alias-expanded
    /// value matching.
    value_aliases: std::collections::HashMap<String, InferredType>,
    /// Generic (`TypeVar`-parameterised) value aliases such as
    /// `G = list["G[T]" | T]`, keyed by lowercase name. Used to validate
    /// literal assignments against a specialised recursive alias (`G[str]`).
    generic_aliases: std::collections::HashMap<String, alias_match::GenericAlias>,
    /// Effective field schemas (class name → fields) for every `TypedDict`,
    /// used for PEP 705 structural assignability of `TypedDict`-to-`TypedDict`
    /// assignments instead of name equality.
    typeddict_schemas: typeddict_struct::TdSchemas,
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
    let ctx = alias_match::AliasCtx {
        union: &skip.value_aliases,
        generic: &skip.generic_aliases,
    };
    alias_match::alias_assignable(inferred, declared, &ctx, 0)
}

/// Collect names defined via `Name = TypeAliasType(...)` (lowercase).
///
/// E0014 cannot evaluate an expanded `TypeAliasType` alias, so assignments whose
/// declared type references such an alias are skipped to avoid false positives.
fn collect_type_alias_type_names(module: &ResolvedModule) -> std::collections::HashSet<String> {
    module
        .type_alias_type_calls
        .iter()
        .map(|call| call.lhs_name.to_ascii_lowercase())
        .collect()
}

/// Names of `TypedDict` classes declaring `extra_items=` (lowercase).
///
/// Such `TypedDict`s may be assignable to `dict[str, VT]` (PEP 728), which
/// E0014's name-level comparison cannot evaluate — those assignments are
/// skipped rather than flagged.
fn collect_extra_items_typeddict_names(
    module: &ResolvedModule,
) -> std::collections::HashSet<String> {
    module
        .classes
        .iter()
        .filter(|cls| cls.class_keywords.iter().any(|kw| kw == "extra_items"))
        .map(|cls| cls.name.to_ascii_lowercase())
        .collect()
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

            let declared_type = InferredType::from_annotation(annotation_text);

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
            // The annotation may be `TypeAlias`, `TA`, or any local alias.
            {
                let ann_lower = annotation_text.trim().to_ascii_lowercase();
                if ann_lower == "typealias"
                    || ann_lower.ends_with(".typealias")
                    || matches!(declared_type, InferredType::Named(ref n) if n == "ta")
                {
                    return None;
                }
            }

            // Skip annotations that reference a PEP 695 type alias or a
            // `TypeAliasType(...)` alias. E0014 cannot evaluate the expanded alias
            // type, so any assignment check would be unreliable (false positives).
            if let InferredType::Named(ref name) = declared_type {
                let base = name.split('[').next().unwrap_or(name);
                if skip.type_alias.contains(base)
                    || skip.type_alias_type.contains(&base.to_ascii_lowercase())
                {
                    return None;
                }
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
            // expanded definition rather than the `Named`-vs-literal comparison
            // below.
            if let InferredType::Named(ref name) = declared_type {
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
            if let (InferredType::Named(decl), InferredType::Named(inf)) =
                (&declared_type, &inferred_type)
            {
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
) {
    let source = &module.source;
    for func in &module.functions {
        let params = build_param_maps(&func.parameters, source);
        check_vars(
            &func.local_vars,
            source,
            &module.path,
            diagnostics,
            &params,
            skip,
            &module.functions,
            call_index,
        );
    }
}

/// Build maps from parameter name to its declared `InferredType` and raw
/// annotation text by reading the annotation from source spans.
fn build_param_maps(params: &[basilisk_resolver::ParameterInfo], source: &str) -> ParamMaps {
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
        let inferred = InferredType::from_annotation(ann_text.trim());
        let _ = maps.types.insert(param.name.clone(), inferred);
        let _ = maps
            .texts
            .insert(param.name.clone(), ann_text.trim().to_owned());
    }
    maps
}

/// `true` when a dict-literal assignment to a `TypedDict` annotation should
/// be skipped (field-level checking is E0093's job).
fn typeddict_literal_skipped(
    var: &VariableInfo,
    source: &str,
    declared_type: &InferredType,
    skip: &SkipNames,
) -> bool {
    let InferredType::Named(name) = declared_type else {
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
    let InferredType::Named(name) = inferred_type else {
        return false;
    };
    let base = name.split('[').next().unwrap_or(name);
    skip.typeddict_extra_items.contains(base)
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
