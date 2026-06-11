//! BSK-E0014: Assignment type incompatibility (literal mismatches).
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
mod tuple_check;
mod typeform_check;

use crate::span_util::slice_span;
use crate::types::InferredType;
use basilisk_resolver::{ResolvedModule, Span, VariableInfo};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

use dataclass_check::check_dataclass_attr_assignments;
use literal_parse::infer_with_literal_value;
use tuple_check::check_tuple_reassignments;

pub(crate) const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0014",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0014",
};

/// Emits BSK-E0014 for annotated module variables whose annotation and literal
/// RHS are obviously incompatible.
pub(crate) struct AssignmentTypeMismatch;

impl Rule for AssignmentTypeMismatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let empty_params = ParamMaps::default();
        let skip = SkipNames {
            typeddict: collect_typeddict_names(module),
            type_alias: collect_type_alias_names(module),
            value_aliases: alias_match::collect_union_aliases(module),
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
    }
}

/// Collect names of `TypedDict` classes defined in this module.
///
/// BSK-E0014 cannot do structural field-level type checking on `TypedDict`
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
    /// PEP 695 type alias names (lowercase).
    type_alias: std::collections::HashSet<String>,
    /// Legacy `Name = Union[...]` value aliases (lowercase → definition), used
    /// for recursive-alias value matching.
    value_aliases: std::collections::HashMap<String, InferredType>,
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
    reason = "assignment checking threads full module context"
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

            // Skip annotations that reference a PEP 695 type alias. E0014 cannot
            // evaluate the expanded alias type, so any assignment check would be
            // unreliable and produce false positives.
            if let InferredType::Named(ref name) = declared_type {
                let base = name.split('[').next().unwrap_or(name);
                if skip.type_alias.contains(base) {
                    return None;
                }
            }

            // Skip dict literal assignments to TypedDict annotations. E0014 compares
            // the top-level type (e.g. `dict[str, str|int]` vs `Movie`) which always
            // mismatches. Field-level checking is done by E0093 instead.
            if let InferredType::Named(ref name) = declared_type {
                if skip.typeddict.contains(name.as_str()) {
                    let rhs_is_dict_literal = var
                        .rhs_span
                        .and_then(|sp| slice_span(source, sp))
                        .is_some_and(|rhs| rhs.trim_start().starts_with('{'));
                    if rhs_is_dict_literal {
                        return None;
                    }
                }
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

            // A bare reference to a legacy `Union` alias (e.g. a recursive
            // `Json` alias) needs value-level matching against the expanded
            // definition rather than the `Named`-vs-literal comparison below.
            if let InferredType::Named(ref name) = declared_type {
                if !name.contains('[') {
                    if let Some(def) = skip.value_aliases.get(name.as_str()) {
                        return if alias_match::alias_assignable(
                            &inferred_type,
                            def,
                            &skip.value_aliases,
                            0,
                        ) {
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
            }

            if inferred_type.is_assignable_to(&declared_type) {
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
