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
use crate::subtyping::{is_subtype_with_context, SubtypeContext};
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
        let typeddict_names = collect_typeddict_names(module);
        let subtype_ctx = SubtypeContext::from_module(module);
        let cx = CheckVarsCtx {
            source: &module.source,
            path: &module.path,
            param_types: &empty_params,
            typeddict_names: &typeddict_names,
            functions: &module.functions,
            subtype_ctx: &subtype_ctx,
        };
        check_vars(&module.module_vars, diagnostics, &cx);
        check_local_vars(module, diagnostics, &typeddict_names, &subtype_ctx);
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
            c.is_typed_dict
                || c.bases.iter().any(|b| {
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

    // Transitive: classes inheriting from known TypedDicts.
    let mut changed = true;
    while changed {
        changed = false;
        for class in &module.classes {
            let lower = class.name.to_ascii_lowercase();
            if names.contains(&lower) {
                continue;
            }
            if class
                .bases
                .iter()
                .any(|b| names.contains(&b.to_ascii_lowercase()))
            {
                let _ = names.insert(lower);
                changed = true;
            }
        }
    }

    names
}

/// Bundled context for `check_vars` to stay under the argument limit.
struct CheckVarsCtx<'a> {
    /// Source text.
    source: &'a str,
    /// File path for diagnostics.
    path: &'a str,
    /// Parameter name → declared type map.
    param_types: &'a std::collections::HashMap<String, InferredType>,
    /// Names of `TypedDict` classes.
    typeddict_names: &'a std::collections::HashSet<String>,
    /// All functions in the module.
    functions: &'a [basilisk_resolver::FunctionInfo],
    /// Subtype checking context (MRO, protocols, `TypedDict`).
    subtype_ctx: &'a SubtypeContext<'a>,
}

/// Check annotated variables for type mismatches using `SubtypeContext` for
/// proper MRO / protocol / `TypedDict` subtype resolution.
fn check_vars(vars: &[VariableInfo], diagnostics: &mut Vec<Diagnostic>, cx: &CheckVarsCtx<'_>) {
    vars.iter()
        .filter(|var| var.has_annotation && var.rhs_span.is_some())
        .filter_map(|var| {
            let annotation_text = extract_annotation(cx.source, var.name_span)?;
            let declared_type = InferredType::from_annotation(annotation_text);

            // TypeForm assignments require type-expression validation, not
            // value-type inference.  Delegate to the dedicated module.
            if let InferredType::TypeForm(ref inner) = declared_type {
                if typeform_check::is_valid_typeform_assignment(var, cx.source, inner, cx.functions)
                {
                    return None;
                }
                let inferred_type = infer_with_literal_value(var, cx.source, &declared_type);
                return Some((
                    var,
                    annotation_text.to_owned(),
                    inferred_type,
                    declared_type,
                ));
            }

            // Skip TypeAlias-annotated variables — E0048 handles validation.
            {
                let ann_lower = annotation_text.trim().to_ascii_lowercase();
                if ann_lower == "typealias"
                    || ann_lower.ends_with(".typealias")
                    || matches!(declared_type, InferredType::Named(ref n) if n == "ta")
                {
                    return None;
                }
            }

            // Skip dict literal assignments to TypedDict annotations.
            if let InferredType::Named(ref name) = declared_type {
                if cx.typeddict_names.contains(name.as_str()) {
                    let rhs_is_dict_literal = var
                        .rhs_span
                        .and_then(|sp| slice_span(cx.source, sp))
                        .is_some_and(|rhs| rhs.trim_start().starts_with('{'));
                    if rhs_is_dict_literal {
                        return None;
                    }
                }
            }

            let mut inferred_type = infer_with_literal_value(var, cx.source, &declared_type);

            // Substitute parameter type so SubtypeContext can do proper
            // MRO / protocol checking on the concrete Named type.
            if matches!(inferred_type, InferredType::Unknown) {
                if let Some(rhs_span) = var.rhs_span {
                    if let Some(rhs_text) = slice_span(cx.source, rhs_span) {
                        let rhs_name = rhs_text.trim();
                        if let Some(param_type) = cx.param_types.get(rhs_name) {
                            inferred_type = param_type.clone();
                        }
                    }
                }
            }
            // For Named declared types (protocols, classes), resolve
            // constructor calls `ClassName(...)` → Named(classname) so
            // SubtypeContext can check protocol conformance.  Guarded by
            // Named-only to avoid FPs with Callable/builtin targets.
            if matches!(inferred_type, InferredType::Unknown)
                && matches!(declared_type, InferredType::Named(_))
                && matches!(var.rhs_kind, basilisk_resolver::RhsKind::CallExpr)
            {
                if let Some(rhs_span) = var.rhs_span {
                    if let Some(rhs_text) = slice_span(cx.source, rhs_span) {
                        if let Some(class_name) =
                            crate::rules::e0137::helpers::extract_constructor_name(rhs_text.trim())
                        {
                            inferred_type = InferredType::Named(class_name.to_ascii_lowercase());
                        }
                    }
                }
            }

            // Use SubtypeContext for proper subtype checking (MRO, protocols).
            if is_subtype_with_context(&inferred_type, &declared_type, cx.subtype_ctx) {
                return None;
            }

            // Suppress when either type contains a Named component we cannot
            // resolve (type alias, imported type, forward ref).
            if has_unresolvable_named(&inferred_type, cx.subtype_ctx)
                || has_unresolvable_named(&declared_type, cx.subtype_ctx)
            {
                return None;
            }

            Some((
                var,
                annotation_text.to_owned(),
                inferred_type,
                declared_type,
            ))
        })
        .for_each(|(var, annotation, inferred, declared)| {
            diagnostics.push(make_diagnostic(
                var,
                &annotation,
                &inferred,
                &declared,
                cx.path,
            ));
        });
}

/// Check local variables in function bodies for type mismatches.
fn check_local_vars(
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
    typeddict_names: &std::collections::HashSet<String>,
    subtype_ctx: &SubtypeContext<'_>,
) {
    let source = &module.source;
    for func in &module.functions {
        let param_types = build_param_type_map(&func.parameters, source);
        let cx = CheckVarsCtx {
            source,
            path: &module.path,
            param_types: &param_types,
            typeddict_names,
            functions: &module.functions,
            subtype_ctx,
        };
        check_vars(&func.local_vars, diagnostics, &cx);
    }
}

/// Build a map from parameter name to its declared `InferredType`.
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
    }
}

/// Extract the annotation text from the source line containing `name_span`.
fn extract_annotation(source: &str, name_span: Span) -> Option<&str> {
    let start = usize::try_from(name_span.start).ok()?;
    let line_start = source.get(..start)?.rfind('\n').map_or(0, |pos| pos + 1);
    let line_end = source
        .get(start..)?
        .find('\n')
        .map_or(source.len(), |pos| start + pos);

    let line = source.get(line_start..line_end)?;
    let name_offset = start.checked_sub(line_start)?;
    let colon_pos = line.get(name_offset..)?.find(": ")? + name_offset;
    let after_colon = colon_pos + 2;

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

/// Extract the class name from a constructor call expression like `ClassName()`.
///
/// Returns `true` if `ty` contains a `Named` type that is not a known class
/// or builtin.  Such types (type aliases, imports, forward refs) cannot be
/// resolved — flagging a mismatch would be a potential false positive.
fn has_unresolvable_named(ty: &InferredType, ctx: &SubtypeContext<'_>) -> bool {
    match ty {
        InferredType::Named(name) => {
            // Strip type parameters before checking (e.g. "proto5[any]" → "proto5").
            let base = name.split('[').next().unwrap_or(name);
            !ctx.is_name_known(base)
        }
        InferredType::Union(types) => types.iter().any(|t| has_unresolvable_named(t, ctx)),
        InferredType::Optional(inner)
        | InferredType::List(inner)
        | InferredType::Set(inner)
        | InferredType::TypeForm(inner) => has_unresolvable_named(inner, ctx),
        InferredType::Dict(k, v) => {
            has_unresolvable_named(k, ctx) || has_unresolvable_named(v, ctx)
        }
        InferredType::Tuple(elems) => elems.iter().any(|e| has_unresolvable_named(e, ctx)),
        InferredType::Callable(info) => {
            has_unresolvable_named(&info.return_type, ctx)
                || info
                    .param_types
                    .iter()
                    .any(|p| has_unresolvable_named(p, ctx))
        }
        _ => false,
    }
}
