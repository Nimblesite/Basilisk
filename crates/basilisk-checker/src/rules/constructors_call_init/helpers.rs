//! Implements [`constructors_call_init`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Helper functions for `constructors_call_init`: Constructor call errors.

use std::collections::HashMap;

use basilisk_resolver::ClassInfo;

use basilisk_resolver::{assignable, ResolvedModule, Span, TypeNode};
use ruff_python_ast::Expr;

use crate::rules::shared::ExprIndex;
use crate::span_util::{node_message_text, node_span, slice_span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

/// Error code for `constructors_call_init` diagnostics.
pub(super) const CODE: ErrorCode = ErrorCode {
    code: "constructors_call_init",
    docs_url: "https://www.basilisk-python.dev/errors/constructors_call_init",
};

/// Collect all base class names (simple and subscripted) for a class.
pub(super) fn all_base_names(class_info: &ClassInfo) -> Vec<&str> {
    let mut names: Vec<&str> = class_info
        .bases
        .iter()
        .map(|b| b.split('[').next().unwrap_or(b.as_str()))
        .collect();
    for entry in &class_info.base_subscripts {
        let name = entry.base_name.as_str();
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// Recursively check if any base class defines `__init__` or `__new__`.
pub(super) fn has_custom_init_in_bases(
    class_info: &ClassInfo,
    class_map: &HashMap<&str, &ClassInfo>,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
) -> bool {
    let mut visited = std::collections::HashSet::new();
    let _ = visited.insert(class_info.name.as_str());
    custom_init_walk(class_info, class_map, method_map, &mut visited)
}

/// Recursive body of [`has_custom_init_in_bases`]; `visited` breaks base-name
/// cycles (GitHub #278).
fn custom_init_walk<'a>(
    class_info: &'a ClassInfo,
    class_map: &HashMap<&str, &'a ClassInfo>,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    visited: &mut std::collections::HashSet<&'a str>,
) -> bool {
    for base_name in all_base_names(class_info) {
        if base_name == "object" {
            continue;
        }

        // Check if the base class itself defines __init__ or __new__.
        if method_map.contains_key(&(base_name, "__init__"))
            || method_map.contains_key(&(base_name, "__new__"))
        {
            return true;
        }

        // Recurse into the base's bases.
        if visited.insert(base_name) {
            if let Some(base_class) = class_map.get(base_name) {
                if custom_init_walk(base_class, class_map, method_map, visited) {
                    return true;
                }
            }
        }
    }
    false
}

/// Returns `true` if the class has a base the checker cannot resolve to a known
/// definition — i.e. a base that is not `object` and not a class defined in this
/// module.
///
/// Such a base is an external import (e.g. pydantic `BaseModel`, attrs, msgspec)
/// that may provide an argument-accepting constructor we cannot see. Callers
/// must therefore not conclude the class "inherits only from `object`".
pub(super) fn has_unresolved_base(
    class_info: &ClassInfo,
    class_map: &HashMap<&str, &ClassInfo>,
) -> bool {
    all_base_names(class_info)
        .into_iter()
        .any(|base_name| base_name != "object" && !class_map.contains_key(base_name))
}

/// Resolve a string annotation by stripping surrounding quotes.
pub(super) fn resolve_string_annotation(annotation: &str) -> String {
    if (annotation.starts_with('"') && annotation.ends_with('"'))
        || (annotation.starts_with('\'') && annotation.ends_with('\''))
    {
        annotation
            .get(1..annotation.len().saturating_sub(1))
            .unwrap_or(annotation)
            .to_owned()
    } else {
        annotation.to_owned()
    }
}

/// The element expressions of a subscript slice: a tuple's elements, or the
/// single expression itself.
fn type_argument_exprs(slice: &Expr) -> Vec<&Expr> {
    match slice {
        Expr::Tuple(tuple) => tuple.elts.iter().collect(),
        single => vec![single],
    }
}

/// Check arguments to a specialized `__init__` call, `Class[Args](values…)`.
///
/// Each literal argument is related to the parameter's annotation after
/// substituting the class's type parameters with the call's type arguments —
/// annotations and type arguments are lowered through the module's binding
/// table and related with [`assignable`] ([ASTREBUILD-LAW]). A relation the
/// layer cannot decide abstains instead of guessing; source text appears in
/// diagnostic messages only.
pub(super) fn check_init_method_args(
    init_func: &basilisk_resolver::FunctionInfo,
    call: &ruff_python_ast::ExprCall,
    class_name: &str,
    class_info: &basilisk_resolver::ClassInfo,
    typevar_names: &[&str],
    module: &ResolvedModule,
    index: &ExprIndex<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // The callee is `ClassName[TypeArgs]`; the type arguments come from the
    // subscript slice's AST nodes.
    let Expr::Subscript(sub) = call.func.as_ref() else {
        return;
    };
    let type_arg_exprs = type_argument_exprs(&sub.slice);
    let arg_nodes: Vec<TypeNode> = type_arg_exprs
        .iter()
        .map(|expr| TypeNode::lower(&module.bindings, expr))
        .collect();
    // Rendered for messages and the pre-existing `self`-annotation check only.
    let type_args: Vec<String> = type_arg_exprs
        .iter()
        .map(|expr| node_message_text(&module.source, *expr).trim().to_owned())
        .collect();

    // Check 3: Explicit self annotation mismatch.
    if let Some(self_param) = init_func.parameters.first() {
        if let Some(ann_span) = self_param.annotation_span {
            if let Some(ann_text) = slice_span(&module.source, ann_span) {
                let resolved = resolve_string_annotation(ann_text.trim());
                check_self_param_init_mismatch(
                    &resolved,
                    class_name,
                    &type_args,
                    call,
                    &module.path,
                    class_info,
                    typevar_names,
                    diagnostics,
                );
            }
        }
    }

    // Check 1: Non-self parameters for type mismatch after substitution.
    let non_self_params: Vec<&basilisk_resolver::ParameterInfo> =
        init_func.parameters.iter().skip(1).collect();

    // If __init__ accepts *args/**kwargs (passthrough), skip argument checking.
    if init_func.vararg.is_some() {
        return;
    }

    for (arg_idx, arg_expr) in call.arguments.args.iter().enumerate() {
        let Some(param) = non_self_params.get(arg_idx) else {
            break;
        };
        let Some(ann_span) = param.annotation_span else {
            continue;
        };
        // Annotations the index cannot map to a node (e.g. string forward
        // references) abstain ([ASTREBUILD-PHASE-RESOLVER]).
        let Some(ann_expr) = index.expr(ann_span) else {
            continue;
        };
        let (target, target_expr) =
            substituted_annotation(module, ann_expr, class_info, &type_arg_exprs, &arg_nodes);
        if assignable(&TypeNode::of_literal_expr(arg_expr), &target) == Some(false) {
            let arg_text = node_message_text(&module.source, arg_expr);
            let target_text = node_message_text(&module.source, target_expr);
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Argument `{arg_text}` is incompatible with parameter `{}` \
                     of type `{target_text}` in `{class_name}.__init__`",
                    param.name
                ),
                node_span(arg_expr),
                &module.path,
                Some(format!(
                    "Pass a value of type `{target_text}` for parameter `{}`",
                    param.name
                )),
                Some(format!(
                    "`{class_name}` is specialized with type arguments `[{}]`",
                    type_args.join(", ")
                )),
            ));
        }
    }
}

/// The parameter's target type: the matching call type argument when the
/// annotation names one of the class's own type parameters, otherwise the
/// annotation itself, lowered. A composite annotation containing a type
/// parameter (`T | None`) lowers its unresolvable leaves to `Unknown`, on
/// which the relation abstains. Also returns the expression that names the
/// target, for the diagnostic message.
fn substituted_annotation<'a>(
    module: &ResolvedModule,
    annotation: &'a Expr,
    class_info: &ClassInfo,
    type_args: &[&'a Expr],
    arg_nodes: &[TypeNode],
) -> (TypeNode, &'a Expr) {
    if let Expr::Name(name) = annotation {
        let position = class_info
            .generic_params
            .iter()
            .position(|param| param.name == name.id.as_str());
        if let Some(idx) = position {
            if let (Some(node), Some(expr)) = (arg_nodes.get(idx), type_args.get(idx)) {
                return (node.clone(), expr);
            }
        }
    }
    (TypeNode::lower(&module.bindings, annotation), annotation)
}

/// Check if the `self` parameter annotation in `__init__` is incompatible with
/// the provided type arguments.
#[expect(
    clippy::too_many_arguments,
    reason = "all args needed for mismatch check"
)]
pub(super) fn check_self_param_init_mismatch(
    self_annotation: &str,
    class_name: &str,
    type_args: &[String],
    call: &ruff_python_ast::ExprCall,
    path: &str,
    class_info: &basilisk_resolver::ClassInfo,
    typevar_names: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_text_size::Ranged as _;

    // Looking for annotations like "Class4[int]"
    let Some(bracket_start) = self_annotation.find('[') else {
        return;
    };
    let Some(bracket_end) = self_annotation.rfind(']') else {
        return;
    };

    let ann_class_name = self_annotation[..bracket_start].trim();
    if ann_class_name != class_name {
        return;
    }

    let args_str = &self_annotation[bracket_start + 1..bracket_end];
    let ann_type_args: Vec<&str> = args_str.split(',').map(str::trim).collect();

    // Check if annotation args contain class-scoped or function-scoped type vars.
    let generic_param_names: Vec<&str> =
        basilisk_resolver::collect_names(&class_info.generic_params);

    // If all annotation args are fixed (not type variables), check for mismatch.
    let all_fixed = ann_type_args
        .iter()
        .all(|arg| !generic_param_names.contains(arg) && !typevar_names.contains(arg));

    if !all_fixed {
        return;
    }

    // The annotation has fixed type args (e.g. `Class4[int]`).
    if type_args.len() != ann_type_args.len() {
        return;
    }

    let all_match = type_args
        .iter()
        .zip(ann_type_args.iter())
        .all(|(provided, expected)| provided.as_str() == *expected);

    if !all_match {
        let range = call.range();
        let span = Span {
            start: range.start().to_u32(),
            end: range.end().to_u32(),
        };
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "`{class_name}[{}]()` is incompatible: `__init__` expects \
                 `self: {self_annotation}` but received `{class_name}[{}]`",
                type_args.join(", "),
                type_args.join(", ")
            ),
            span,
            path,
            Some(format!(
                "Use `{class_name}[{}]()` to match the expected `self` parameter type",
                ann_type_args.join(", ")
            )),
            Some(format!(
                "The `__init__` method constrains `self` to `{self_annotation}`"
            )),
        ));
    }
}
