//! BSK-E0074: Constructor call type mismatch with specialized generic class.
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

use std::collections::HashMap;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0074",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0074",
};

/// Emits BSK-E0074 for constructor calls on specialized generic classes where
/// the provided arguments are incompatible with the substituted parameter types.
pub(crate) struct ConstructorCallNewMismatch;

impl Rule for ConstructorCallNewMismatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        // Build class info maps.
        let class_map: HashMap<&str, &basilisk_resolver::ClassInfo> = module
            .classes
            .iter()
            .map(|c| (c.name.as_str(), c))
            .collect();

        // Build method map: (class_name, method_name) -> Vec<&FunctionInfo>
        let mut method_map: HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>> =
            HashMap::new();
        for func in &module.functions {
            if let Some(ref class_name) = func.class_name {
                method_map
                    .entry((class_name.as_str(), func.name.as_str()))
                    .or_default()
                    .push(func);
            }
        }

        // Re-parse source to get AST for walking call expressions.
        let Ok(parsed) = basilisk_parser::parse_source(source.clone(), path.clone()) else {
            return;
        };

        for stmt in &parsed.ast.body {
            check_stmt_for_specialized_constructor_calls(
                stmt,
                source,
                path,
                &class_map,
                &method_map,
                diagnostics,
            );
        }
    }
}

/// Walk a statement looking for call expressions on specialized generic classes.
fn check_stmt_for_specialized_constructor_calls(
    stmt: &ruff_python_ast::Stmt,
    source: &str,
    path: &str,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Stmt;

    match stmt {
        Stmt::Expr(node) => {
            check_expr_recursive(
                &node.value,
                source,
                path,
                class_map,
                method_map,
                diagnostics,
            );
        }
        Stmt::Assign(node) => {
            check_expr_recursive(
                &node.value,
                source,
                path,
                class_map,
                method_map,
                diagnostics,
            );
        }
        Stmt::AnnAssign(node) => {
            if let Some(val) = node.value.as_deref() {
                check_expr_recursive(val, source, path, class_map, method_map, diagnostics);
            }
        }
        Stmt::Try(try_stmt) => {
            for body in [&try_stmt.body, &try_stmt.orelse, &try_stmt.finalbody] {
                for s in body {
                    check_stmt_for_specialized_constructor_calls(
                        s,
                        source,
                        path,
                        class_map,
                        method_map,
                        diagnostics,
                    );
                }
            }
            for handler in &try_stmt.handlers {
                let ruff_python_ast::ExceptHandler::ExceptHandler(h) = handler;
                for s in &h.body {
                    check_stmt_for_specialized_constructor_calls(
                        s,
                        source,
                        path,
                        class_map,
                        method_map,
                        diagnostics,
                    );
                }
            }
        }
        _ => {}
    }
}

/// Recursively check an expression and its sub-expressions for specialized
/// constructor calls.
fn check_expr_recursive(
    expr: &ruff_python_ast::Expr,
    source: &str,
    path: &str,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Expr;

    // Check this expression if it is a call.
    if let Expr::Call(call) = expr {
        // Recurse into arguments first.
        for arg in &call.arguments.args {
            check_expr_recursive(arg, source, path, class_map, method_map, diagnostics);
        }

        // Now check whether this call is a specialized constructor call.
        check_specialized_constructor_call(call, source, path, class_map, method_map, diagnostics);
    }
}

/// Check a single call expression to see if it is a specialized generic
/// constructor call with mismatched arguments.
fn check_specialized_constructor_call(
    call: &ruff_python_ast::ExprCall,
    source: &str,
    path: &str,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Expr;

    // The callee must be a subscript expression: ClassName[TypeArgs]
    let Expr::Subscript(sub) = call.func.as_ref() else {
        return;
    };

    // The subscript value must be a simple class name.
    let Expr::Name(class_name_node) = sub.value.as_ref() else {
        return;
    };
    let class_name = class_name_node.id.as_str();

    // Look up the class.
    let Some(class_info) = class_map.get(class_name) else {
        return;
    };

    // The class must be generic.
    if class_info.generic_params.is_empty() {
        return;
    }

    // Extract type arguments from the subscript.
    let type_args = extract_type_args_text(&sub.slice, source);

    // Build substitution map: type_param_name -> type_arg_text
    let mut substitutions: HashMap<&str, &str> = HashMap::new();
    for (idx, param) in class_info.generic_params.iter().enumerate() {
        if let Some(arg) = type_args.get(idx) {
            let _ = substitutions.insert(param.name.as_str(), arg.as_str());
        }
    }

    // Look up the __new__ method.
    if let Some(new_funcs) = method_map.get(&(class_name, "__new__")) {
        for new_func in new_funcs {
            check_new_method_args(
                new_func,
                &substitutions,
                call,
                class_name,
                &type_args,
                source,
                path,
                class_info,
                diagnostics,
            );
        }
    }
}

/// Extract type argument texts from a subscript slice expression.
fn extract_type_args_text(slice: &ruff_python_ast::Expr, source: &str) -> Vec<String> {
    use ruff_python_ast::Expr;
    use ruff_text_size::Ranged as _;

    match slice {
        Expr::Tuple(tuple) => tuple
            .elts
            .iter()
            .map(|e| {
                let range = e.range();
                source
                    .get(range.start().to_usize()..range.end().to_usize())
                    .unwrap_or("")
                    .trim()
                    .to_owned()
            })
            .collect(),
        other => {
            let range = other.range();
            vec![source
                .get(range.start().to_usize()..range.end().to_usize())
                .unwrap_or("")
                .trim()
                .to_owned()]
        }
    }
}

/// Check whether the arguments to a `__new__` method are compatible after
/// type parameter substitution.
#[expect(clippy::too_many_arguments, reason = "diagnostic context requires many parameters")]
fn check_new_method_args(
    new_func: &basilisk_resolver::FunctionInfo,
    substitutions: &HashMap<&str, &str>,
    call: &ruff_python_ast::ExprCall,
    class_name: &str,
    type_args: &[String],
    source: &str,
    path: &str,
    class_info: &basilisk_resolver::ClassInfo,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_text_size::Ranged as _;

    // Check the cls parameter for explicit type annotation mismatch (Case 2).
    if let Some(cls_param) = new_func.parameters.first() {
        if let Some(ann_span) = cls_param.annotation_span {
            if let Some(ann_text) = slice_span(source, ann_span) {
                // Resolve string annotations (quoted type expressions).
                let resolved_ann = resolve_string_annotation(ann_text.trim());
                check_cls_param_mismatch(
                    &resolved_ann,
                    class_name,
                    type_args,
                    call,
                    path,
                    class_info,
                    diagnostics,
                );
            }
        }
    }

    // Check non-cls parameters (skip first param which is cls) for type mismatch (Case 1).
    let non_cls_params: Vec<&basilisk_resolver::ParameterInfo> =
        new_func.parameters.iter().skip(1).collect();

    // If __new__ accepts *args/**kwargs (passthrough), skip argument checking
    // for the non-cls parameters.
    if new_func.vararg.is_some() {
        return;
    }

    for (arg_idx, arg_expr) in call.arguments.args.iter().enumerate() {
        let Some(param) = non_cls_params.get(arg_idx) else {
            break;
        };

        let Some(ann_span) = param.annotation_span else {
            continue;
        };
        let Some(ann_text) = slice_span(source, ann_span) else {
            continue;
        };

        let ann_trimmed = ann_text.trim();

        // Substitute type parameters in the annotation.
        let resolved_type = if let Some(replacement) = substitutions.get(ann_trimmed) {
            (*replacement).to_owned()
        } else {
            ann_trimmed.to_owned()
        };

        // Classify the argument expression type.
        let Some(arg_type) = classify_literal_type(arg_expr) else {
            continue;
        };

        // Check compatibility.
        if !is_type_compatible(arg_type, &resolved_type) {
            let range = arg_expr.range();
            let span = Span {
                start: range.start().to_u32(),
                end: range.end().to_u32(),
            };
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Argument type `{arg_type}` is incompatible with parameter `{}` \
                     of type `{resolved_type}` in `{class_name}.__new__`",
                    param.name
                ),
                span,
                path: path.to_owned(),
                help: Some(format!(
                    "Pass a value of type `{resolved_type}` for parameter `{}`",
                    param.name
                )),
                note: Some(format!(
                    "`{class_name}` is specialized with type arguments `[{}]`, \
                     binding `{}` to `{resolved_type}`",
                    type_args.join(", "),
                    ann_trimmed
                )),
            });
        }
    }
}

/// Check if the `cls` parameter annotation is incompatible with the provided
/// type arguments.
#[expect(clippy::too_many_arguments, reason = "diagnostic context requires many parameters")]
fn check_cls_param_mismatch(
    cls_annotation: &str,
    class_name: &str,
    type_args: &[String],
    call: &ruff_python_ast::ExprCall,
    path: &str,
    class_info: &basilisk_resolver::ClassInfo,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_text_size::Ranged as _;

    // We're looking for annotations like `type[ClassName[SomeType]]`
    // e.g. `type[Class11[int]]`
    let Some(inner) = cls_annotation
        .strip_prefix("type[")
        .and_then(|s| s.strip_suffix(']'))
    else {
        return;
    };

    let inner = inner.trim();

    // Check if this is `ClassName[SpecificArgs]`
    let Some(bracket_pos) = inner.find('[') else {
        return;
    };

    let Some(ann_class_name) = inner.get(..bracket_pos) else {
        return;
    };
    let ann_class_name = ann_class_name.trim();
    if ann_class_name != class_name {
        return;
    }

    // Extract the type args from the annotation.
    let Some(ann_args_str) = inner
        .get(bracket_pos..)
        .and_then(|s| s.strip_prefix('['))
        .and_then(|s| s.strip_suffix(']'))
    else {
        return;
    };

    let ann_type_args: Vec<&str> = ann_args_str.split(',').map(str::trim).collect();

    // Check if the annotation type args contain any type variables from the class.
    let generic_param_names: Vec<&str> = class_info
        .generic_params
        .iter()
        .map(|p| p.name.as_str())
        .collect();

    let all_fixed = ann_type_args
        .iter()
        .all(|arg| !generic_param_names.contains(arg));

    if !all_fixed {
        // The cls annotation uses type variables -- substitution needed but
        // the mismatch won't occur in this case (the type variables get
        // unified with the call's type args).
        return;
    }

    // The annotation has fixed type args (e.g. `type[Class11[int]]`).
    // The call's type args must match these fixed type args.
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
        diagnostics.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "`{class_name}[{}]()` is incompatible: `__new__` expects \
                 `cls: {cls_annotation}` but received `type[{class_name}[{}]]`",
                type_args.join(", "),
                type_args.join(", ")
            ),
            span,
            path: path.to_owned(),
            help: Some(format!(
                "Use `{class_name}[{}]()` to match the expected `cls` parameter type",
                ann_type_args.join(", ")
            )),
            note: Some(format!(
                "The `__new__` method constrains `cls` to `{cls_annotation}`"
            )),
        });
    }
}

/// Resolve a string annotation by stripping surrounding quotes.
///
/// In Python, string annotations like `"type[Class11[int]]"` are forward
/// references. We strip the quotes to get the underlying type expression.
fn resolve_string_annotation(annotation: &str) -> String {
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

/// Classify the Python type of a literal expression.
fn classify_literal_type(expr: &ruff_python_ast::Expr) -> Option<&'static str> {
    use ruff_python_ast::Expr;
    match expr {
        Expr::StringLiteral(_) => Some("str"),
        Expr::NumberLiteral(num) => {
            if num.value.is_int() {
                Some("int")
            } else {
                Some("float")
            }
        }
        Expr::BooleanLiteral(_) => Some("bool"),
        Expr::BytesLiteral(_) => Some("bytes"),
        Expr::NoneLiteral(_) => Some("None"),
        _ => None,
    }
}

/// Check if an argument type is compatible with a parameter type.
fn is_type_compatible(arg_type: &str, param_type: &str) -> bool {
    // Direct match.
    if arg_type == param_type {
        return true;
    }

    // `Any` matches everything.
    if param_type == "Any" {
        return true;
    }

    // `object` matches everything.
    if param_type == "object" {
        return true;
    }

    // bool is a subtype of int.
    if param_type == "int" && arg_type == "bool" {
        return true;
    }

    // int is a subtype of float (numeric tower).
    if param_type == "float" && (arg_type == "int" || arg_type == "bool") {
        return true;
    }

    // int is a subtype of complex (numeric tower).
    if param_type == "complex" && (arg_type == "int" || arg_type == "float" || arg_type == "bool") {
        return true;
    }

    // Union types: `X | Y`
    if param_type.contains('|') {
        return param_type
            .split('|')
            .any(|part| is_type_compatible(arg_type, part.trim()));
    }

    false
}
