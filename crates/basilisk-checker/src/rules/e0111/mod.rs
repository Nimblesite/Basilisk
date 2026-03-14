//! BSK-E0111: Constructor call errors via `__init__` method.
//!
//! Detects several categories of constructor call errors when a class defines
//! or inherits `__init__`:
//!
//! 1. **Specialized generic argument mismatch** (L21): Calling `Class[int](1.0)`
//!    when `__init__` expects `x: T` and `T=int`, but `1.0` is `float`.
//!
//! 2. **Self type incompatibility** (L42): Passing a base-class instance where
//!    `Self` in `__init__` demands a subclass instance.
//!
//! 3. **Explicit self annotation mismatch** (L56): `__init__` annotates `self`
//!    as `Class4[int]` but the constructor is called as `Class4[str]()`.
//!
//! 4. **Class-scoped `TypeVar`s in self annotation** (L107): Using class-scoped
//!    type variables in a reordered `self` annotation is invalid.
//!
//! 5. **No custom `__init__` with arguments** (L130): Classes inheriting only
//!    from `object` (no custom `__init__` or `__new__`) cannot accept arguments.

use std::collections::HashMap;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, Severity};

use super::Rule;

mod helpers;

use helpers::{
    check_init_method_args, check_self_type_incompatibility, extract_type_args_text,
    has_custom_init_in_bases, is_namedtuple_class, resolve_string_annotation, CODE,
};

/// Emits BSK-E0111 for constructor call errors involving `__init__`.
pub(crate) struct ConstructorCallError;

impl Rule for ConstructorCallError {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        // Build class info map.
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

        // Collect module-level TypeVar names for class-scoped TypeVar detection.
        let typevar_names: Vec<&str> = module
            .typevar_calls
            .iter()
            .map(|tv| tv.name.as_str())
            .collect();

        // Check 4: Class-scoped `TypeVar`s in self annotation of __init__.
        check_class_scoped_typevars_in_self(
            module,
            source,
            &class_map,
            &method_map,
            &typevar_names,
            diagnostics,
        );

        // Re-parse source to walk call expressions.
        let Ok(parsed) = basilisk_parser::parse_source(source.clone(), path.clone()) else {
            return;
        };

        for stmt in &parsed.ast.body {
            check_stmt(
                stmt,
                source,
                path,
                &class_map,
                &method_map,
                &typevar_names,
                diagnostics,
            );
        }
    }
}

/// Check 4: Detect class-scoped `TypeVar`s used in `self` annotation of `__init__`
/// in a different order from the class's generic params.
fn check_class_scoped_typevars_in_self(
    module: &ResolvedModule,
    source: &str,
    _class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    typevar_names: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for class in &module.classes {
        if class.generic_params.is_empty() {
            continue;
        }

        let class_param_names: Vec<&str> = class
            .generic_params
            .iter()
            .map(|p| p.name.as_str())
            .collect();

        let Some(init_funcs) = method_map.get(&(class.name.as_str(), "__init__")) else {
            continue;
        };

        for init_func in init_funcs {
            // Skip overload decorators — only check the implementation.
            if init_func.decorators.iter().any(|d| d == "overload") {
                continue;
            }

            let Some(self_param) = init_func.parameters.first() else {
                continue;
            };

            let Some(ann_span) = self_param.annotation_span else {
                continue;
            };

            let Some(ann_text) = ann_span.slice_source(source) else {
                continue;
            };

            let resolved = resolve_string_annotation(ann_text.trim());

            // Extract type args from annotation like "Class8[T2, T1]"
            let Some(bracket_start) = resolved.find('[') else {
                continue;
            };
            let Some(bracket_end) = resolved.rfind(']') else {
                continue;
            };

            let ann_class_name = resolved[..bracket_start].trim();
            if ann_class_name != class.name {
                continue;
            }

            let args_str = &resolved[bracket_start + 1..bracket_end];
            let ann_args: Vec<&str> = args_str.split(',').map(str::trim).collect();

            // Check if all annotation args are class-scoped TypeVars.
            let all_class_scoped = ann_args
                .iter()
                .all(|arg| class_param_names.contains(arg) && typevar_names.contains(arg));

            if !all_class_scoped {
                continue;
            }

            // Check if the order differs from the class generic params.
            if ann_args.len() == class_param_names.len()
                && ann_args
                    .iter()
                    .zip(class_param_names.iter())
                    .any(|(a, b)| a != b)
            {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Class-scoped type variables should not be used in the `self` \
                         annotation of `__init__` in class `{}`",
                        class.name
                    ),
                    span: init_func.def_span,
                    path: module.path.clone(),
                    help: Some(
                        "Use function-scoped type variables instead of class-scoped ones \
                         in the `self` annotation"
                            .to_owned(),
                    ),
                    note: Some(format!(
                        "Class `{}` declares generic params `[{}]` but `self` annotation \
                         uses `[{}]`",
                        class.name,
                        class_param_names.join(", "),
                        ann_args.join(", ")
                    )),
                });
            }
        }
    }
}

/// Walk a statement looking for constructor call expressions.
fn check_stmt(
    stmt: &ruff_python_ast::Stmt,
    source: &str,
    path: &str,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    typevar_names: &[&str],
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
                typevar_names,
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
                typevar_names,
                diagnostics,
            );
        }
        Stmt::AnnAssign(node) => {
            if let Some(val) = node.value.as_deref() {
                check_expr_recursive(
                    val,
                    source,
                    path,
                    class_map,
                    method_map,
                    typevar_names,
                    diagnostics,
                );
            }
        }
        Stmt::Try(try_stmt) => {
            for body in [&try_stmt.body, &try_stmt.orelse, &try_stmt.finalbody] {
                for s in body {
                    check_stmt(
                        s,
                        source,
                        path,
                        class_map,
                        method_map,
                        typevar_names,
                        diagnostics,
                    );
                }
            }
            for handler in &try_stmt.handlers {
                let ruff_python_ast::ExceptHandler::ExceptHandler(h) = handler;
                for s in &h.body {
                    check_stmt(
                        s,
                        source,
                        path,
                        class_map,
                        method_map,
                        typevar_names,
                        diagnostics,
                    );
                }
            }
        }
        _ => {}
    }
}

/// Recursively check expressions for constructor call errors.
fn check_expr_recursive(
    expr: &ruff_python_ast::Expr,
    source: &str,
    path: &str,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    typevar_names: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Expr;

    if let Expr::Call(call) = expr {
        // Recurse into arguments first.
        for arg in &call.arguments.args {
            check_expr_recursive(
                arg,
                source,
                path,
                class_map,
                method_map,
                typevar_names,
                diagnostics,
            );
        }

        check_constructor_call(
            call,
            source,
            path,
            class_map,
            method_map,
            typevar_names,
            diagnostics,
        );
    }
}

/// Check a single call expression for constructor call errors.
fn check_constructor_call(
    call: &ruff_python_ast::ExprCall,
    source: &str,
    path: &str,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    typevar_names: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Expr;

    match call.func.as_ref() {
        // Case A: Simple class call like `Class11(1)` or `Class3(Class2(None))`
        Expr::Name(name_node) => {
            let class_name = name_node.id.as_str();
            let Some(class_info) = class_map.get(class_name) else {
                return;
            };

            // Check 5: No custom __init__ with arguments.
            check_no_init_with_args(
                call,
                class_name,
                class_info,
                class_map,
                method_map,
                path,
                diagnostics,
            );

            // Check 2: Self type incompatibility through inheritance.
            check_self_type_incompatibility(
                call,
                class_name,
                class_info,
                source,
                class_map,
                method_map,
                path,
                diagnostics,
            );
        }
        // Case B: Specialized call like `Class1[int](1.0)` or `Class4[str]()`
        Expr::Subscript(sub) => {
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

            let type_args = extract_type_args_text(&sub.slice, source);

            // Build substitution map.
            let mut substitutions: HashMap<&str, &str> = HashMap::new();
            for (idx, param) in class_info.generic_params.iter().enumerate() {
                if let Some(arg) = type_args.get(idx) {
                    let _ = substitutions.insert(param.name.as_str(), arg.as_str());
                }
            }

            // Check 1: Argument type mismatch after substitution (__init__).
            if let Some(init_funcs) = method_map.get(&(class_name, "__init__")) {
                for init_func in init_funcs {
                    if init_func.decorators.iter().any(|d| d == "overload") {
                        continue;
                    }
                    check_init_method_args(
                        init_func,
                        &substitutions,
                        call,
                        class_name,
                        &type_args,
                        source,
                        path,
                        class_info,
                        typevar_names,
                        diagnostics,
                    );
                }
            }
        }
        _ => {}
    }
}

/// Check 5: Classes without custom `__init__` that receive arguments.
fn check_no_init_with_args(
    call: &ruff_python_ast::ExprCall,
    class_name: &str,
    class_info: &basilisk_resolver::ClassInfo,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_text_size::Ranged as _;

    // If there are no positional arguments, nothing to check.
    if call.arguments.args.is_empty() && call.arguments.keywords.is_empty() {
        return;
    }

    // NamedTuple classes have a synthesized __new__; do not flag them here.
    if is_namedtuple_class(class_info, class_map) {
        return;
    }

    // Check if the class itself defines __init__ or __new__.
    if method_map.contains_key(&(class_name, "__init__"))
        || method_map.contains_key(&(class_name, "__new__"))
    {
        return;
    }

    // Check if any base class (other than object) defines __init__ or __new__.
    if has_custom_init_in_bases(class_info, class_map, method_map) {
        return;
    }

    let range = call.range();
    let span = Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    };

    diagnostics.push(Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Class `{class_name}` does not define `__init__` or `__new__` and inherits \
             only from `object`; constructor does not accept arguments"
        ),
        span,
        path: path.to_owned(),
        help: Some(format!(
            "Define an `__init__` method on `{class_name}` or one of its base classes"
        )),
        note: None,
    });
}
