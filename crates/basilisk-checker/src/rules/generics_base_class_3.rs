//! Implements [`generics_base_class_3`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `generics_base_class_3`: Invariant generic type mismatch at call site.
//!
//! When a function parameter expects a parameterised generic like
//! `dict[str, list[object]]` and a subclass whose base parameterisation
//! differs in an invariant position is passed, the call is invalid.
//!
//! ```python
//! class SymbolTable(dict[str, list[int]]): ...
//!
//! def takes(x: dict[str, list[object]]): ...
//!
//! def test(s: SymbolTable):
//!     takes(s)  # E -- list is invariant, list[int] != list[object]
//! ```
//!
//! Verdicts come from the resolved semantic model ([ASTREBUILD-LAW]): the
//! subclass's base annotation and the parameter annotation are lowered
//! through the module's binding table to [`TypeNode`]s and related with
//! [`assignable`], whose builtin-container variance rules decide the
//! mismatch. A relation the layer cannot decide (user classes among the type
//! arguments, unresolved names) abstains and no diagnostic is emitted.
//! Source text appears only inside diagnostic messages.

use std::collections::HashMap;

use basilisk_resolver::{assignable, ResolvedModule, Span, TypeNode};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "generics_base_class_3",
    docs_url: "https://www.basilisk-python.dev/errors/generics_base_class_3",
};

/// Emits `generics_base_class_3` for calls where a subclass argument is incompatible
/// with a parameterised generic parameter due to invariance.
pub(crate) struct InvariantGenericArgMismatch;

impl Rule for InvariantGenericArgMismatch {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };

        // Step 1: class name -> its parameterised builtin-generic base.
        let class_bases = collect_class_bases(module, &parsed.ast.body);
        if class_bases.is_empty() {
            return;
        }

        // Step 2: module-level function name -> positional parameters.
        let func_params = collect_function_params(&parsed.ast.body);

        // Step 3: walk function bodies for calls.
        for stmt in &parsed.ast.body {
            visit_stmt(stmt, module, &class_bases, &func_params, diagnostics);
        }
    }
}

/// A class's parameterised base: the lowered node for verdicts and the base
/// expression for diagnostic messages.
struct ClassBase<'a> {
    node: TypeNode,
    expr: &'a Expr,
}

/// Map each module-level class to its base whose lowered form is a
/// parameterised builtin generic (`dict[...]`, `list[...]`, ...). Bases the
/// binding table cannot resolve to a builtin generic lower to `Unknown` and
/// are skipped — the relation would abstain on them anyway.
fn collect_class_bases<'a>(
    module: &ResolvedModule,
    stmts: &'a [Stmt],
) -> HashMap<&'a str, ClassBase<'a>> {
    let mut map = HashMap::new();
    for stmt in stmts {
        let Stmt::ClassDef(class) = stmt else {
            continue;
        };
        let Some(arguments) = class.arguments.as_deref() else {
            continue;
        };
        for base in &arguments.args {
            let node = TypeNode::lower(&module.bindings, base);
            if matches!(
                &node,
                TypeNode::Subscript { base, .. } if matches!(base.as_ref(), TypeNode::Builtin(_))
            ) {
                let _ = map.insert(class.name.as_str(), ClassBase { node, expr: base });
            }
        }
    }
    map
}

/// The positional parameters of every module-level function: name and
/// optional annotation expression, in call order.
fn collect_function_params(
    stmts: &[Stmt],
) -> HashMap<&str, Vec<(&str, Option<&Expr>)>> {
    let mut map = HashMap::new();
    for stmt in stmts {
        let Stmt::FunctionDef(func) = stmt else {
            continue;
        };
        let params: Vec<(&str, Option<&Expr>)> = positional_params(func)
            .map(|param| {
                (
                    param.parameter.name.as_str(),
                    param.parameter.annotation.as_deref(),
                )
            })
            .collect();
        if !params.is_empty() {
            let _ = map.insert(func.name.as_str(), params);
        }
    }
    map
}

/// The positional parameters of a function definition, in call order.
fn positional_params(
    func: &ast::StmtFunctionDef,
) -> impl Iterator<Item = &ast::ParameterWithDefault> {
    func.parameters
        .posonlyargs
        .iter()
        .chain(func.parameters.args.iter())
}

/// Walk statements to find function definitions and check their bodies.
fn visit_stmt(
    stmt: &Stmt,
    module: &ResolvedModule,
    class_bases: &HashMap<&str, ClassBase<'_>>,
    func_params: &HashMap<&str, Vec<(&str, Option<&Expr>)>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Stmt::FunctionDef(func_def) = stmt {
        let caller_params = caller_param_annotations(func_def);
        for body_stmt in &func_def.body {
            check_body_stmt(
                body_stmt,
                module,
                class_bases,
                func_params,
                &caller_params,
                diagnostics,
            );
        }
    } else if let Stmt::ClassDef(cls) = stmt {
        for body_stmt in &cls.body {
            visit_stmt(body_stmt, module, class_bases, func_params, diagnostics);
        }
    }
}

/// Map the enclosing function's parameter names to their annotation
/// expressions.
fn caller_param_annotations(func_def: &ast::StmtFunctionDef) -> HashMap<&str, &Expr> {
    positional_params(func_def)
        .filter_map(|param| {
            param
                .parameter
                .annotation
                .as_deref()
                .map(|annotation| (param.parameter.name.as_str(), annotation))
        })
        .collect()
}

/// Check a statement inside a function body for calls with invariant
/// mismatches.
fn check_body_stmt(
    stmt: &Stmt,
    module: &ResolvedModule,
    class_bases: &HashMap<&str, ClassBase<'_>>,
    func_params: &HashMap<&str, Vec<(&str, Option<&Expr>)>>,
    caller_params: &HashMap<&str, &Expr>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let call = match stmt {
        Stmt::Expr(expr_stmt) => match expr_stmt.value.as_ref() {
            Expr::Call(call) => call,
            _ => return,
        },
        Stmt::Assign(assign) => match assign.value.as_ref() {
            Expr::Call(call) => call,
            _ => return,
        },
        _ => return,
    };

    let Expr::Name(callee) = call.func.as_ref() else {
        return;
    };
    let Some(callee_param_list) = func_params.get(callee.id.as_str()) else {
        return;
    };

    for (arg_idx, arg_expr) in call.arguments.args.iter().enumerate() {
        let Some(&(param_name, Some(param_ann))) = callee_param_list.get(arg_idx) else {
            continue;
        };
        let Expr::Name(arg_name) = arg_expr else {
            continue;
        };
        let Some(&arg_ann) = caller_params.get(arg_name.id.as_str()) else {
            continue;
        };
        // The argument's declared type must be a Name of a class whose base is
        // a parameterised builtin generic.
        let Expr::Name(arg_class) = arg_ann else {
            continue;
        };
        let Some(class_base) = class_bases.get(arg_class.id.as_str()) else {
            continue;
        };

        let target = TypeNode::lower(&module.bindings, param_ann);
        // Only relate parameterisations of the SAME builtin generic: the
        // class may have other bases this rule does not model, so a verdict
        // across different constructors would be unsound
        // ([ASTREBUILD-PHASE-RESOLVER]).
        if !same_builtin_base(&class_base.node, &target) {
            continue;
        }
        if assignable(&class_base.node, &target) == Some(false) {
            emit_diagnostic(
                call,
                callee.id.as_str(),
                param_name,
                param_ann,
                arg_class.id.as_str(),
                class_base.expr,
                module,
                diagnostics,
            );
            return;
        }
    }
}

/// `true` when both nodes are parameterisations of the same builtin class.
fn same_builtin_base(a: &TypeNode, b: &TypeNode) -> bool {
    match (a, b) {
        (
            TypeNode::Subscript { base: base_a, .. },
            TypeNode::Subscript { base: base_b, .. },
        ) => match (base_a.as_ref(), base_b.as_ref()) {
            (TypeNode::Builtin(class_a), TypeNode::Builtin(class_b)) => class_a == class_b,
            _ => false,
        },
        _ => false,
    }
}

/// Emit the invariant generic argument mismatch diagnostic. Source text is
/// rendered for the message only.
#[expect(
    clippy::too_many_arguments,
    reason = "diagnostic formatting requires all context"
)]
fn emit_diagnostic(
    call: &ast::ExprCall,
    callee_name: &str,
    param_name: &str,
    param_ann: &Expr,
    arg_class_name: &str,
    class_base_expr: &Expr,
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let param_text = expr_text(&module.source, param_ann);
    let base_text = expr_text(&module.source, class_base_expr);
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Argument `{param_name}` of `{callee_name}` expects `{param_text}` \
             but received `{arg_class_name}` (subclass of `{base_text}`) -- \
             the base parameterisation is incompatible in an invariant position"
        ),
        Span::from(call.range()),
        &module.path,
        Some(format!(
            "`{base_text}` is not assignable to `{param_text}`: mutable \
             containers are invariant in their type parameters"
        )),
        Some(
            "Mutable generic containers like `list`, `dict`, `set` \
             are invariant in their type parameters."
                .to_owned(),
        ),
    ));
}

/// The source text of an expression, for diagnostic MESSAGES only — never a
/// verdict.
fn expr_text<'a>(source: &'a str, expr: &impl Ranged) -> &'a str {
    slice_span(source, Span::from(expr.range()))
        .unwrap_or("<expression>")
        .trim()
}
