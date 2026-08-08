//! Implements [`constructors_call_new`] from [CHKARCH-DIAG-OPTIONAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OPTIONAL
//! `constructors_call_new`: Constructor call type mismatch with specialized generic class.
//!
//! When a generic class is called with explicit type arguments (e.g.
//! `Class1[int](1.0)`), Basilisk substitutes the type parameters into the
//! `__new__` method signature and checks that the provided arguments are
//! compatible.
//!
//! This rule covers two cases:
//!
//! 1. **Argument type mismatch after substitution**: The `__new__` method has
//!    a parameter typed with a type variable (e.g. `x: T`), and after
//!    substituting the type argument (e.g. `T=int`), the provided argument
//!    is incompatible (e.g. `1.0` is `float`, not `int`).
//!
//! 2. **Explicit `cls` parameter type mismatch**: The `__new__` method has an
//!    explicitly typed `cls` parameter (e.g. `cls: type[Class11[int]]`),
//!    and the class is called with different type arguments (e.g.
//!    `Class11[str]()`).
//!
//! ```python
//! class Class1(Generic[T]):
//!     def __new__(cls, x: T) -> Self:
//!         return super().__new__(cls)
//!
//! Class1[int](1.0)  # E: float is not compatible with int
//! ```
//!
//! Verdicts come from the resolved AST: type arguments and annotations are
//! lowered through the module's binding table and related semantically, so a
//! relation the layer cannot decide abstains instead of guessing. Source text
//! appears only inside diagnostic messages.

use std::collections::HashMap;

use basilisk_canonical::{assignable, equivalent, TypeNode, TypingForm};
use basilisk_resolver::{ResolvedModule, Span};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::parse_module;
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "constructors_call_new",
    docs_url: "https://www.basilisk-python.dev/errors/constructors_call_new",
};

/// Emits `constructors_call_new` for constructor calls on specialized generic classes where
/// the provided arguments are incompatible with the substituted parameter types.
pub(crate) struct ConstructorCallNewMismatch;

impl Rule for ConstructorCallNewMismatch {
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
        let Some(parsed) = parse_module(module) else {
            return;
        };
        let Some(oracle) = types.oracle() else {
            return;
        };
        let class_map: HashMap<&str, &basilisk_resolver::ClassInfo> =
            basilisk_resolver::name_lookup(&module.classes);
        for call in oracle.calls() {
            check_specialized_constructor_call(call, module, &parsed.ast, &class_map, diagnostics);
        }
    }
}

/// Check a single call expression to see if it is a specialized generic
/// constructor call with mismatched arguments.
fn check_specialized_constructor_call(
    call: &ast::ExprCall,
    module: &ResolvedModule,
    ast: &ast::ModModule,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // The callee must be `ClassName[TypeArgs]` naming a generic class.
    let Expr::Subscript(sub) = call.func.as_ref() else {
        return;
    };
    let Expr::Name(class_name_node) = sub.value.as_ref() else {
        return;
    };
    let class_name = class_name_node.id.as_str();
    let Some(class_info) = class_map.get(class_name) else {
        return;
    };
    if class_info.generic_params.is_empty() {
        return;
    }
    let type_args = type_argument_exprs(&sub.slice);
    let arg_nodes: Vec<TypeNode> = type_args
        .iter()
        .map(|expr| TypeNode::lower(&module.bindings, expr))
        .collect();
    for new_def in new_methods(ast, class_name) {
        check_cls_annotation(new_def, call, class_name, class_info, &type_args, module, diagnostics);
        check_value_args(new_def, call, class_name, class_info, &type_args, &arg_nodes, module, diagnostics);
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

/// Every `__new__` definition in the module-level class named `class_name`.
fn new_methods<'a>(ast: &'a ast::ModModule, class_name: &str) -> Vec<&'a ast::StmtFunctionDef> {
    ast.body
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::ClassDef(class) if class.name.as_str() == class_name => Some(class),
            _ => None,
        })
        .flat_map(|class| {
            class.body.iter().filter_map(|stmt| match stmt {
                Stmt::FunctionDef(function) if function.name.as_str() == "__new__" => {
                    Some(function)
                }
                _ => None,
            })
        })
        .collect()
}

/// The non-`cls` parameters of a `__new__` definition, in call order.
fn value_parameters(new_def: &ast::StmtFunctionDef) -> Vec<&ast::ParameterWithDefault> {
    new_def
        .parameters
        .posonlyargs
        .iter()
        .chain(new_def.parameters.args.iter())
        .skip(1)
        .collect()
}

/// Case 1: relate each literal call argument to the parameter's annotation
/// after substituting the class's type parameters with the call's type
/// arguments.
#[expect(
    clippy::too_many_arguments,
    reason = "diagnostic context requires many parameters"
)]
fn check_value_args(
    new_def: &ast::StmtFunctionDef,
    call: &ast::ExprCall,
    class_name: &str,
    class_info: &basilisk_resolver::ClassInfo,
    type_args: &[&Expr],
    arg_nodes: &[TypeNode],
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // `*args` passthrough accepts anything this rule can verify.
    if new_def.parameters.vararg.is_some() {
        return;
    }
    let params = value_parameters(new_def);
    for (arg_expr, param) in call.arguments.args.iter().zip(params) {
        let Some(annotation) = param.parameter.annotation.as_deref() else {
            continue;
        };
        let (target, target_expr) =
            substituted_annotation(module, annotation, class_info, type_args, arg_nodes);
        if assignable(&TypeNode::of_literal_expr(arg_expr), &target) == Some(false) {
            push_value_arg_diagnostic(
                arg_expr,
                &param.parameter.name,
                target_expr,
                class_name,
                type_args,
                module,
                diagnostics,
            );
        }
    }
}

/// The parameter's target type: the matching call type argument when the
/// annotation names one of the class's own type parameters, otherwise the
/// annotation itself, lowered. Also returns the expression that names the
/// target, for the diagnostic message.
fn substituted_annotation<'a>(
    module: &ResolvedModule,
    annotation: &'a Expr,
    class_info: &basilisk_resolver::ClassInfo,
    type_args: &[&'a Expr],
    arg_nodes: &[TypeNode],
) -> (TypeNode, &'a Expr) {
    if let Expr::Name(name) = annotation {
        let position = class_info
            .generic_params
            .iter()
            .position(|param| param.name == name.id.as_str());
        if let Some(index) = position {
            if let (Some(node), Some(expr)) = (arg_nodes.get(index), type_args.get(index)) {
                return (node.clone(), expr);
            }
        }
    }
    (TypeNode::lower(&module.bindings, annotation), annotation)
}

/// Emit the case-1 diagnostic. Source text is rendered for the message only.
#[expect(
    clippy::too_many_arguments,
    reason = "diagnostic context requires many parameters"
)]
fn push_value_arg_diagnostic(
    arg_expr: &Expr,
    param_name: &str,
    target_expr: &Expr,
    class_name: &str,
    type_args: &[&Expr],
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let arg_text = expr_text(&module.source, arg_expr);
    let target_text = expr_text(&module.source, target_expr);
    let args_text: Vec<&str> = type_args
        .iter()
        .map(|expr| expr_text(&module.source, expr))
        .collect();
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Argument `{arg_text}` is incompatible with parameter `{param_name}` \
             of type `{target_text}` in `{class_name}.__new__`"
        ),
        expr_span(arg_expr),
        &module.path,
        Some(format!(
            "Pass a value of type `{target_text}` for parameter `{param_name}`"
        )),
        Some(format!(
            "`{class_name}` is specialized with type arguments `[{}]`",
            args_text.join(", ")
        )),
    ));
}

/// Case 2: an explicitly annotated `cls: type[Class[FixedArgs]]` constrains
/// the type arguments a specialized call may supply.
#[expect(
    clippy::too_many_arguments,
    reason = "diagnostic context requires many parameters"
)]
fn check_cls_annotation(
    new_def: &ast::StmtFunctionDef,
    call: &ast::ExprCall,
    class_name: &str,
    class_info: &basilisk_resolver::ClassInfo,
    type_args: &[&Expr],
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(inner_sub) = cls_constraint(new_def, class_name, &module.bindings) else {
        return;
    };
    let ann_args = type_argument_exprs(&inner_sub.slice);
    // A type parameter among the fixed arguments unifies with the call's
    // arguments instead of constraining them.
    let names_own_param = ann_args.iter().any(|expr| {
        matches!(expr, Expr::Name(name)
            if class_info.generic_params.iter().any(|p| p.name == name.id.as_str()))
    });
    if names_own_param || ann_args.len() != type_args.len() {
        return;
    }
    let mismatch = type_args.iter().zip(ann_args.iter()).any(|(provided, expected)| {
        let provided_node = TypeNode::lower(&module.bindings, provided);
        let expected_node = TypeNode::lower(&module.bindings, expected);
        equivalent(&provided_node, &expected_node) == Some(false)
    });
    if mismatch {
        push_cls_diagnostic(call, class_name, type_args, &ann_args, module, diagnostics);
    }
}

/// The `Class[FixedArgs]` subscript inside `cls: type[Class[FixedArgs]]`,
/// when the annotation's base resolves to the builtin `type` class and the
/// inner name is this class.
fn cls_constraint<'a>(
    new_def: &'a ast::StmtFunctionDef,
    class_name: &str,
    bindings: &basilisk_canonical::BindingTable,
) -> Option<&'a ast::ExprSubscript> {
    let cls_param = new_def
        .parameters
        .posonlyargs
        .iter()
        .chain(new_def.parameters.args.iter())
        .next()?;
    let Expr::Subscript(type_sub) = cls_param.parameter.annotation.as_deref()? else {
        return None;
    };
    let base_form = bindings.form_of_with_builtins(&type_sub.value);
    if !matches!(
        base_form,
        Some(TypingForm::TypeClass | TypingForm::TypeAliasBuiltin)
    ) {
        return None;
    }
    let Expr::Subscript(inner_sub) = type_sub.slice.as_ref() else {
        return None;
    };
    matches!(inner_sub.value.as_ref(), Expr::Name(name) if name.id.as_str() == class_name)
        .then_some(inner_sub)
}

/// Emit the case-2 diagnostic. Source text is rendered for the message only.
fn push_cls_diagnostic(
    call: &ast::ExprCall,
    class_name: &str,
    type_args: &[&Expr],
    ann_args: &[&Expr],
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let provided: Vec<&str> = type_args
        .iter()
        .map(|expr| expr_text(&module.source, expr))
        .collect();
    let expected: Vec<&str> = ann_args
        .iter()
        .map(|expr| expr_text(&module.source, expr))
        .collect();
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "`{class_name}[{}]()` is incompatible: `__new__` constrains `cls` to \
             `type[{class_name}[{}]]`",
            provided.join(", "),
            expected.join(", ")
        ),
        expr_span(call),
        &module.path,
        Some(format!(
            "Use `{class_name}[{}]()` to match the expected `cls` parameter type",
            expected.join(", ")
        )),
        Some(format!(
            "The `__new__` method constrains `cls` to `type[{class_name}[{}]]`",
            expected.join(", ")
        )),
    ));
}

/// The source text of an expression, for diagnostic MESSAGES only — never a
/// verdict.
fn expr_text<'a>(source: &'a str, expr: &impl Ranged) -> &'a str {
    slice_span(source, expr_span(expr)).unwrap_or("<expression>").trim()
}

/// The diagnostic span of an expression.
fn expr_span(expr: &impl Ranged) -> Span {
    let range = expr.range();
    Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    }
}
