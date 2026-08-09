//! Implements [`constructors_call_init`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Helper functions for `constructors_call_init`: Constructor call errors.

use basilisk_resolver::{assignable, ClassInfo, ResolvedModule, TypeNode};
use ruff_python_ast::Expr;

use crate::rules::shared::ExprIndex;
use crate::span_util::{node_message_text, node_span, slice_span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

/// Error code for `constructors_call_init` diagnostics.
pub(super) const CODE: ErrorCode = ErrorCode {
    code: "constructors_call_init",
    docs_url: "https://www.basilisk-python.dev/errors/constructors_call_init",
};

// ##########################################################################
// # DELETED AND GONE — `all_base_names`, `has_custom_init_in_bases`,       #
// # `custom_init_walk`, `has_unresolved_base`. NO PANIC SHELLS: their only #
// # caller (`check_no_init_with_args`) was deleted too, so there are no    #
// # call sites left to keep visible. DO NOT RECREATE ANY OF THEM.          #
// #                                                                        #
// # All four derived a base class's identity from its SOURCE TEXT:         #
// #                                                                        #
// #   b.split('[').next().unwrap_or(b.as_str())   — base head by bracket   #
// #   base_name == "object"                       — top type by spelling   #
// #   base_name != "object" && !class_map.contains_key(base_name)          #
// #   method_map.contains_key(&(base_name, "__init__"))                    #
// #                                                                        #
// # A base written `Base [T]`, reached under an alias, or sharing a        #
// # rendered name with an unrelated class produced the wrong answer every  #
// # time. Whether a class inherits a constructor is a question about       #
// # RESOLVED class symbols — rebuild it on the binding table, in one       #
// # place, not as a fourth copy of a base-name string walk.                #
// #                                                                        #
// # Pinned by: tests/no_type_spelling_surgery_tests.rs                     #
// ##########################################################################

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
    _self_annotation: &str,
    _class_name: &str,
    _type_args: &[String],
    _call: &ruff_python_ast::ExprCall,
    _path: &str,
    _class_info: &basilisk_resolver::ClassInfo,
    _typevar_names: &[&str],
    _diagnostics: &mut Vec<Diagnostic>,
) {
    // ######################################################################
    // # DELETED BODY. DO NOT RESTORE IT AND DO NOT RETURN WITHOUT          #
    // # CHECKING IN ITS PLACE.                                             #
    // #                                                                    #
    // # It was a HAND-WRITTEN PARSER over the `self` annotation's text:    #
    // #                                                                    #
    // #   let bracket_start = self_annotation.find('[')?;                  #
    // #   let bracket_end   = self_annotation.rfind(']')?;                 #
    // #   let ann_class_name = self_annotation[..bracket_start].trim();    #
    // #   let args = self_annotation[bracket_start+1..bracket_end]         #
    // #                  .split(',').map(str::trim);                       #
    // #                                                                    #
    // # Character offsets, a bare `split(',')` that cannot see nesting (so #
    // # `Class[dict[str, int]]` was read as two arguments), and class      #
    // # identity by rendered equality. `ruff_python_parser` already        #
    // # produces this: an `Expr::Subscript` with a `value` and a `slice`.  #
    // #                                                                    #
    // # Pinned by: tests/no_type_spelling_surgery_tests.rs                 #
    // ######################################################################
    panic!(
        "basilisk-checker: `check_self_param_init_mismatch` was DELETED because it \
         hand-parsed the `self` annotation from TEXT — `find('[')`, `rfind(']')`, \
         slicing by character offset, and a nesting-blind `split(',')`. It panics \
         because the real implementation — reading the annotation's `Expr::Subscript` \
         and resolving its slice through the binding table — DOES NOT EXIST YET. Do \
         not restore the parser and do not return without checking in its place."
    )
}
