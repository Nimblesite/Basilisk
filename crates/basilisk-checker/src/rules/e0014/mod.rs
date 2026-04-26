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

mod dataclass_check;
mod literal_parse;
mod tuple_check;
mod typeform_check;

use crate::span_util::slice_span;
use crate::types::InferredType;
use basilisk_resolver::{ResolvedModule, Span, VariableInfo};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

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
        let empty_params = std::collections::HashMap::new();
        let skip = SkipNames {
            typeddict: collect_typeddict_names(module),
            type_alias: collect_type_alias_names(module),
        };
        check_vars(
            &module.module_vars,
            &module.source,
            &module.path,
            diagnostics,
            &empty_params,
            &skip,
            &module.functions,
        );
        check_local_vars(module, diagnostics, &skip);
        check_tuple_reassignments(module, diagnostics);
        check_dataclass_attr_assignments(module, diagnostics);
        typeform_check::check_typeform_calls(module, diagnostics);
    }
}

/// Collect names of `TypedDict` classes defined in this module.
///
/// BSK-E0014 cannot do structural field-level type checking on `TypedDict`
/// subclasses, so dict literal assignments to `TypedDict` annotations are
/// skipped to avoid false positives.
fn collect_typeddict_names(module: &ResolvedModule) -> std::collections::HashSet<String> {
    let mut names: std::collections::HashSet<String> = module
        .classes
        .iter()
        .filter(|c| {
            c.bases.iter().any(|b| {
                matches!(
                    b.as_str(),
                    "TypedDict" | "typing.TypedDict" | "typing_extensions.TypedDict"
                )
            })
        })
        .map(|c| c.name.to_ascii_lowercase())
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
}

/// Check a slice of annotated variables for type mismatches.
///
/// `param_types` maps parameter names to their declared annotation types.
/// When the RHS of an annotated local variable is a simple name reference
/// that matches a parameter, the parameter's type is used for assignability
/// checking instead of the generic `Unknown` fallback.
fn check_vars(
    vars: &[VariableInfo],
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
    param_types: &std::collections::HashMap<String, InferredType>,
    skip: &SkipNames,
    functions: &[basilisk_resolver::FunctionInfo],
) {
    vars.iter()
        .filter(|var| var.has_annotation && var.rhs_span.is_some())
        .filter_map(|var| {
            let annotation_text = extract_annotation(source, var.name_span)?;
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
                        if let Some(param_type) = param_types.get(rhs_name) {
                            inferred_type = param_type.clone();
                        }
                    }
                }
            }

            if inferred_type.is_assignable_to(&declared_type) {
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

/// Check local variables in function bodies for type mismatches.
///
/// Builds a map of parameter name to declared type for each function so that
/// assignments like `x: Literal[False] = a` (where `a: Literal[0]`) can be
/// checked for Literal-level incompatibility.
fn check_local_vars(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>, skip: &SkipNames) {
    let source = &module.source;
    for func in &module.functions {
        let param_types = build_param_type_map(&func.parameters, source);
        check_vars(
            &func.local_vars,
            source,
            &module.path,
            diagnostics,
            &param_types,
            skip,
            &module.functions,
        );
    }
}

/// Build a map from parameter name to its declared `InferredType` by reading
/// the annotation text from source spans.
fn build_param_type_map(
    params: &[basilisk_resolver::ParameterInfo],
    source: &str,
) -> std::collections::HashMap<String, InferredType> {
    let mut map = std::collections::HashMap::new();
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
        let _ = map.insert(param.name.clone(), inferred);
    }
    map
}

/// Create diagnostic for inference-based type mismatch.
fn make_diagnostic(
    var: &VariableInfo,
    annotation: &str,
    inferred: &InferredType,
    declared: &InferredType,
    path: &str,
) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Type mismatch: `{}` is annotated `{annotation}` ({}) but assigned {}",
            var.name, declared, inferred
        ),
        span: var.name_span,
        path: path.to_owned(),
        help: Some(format!(
            "Either change the annotation to match the value, or change the value to `{annotation}`"
        )),
        note: Some(
            "Basilisk requires the inferred type to be assignable to the declared type".to_owned(),
        ),
        provenance: None,
    }
}

/// Extract the annotation text from the source line containing `name_span`.
///
/// Looks for `: <annotation>` on the same source line as the variable name,
/// stopping at the `=` sign that introduces the RHS.  Returns `None` if no
/// such pattern is found.
fn extract_annotation(source: &str, name_span: Span) -> Option<&str> {
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
